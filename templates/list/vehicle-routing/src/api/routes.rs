use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::{delete, get, post},
    Json, Router,
};
use std::sync::Arc;
use uuid::Uuid;

use super::dto::{InstanceDto, SolutionDto};
use crate::data::demo_instance;
use crate::domain::ProblemData;
use crate::solver::{SolveJob, SolverService};

pub struct AppState {
    pub solver: SolverService,
}

impl AppState {
    pub fn new() -> Self {
        Self { solver: SolverService::new() }
    }
}

impl Default for AppState {
    fn default() -> Self { Self::new() }
}

pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/demo-data", get(get_demo))
        .route("/solutions", post(create_solution))
        .route("/solutions", get(list_solutions))
        .route("/solutions/{id}", get(get_solution))
        .route("/solutions/{id}", delete(stop_solving))
        .with_state(state)
}

async fn health() -> &'static str { "OK" }

async fn get_demo() -> Json<InstanceDto> {
    let (data, n_vehicles) = demo_instance();
    Json(InstanceDto {
        capacity: data.capacity,
        depot: data.depot,
        demands: data.demands.iter().map(|&d| d as i32).collect(),
        distance_matrix: data.distance_matrix,
        n_vehicles,
        time_limit_secs: 30,
    })
}

async fn create_solution(
    State(state): State<Arc<AppState>>,
    Json(dto): Json<InstanceDto>,
) -> String {
    let id = Uuid::new_v4().to_string();
    let problem = Box::new(ProblemData {
        capacity: dto.capacity,
        depot: dto.depot,
        demands: dto.demands.iter().map(|&d| d as i32).collect(),
        distance_matrix: dto.distance_matrix,
    });
    let job = SolveJob::new(id.clone(), problem, dto.n_vehicles, dto.time_limit_secs);
    let job = state.solver.create_job(id.clone(), job);
    state.solver.start_solving(job);
    id
}

async fn list_solutions(State(state): State<Arc<AppState>>) -> Json<Vec<String>> {
    Json(state.solver.list_jobs())
}

async fn get_solution(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<SolutionDto>, StatusCode> {
    match state.solver.get_job(&id) {
        Some(job) => {
            let g = job.read();
            Ok(Json(SolutionDto {
                routes: g.routes.clone(),
                cost: g.cost,
                score: g.score.clone(),
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
