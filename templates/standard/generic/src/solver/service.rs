use parking_lot::RwLock;
use serde::Serialize;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{broadcast, mpsc};

use solverforge::{HardSoftScore, SolverEvent, SolverManager, SolverStatus};

use crate::api::PlanDto;
use crate::domain::Plan;

// Static manager — must be 'static for SolverManager::solve.
static MANAGER: SolverManager<Plan> = SolverManager::new();

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SsePayload {
    id: String,
    event_type: &'static str,
    solver_status: SolverStatus,
    current_score: Option<String>,
    best_score: Option<String>,
    moves_per_second: u64,
    solution: Option<PlanDto>,
}

fn sse_payload(
    id: &str,
    event_type: &'static str,
    current_score: Option<HardSoftScore>,
    best_score: Option<HardSoftScore>,
    status: SolverStatus,
    mps: u64,
    solution: Option<&Plan>,
) -> String {
    serde_json::to_string(&SsePayload {
        id: id.to_string(),
        event_type,
        solver_status: status,
        current_score: current_score.map(|s| s.to_string()),
        best_score: best_score.map(|s| s.to_string()),
        moves_per_second: mps,
        solution: solution.map(|plan| PlanDto::from_plan(plan, Some(status))),
    })
    .expect("failed to serialize solver SSE payload")
}

fn snapshot_event(state: &JobState) -> (&'static str, Option<&Plan>) {
    if state.status == SolverStatus::NotSolving && state.latest_best.is_some() {
        ("finished", state.latest_best.as_ref())
    } else if state.best_score.is_some() && state.latest_best.is_some() {
        ("best_solution", state.latest_best.as_ref())
    } else {
        ("progress", None)
    }
}

struct JobState {
    slot_id: usize,
    latest_best: Option<Plan>,
    current_score: Option<HardSoftScore>,
    best_score: Option<HardSoftScore>,
    moves_per_second: u64,
    status: SolverStatus,
    sse_tx: broadcast::Sender<String>,
}

/// Manages solving jobs using the framework SolverManager.
pub struct SolverService {
    jobs: Arc<RwLock<HashMap<String, JobState>>>,
}

