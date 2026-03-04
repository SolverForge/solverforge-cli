use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::{delete, get, post},
    Json, Router,
};
use std::sync::Arc;
use uuid::Uuid;

use super::dto::PlanDto;
use crate::data::demo_plan;
use crate::solver::{SolverService, SolverStatus};

pub struct AppState {
    pub solver: SolverService,
}

impl AppState {
    pub fn new() -> Self {
        Self { solver: SolverService::new() }
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}

pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/demo-data", get(get_demo_data))
        .route("/plans", post(create_plan))
        .route("/plans", get(list_plans))
        .route("/plans/{id}", get(get_plan))
        .route("/plans/{id}/status", get(get_plan_status))
        .route("/plans/{id}", delete(stop_solving))
        .with_state(state)
}

async fn health() -> &'static str {
    "OK"
}

async fn get_demo_data() -> Json<PlanDto> {
    Json(PlanDto::from_plan(&demo_plan(), None))
}

async fn create_plan(
    State(state): State<Arc<AppState>>,
    Json(dto): Json<PlanDto>,
) -> String {
    let id = Uuid::new_v4().to_string();
    let plan = dto.to_domain();
    let job = state.solver.create_job(id.clone(), plan);
    state.solver.start_solving(job);
    id
}

async fn list_plans(State(state): State<Arc<AppState>>) -> Json<Vec<String>> {
    Json(state.solver.list_jobs())
}

async fn get_plan(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<PlanDto>, StatusCode> {
    match state.solver.get_job(&id) {
        Some(job) => {
            let g = job.read();
            Ok(Json(PlanDto::from_plan(&g.plan, Some(g.status))))
        }
        None => Err(StatusCode::NOT_FOUND),
    }
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct StatusResponse {
    score: Option<String>,
    solver_status: SolverStatus,
}

async fn get_plan_status(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<StatusResponse>, StatusCode> {
    match state.solver.get_job(&id) {
        Some(job) => {
            let g = job.read();
            Ok(Json(StatusResponse {
                score: g.plan.score.map(|s| s.to_string()),
                solver_status: g.status,
            }))
        }
        None => Err(StatusCode::NOT_FOUND),
    }
}

async fn stop_solving(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> StatusCode {
    state.solver.stop_solving(&id);
    if state.solver.remove_job(&id).is_some() {
        StatusCode::NO_CONTENT
    } else {
        StatusCode::NOT_FOUND
    }
}
