use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use super::dto::{analysis_response, JobAnalysisDto, JobSnapshotDto, JobSummaryDto, PlanDto};
use super::sse;
use crate::data::{generate, DemoData};
use crate::solver::SolverService;

/// Shared application state.
pub struct AppState {
    pub solver: SolverService,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            solver: SolverService::new(),
        }
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}

/// Creates the API router.
pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/info", get(info))
        .route("/demo-data", get(list_demo_data))
        .route("/demo-data/{id}", get(get_demo_data))
        .route("/jobs", post(create_job))
        .route("/jobs/{id}", get(get_job).delete(delete_job))
        .route("/jobs/{id}/status", get(get_job_status))
        .route("/jobs/{id}/snapshot", get(get_snapshot))
        .route("/jobs/{id}/analysis", get(analyze_by_id))
        .route("/jobs/{id}/pause", post(pause_job))
        .route("/jobs/{id}/resume", post(resume_job))
        .route("/jobs/{id}/cancel", post(cancel_job))
        .route("/jobs/{id}/events", get(sse::events))
        .with_state(state)
}

#[derive(Serialize)]
struct HealthResponse {
    status: &'static str,
}

async fn health() -> Json<HealthResponse> {
    Json(HealthResponse { status: "UP" })
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct InfoResponse {
    name: &'static str,
    version: &'static str,
    solver_engine: &'static str,
}

async fn info() -> Json<InfoResponse> {
    Json(InfoResponse {
        name: env!("CARGO_PKG_NAME"),
        version: env!("CARGO_PKG_VERSION"),
        solver_engine: "SolverForge",
    })
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct DemoDataCatalogResponse {
    default_id: &'static str,
    available_ids: Vec<&'static str>,
}

async fn list_demo_data() -> Json<DemoDataCatalogResponse> {
    Json(DemoDataCatalogResponse {
        default_id: DemoData::default_demo_data().id(),
        available_ids: DemoData::available_demo_data()
            .iter()
            .map(|demo| demo.id())
            .collect(),
    })
}

async fn get_demo_data(Path(id): Path<String>) -> Result<Json<PlanDto>, StatusCode> {
    let demo = id.parse::<DemoData>().map_err(|_| StatusCode::NOT_FOUND)?;
    let plan = generate(demo);
    Ok(Json(PlanDto::from_plan(&plan)))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CreateJobResponse {
    id: String,
}

async fn create_job(
    State(state): State<Arc<AppState>>,
    Json(dto): Json<PlanDto>,
) -> Result<Json<CreateJobResponse>, StatusCode> {
    let plan = dto.to_domain().map_err(|_| StatusCode::BAD_REQUEST)?;
    let id = state
        .solver
        .start_job(plan)
        .map_err(status_from_solver_error)?;
    Ok(Json(CreateJobResponse { id }))
}

async fn get_job(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<JobSummaryDto>, StatusCode> {
    let job_id = parse_job_id(&id)?;
    let status = state
        .solver
        .get_status(&id)
        .map_err(status_from_solver_error)?;
    Ok(Json(JobSummaryDto::from_status(job_id, &status)))
}

async fn get_job_status(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<JobSummaryDto>, StatusCode> {
    get_job(State(state), Path(id)).await
}

#[derive(Debug, Default, Deserialize)]
struct SnapshotQuery {
    snapshot_revision: Option<u64>,
}

async fn get_snapshot(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Query(query): Query<SnapshotQuery>,
) -> Result<Json<JobSnapshotDto>, StatusCode> {
    let snapshot = state
        .solver
        .get_snapshot(&id, query.snapshot_revision)
        .map_err(status_from_solver_error)?;
    Ok(Json(JobSnapshotDto::from_snapshot(&snapshot)))
}

async fn analyze_by_id(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Query(query): Query<SnapshotQuery>,
) -> Result<Json<JobAnalysisDto>, StatusCode> {
    let snapshot_analysis = state
        .solver
        .analyze_snapshot(&id, query.snapshot_revision)
        .map_err(status_from_solver_error)?;
    let analysis = analysis_response(&snapshot_analysis.analysis);
    Ok(Json(JobAnalysisDto::from_snapshot_analysis(
        &snapshot_analysis,
        analysis,
    )))
}

async fn pause_job(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<StatusCode, StatusCode> {
    state.solver.pause(&id).map_err(status_from_solver_error)?;
    Ok(StatusCode::ACCEPTED)
}

async fn resume_job(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<StatusCode, StatusCode> {
    state.solver.resume(&id).map_err(status_from_solver_error)?;
    Ok(StatusCode::ACCEPTED)
}

async fn cancel_job(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<StatusCode, StatusCode> {
    state.solver.cancel(&id).map_err(status_from_solver_error)?;
    Ok(StatusCode::ACCEPTED)
}

async fn delete_job(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<StatusCode, StatusCode> {
    state.solver.delete(&id).map_err(status_from_solver_error)?;
    Ok(StatusCode::NO_CONTENT)
}

fn parse_job_id(id: &str) -> Result<usize, StatusCode> {
    id.parse::<usize>().map_err(|_| StatusCode::NOT_FOUND)
}

fn status_from_solver_error(error: solverforge::SolverManagerError) -> StatusCode {
    match error {
        solverforge::SolverManagerError::NoFreeJobSlots => StatusCode::SERVICE_UNAVAILABLE,
        solverforge::SolverManagerError::JobNotFound { .. } => StatusCode::NOT_FOUND,
        solverforge::SolverManagerError::InvalidStateTransition { .. } => StatusCode::CONFLICT,
        solverforge::SolverManagerError::NoSnapshotAvailable { .. } => StatusCode::CONFLICT,
        solverforge::SolverManagerError::SnapshotNotFound { .. } => StatusCode::NOT_FOUND,
    }
}