impl SolverService {
    pub fn new() -> Self {
        Self {
            jobs: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub fn start_solving(&self, id: String, plan: Plan) {
        let (slot_id, receiver) = MANAGER.solve(plan.clone());
        let (sse_tx, _) = broadcast::channel(64);
        let state = JobState {
            slot_id,
            latest_best: Some(plan),
            current_score: None,
            best_score: None,
            moves_per_second: 0,
            status: SolverStatus::Solving,
            sse_tx: sse_tx.clone(),
        };
        self.jobs.write().insert(id.clone(), state);

        let jobs = Arc::clone(&self.jobs);
        tokio::spawn(async move {
            drain_receiver(jobs, id, slot_id, sse_tx, receiver).await;
        });
    }

    pub fn with_snapshot<R>(
        &self,
        id: &str,
        f: impl FnOnce(&Plan, Option<HardSoftScore>, Option<HardSoftScore>, SolverStatus) -> R,
    ) -> Option<R> {
        let jobs = self.jobs.read();
        let state = jobs.get(id)?;
        Some(f(
            state.latest_best.as_ref()?,
            state.current_score,
            state.best_score,
            state.status,
        ))
    }

    pub fn subscribe(&self, id: &str) -> Option<broadcast::Receiver<String>> {
        self.jobs.read().get(id).map(|s| s.sse_tx.subscribe())
    }

    pub fn sse_snapshot(&self, id: &str) -> Option<String> {
        let jobs = self.jobs.read();
        let state = jobs.get(id)?;
        let (event_type, solution) = snapshot_event(state);
        Some(sse_payload(
            id,
            event_type,
            state.current_score,
            state.best_score,
            state.status,
            state.moves_per_second,
            solution,
        ))
    }

    pub fn has_job(&self, id: &str) -> bool {
        self.jobs.read().contains_key(id)
    }

    pub fn list_jobs(&self) -> Vec<String> {
        self.jobs.read().keys().cloned().collect()
    }

    pub fn stop_solving(&self, id: &str) -> bool {
        let payload = {
            let mut jobs = self.jobs.write();
            if let Some(state) = jobs.get_mut(id) {
                if !MANAGER.terminate_early(state.slot_id) {
                    return false;
                }
                state.status = SolverStatus::NotSolving;
                Some(sse_payload(
                    id,
                    "finished",
                    state.current_score,
                    state.best_score,
                    SolverStatus::NotSolving,
                    state.moves_per_second,
                    state.latest_best.as_ref(),
                ))
            } else {
                None
            }
        };
        if let Some(payload) = payload {
            if let Some(state) = self.jobs.read().get(id) {
                let _ = state.sse_tx.send(payload);
            }
            return true;
        }
        false
    }

    pub fn remove_job(&self, id: &str) -> bool {
        if let Some(state) = self.jobs.write().remove(id) {
            MANAGER.free_slot(state.slot_id);
            return true;
        }
        false
    }
}

async fn drain_receiver(
    jobs: Arc<RwLock<HashMap<String, JobState>>>,
    id: String,
    slot_id: usize,
    sse_tx: broadcast::Sender<String>,
    mut receiver: mpsc::UnboundedReceiver<SolverEvent<Plan>>,
) {
    while let Some(event) = receiver.recv().await {
        match event {
            SolverEvent::Progress {
                current_score,
                best_score,
                telemetry,
            } => {
                let payload = {
                    let mut jobs = jobs.write();
                    if let Some(state) = jobs.get_mut(&id) {
                        state.current_score = current_score;
                        state.best_score = best_score.or(state.best_score);
                        state.moves_per_second = telemetry.moves_per_second;
                        Some(sse_payload(
                            &id,
                            "progress",
                            state.current_score,
                            state.best_score,
                            SolverStatus::Solving,
                            state.moves_per_second,
                            None,
                        ))
                    } else {
                        None
                    }
                };
                if let Some(payload) = payload {
                    let _ = sse_tx.send(payload);
                }
            }
            SolverEvent::BestSolution {
                solution,
                score,
                telemetry,
            } => {
                let payload = {
                    let mut jobs = jobs.write();
                    if let Some(state) = jobs.get_mut(&id) {
                        state.latest_best = Some(solution);
                        state.current_score = Some(score);
                        state.best_score = Some(score);
                        state.moves_per_second = telemetry.moves_per_second;
                        Some(sse_payload(
                            &id,
                            "best_solution",
                            state.current_score,
                            state.best_score,
                            SolverStatus::Solving,
                            state.moves_per_second,
                            state.latest_best.as_ref(),
                        ))
                    } else {
                        None
                    }
                };
                if let Some(payload) = payload {
                    let _ = sse_tx.send(payload);
                }
            }
            SolverEvent::Finished {
                solution,
                score,
                telemetry,
            } => {
                let payload = {
                    let mut jobs = jobs.write();
                    if let Some(state) = jobs.get_mut(&id) {
                        state.latest_best = Some(solution);
                        state.current_score = Some(score);
                        state.best_score = Some(score);
                        state.moves_per_second = telemetry.moves_per_second;
                        state.status = SolverStatus::NotSolving;
                        Some(sse_payload(
                            &id,
                            "finished",
                            state.current_score,
                            state.best_score,
                            SolverStatus::NotSolving,
                            state.moves_per_second,
                            state.latest_best.as_ref(),
                        ))
                    } else {
                        None
                    }
                };
                if let Some(payload) = payload {
                    let _ = sse_tx.send(payload);
                }
                return;
            }
        }
    }

    let final_payload = {
        let mut jobs = jobs.write();
        if let Some(state) = jobs.get_mut(&id) {
            state.status = SolverStatus::NotSolving;
            Some(sse_payload(
                &id,
                "finished",
                state.current_score,
                state.best_score,
                SolverStatus::NotSolving,
                state.moves_per_second,
                state.latest_best.as_ref(),
            ))
        } else {
            None
        }
    };
    if let Some(payload) = final_payload {
        let _ = sse_tx.send(payload);
    }

    let mut jobs = jobs.write();
    if let Some(state) = jobs.get_mut(&id) {
        state.status = MANAGER.get_status(slot_id);
    }
}

impl Default for SolverService {
    fn default() -> Self {
        Self::new()
    }
}
