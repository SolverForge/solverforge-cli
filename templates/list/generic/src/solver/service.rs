use parking_lot::RwLock;
use serde::Serialize;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{broadcast, mpsc};

use solverforge::{
    HardSoftScore, SolverEvent, SolverEventMetadata, SolverLifecycleState, SolverManager,
    SolverManagerError, SolverSnapshot, SolverSnapshotAnalysis, SolverStatus, SolverTelemetry,
    SolverTerminalReason,
};

use crate::api::PlanDto;
use crate::domain::Plan;

// Static manager — must be 'static for retained job execution.
static MANAGER: SolverManager<Plan> = SolverManager::new();

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct TelemetryPayload {
    elapsed_ms: u64,
    step_count: u64,
    moves_evaluated: u64,
    moves_accepted: u64,
    score_calculations: u64,
    moves_per_second: u64,
    acceptance_rate: f64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct JobEventPayload {
    id: String,
    job_id: String,
    event_type: &'static str,
    event_sequence: u64,
    lifecycle_state: &'static str,
    terminal_reason: Option<&'static str>,
    telemetry: TelemetryPayload,
    current_score: Option<String>,
    best_score: Option<String>,
    snapshot_revision: Option<u64>,
    solution: Option<PlanDto>,
    error: Option<String>,
}

struct JobState {
    sse_tx: broadcast::Sender<String>,
    last_event: String,
}

/// Manages retained solving jobs and broadcasts lifecycle-complete SSE payloads.
pub struct SolverService {
    jobs: Arc<RwLock<HashMap<usize, JobState>>>,
}

impl SolverService {
    pub fn new() -> Self {
        Self {
            jobs: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub fn start_job(&self, plan: Plan) -> Result<String, SolverManagerError> {
        let (job_id, receiver) = MANAGER.solve(plan)?;
        let status = MANAGER.get_status(job_id)?;
        let initial_event = status_event_payload(job_id, "progress", &status);
        let (sse_tx, _) = broadcast::channel(64);

        self.jobs.write().insert(
            job_id,
            JobState {
                sse_tx: sse_tx.clone(),
                last_event: initial_event,
            },
        );

        let jobs = Arc::clone(&self.jobs);
        tokio::spawn(async move {
            drain_receiver(jobs, job_id, sse_tx, receiver).await;
        });

        Ok(job_id.to_string())
    }

    pub fn subscribe(&self, id: &str) -> Option<broadcast::Receiver<String>> {
        let job_id = parse_job_id(id).ok()?;
        self.jobs
            .read()
            .get(&job_id)
            .map(|state| state.sse_tx.subscribe())
    }

    pub fn sse_snapshot(&self, id: &str) -> Option<String> {
        let job_id = parse_job_id(id).ok()?;
        self.jobs
            .read()
            .get(&job_id)
            .map(|state| state.last_event.clone())
    }

    pub fn get_status(&self, id: &str) -> Result<SolverStatus<HardSoftScore>, SolverManagerError> {
        let job_id = parse_job_id(id)?;
        MANAGER.get_status(job_id)
    }

    pub fn pause(&self, id: &str) -> Result<(), SolverManagerError> {
        MANAGER.pause(parse_job_id(id)?)
    }

    pub fn resume(&self, id: &str) -> Result<(), SolverManagerError> {
        MANAGER.resume(parse_job_id(id)?)
    }

    pub fn cancel(&self, id: &str) -> Result<(), SolverManagerError> {
        MANAGER.cancel(parse_job_id(id)?)
    }

    pub fn delete(&self, id: &str) -> Result<(), SolverManagerError> {
        let job_id = parse_job_id(id)?;
        MANAGER.delete(job_id)?;
        self.jobs.write().remove(&job_id);
        Ok(())
    }

    pub fn get_snapshot(
        &self,
        id: &str,
        snapshot_revision: Option<u64>,
    ) -> Result<SolverSnapshot<Plan>, SolverManagerError> {
        MANAGER.get_snapshot(parse_job_id(id)?, snapshot_revision)
    }

    pub fn analyze_snapshot(
        &self,
        id: &str,
        snapshot_revision: Option<u64>,
    ) -> Result<SolverSnapshotAnalysis<HardSoftScore>, SolverManagerError> {
        MANAGER.analyze_snapshot(parse_job_id(id)?, snapshot_revision)
    }
}

async fn drain_receiver(
    jobs: Arc<RwLock<HashMap<usize, JobState>>>,
    job_id: usize,
    sse_tx: broadcast::Sender<String>,
    mut receiver: mpsc::UnboundedReceiver<SolverEvent<Plan>>,
) {
    while let Some(event) = receiver.recv().await {
        let payload = match &event {
            SolverEvent::Progress { metadata } => {
                event_payload(job_id, "progress", metadata, None, None)
            }
            SolverEvent::BestSolution { metadata, solution } => {
                event_payload(job_id, "best_solution", metadata, Some(solution), None)
            }
            SolverEvent::PauseRequested { metadata } => {
                event_payload(job_id, "pause_requested", metadata, None, None)
            }
            SolverEvent::Paused { metadata } => {
                event_payload(job_id, "paused", metadata, None, None)
            }
            SolverEvent::Resumed { metadata } => {
                event_payload(job_id, "resumed", metadata, None, None)
            }
            SolverEvent::Completed { metadata, solution } => {
                event_payload(job_id, "completed", metadata, Some(solution), None)
            }
            SolverEvent::Cancelled { metadata } => {
                event_payload(job_id, "cancelled", metadata, None, None)
            }
            SolverEvent::Failed { metadata, error } => {
                event_payload(job_id, "failed", metadata, None, Some(error.as_str()))
            }
        };

        let mut jobs = jobs.write();
        if let Some(state) = jobs.get_mut(&job_id) {
            state.last_event = payload.clone();
        } else {
            return;
        }
        drop(jobs);

        let _ = sse_tx.send(payload);
    }
}

fn parse_job_id(id: &str) -> Result<usize, SolverManagerError> {
    id.parse::<usize>()
        .map_err(|_| SolverManagerError::JobNotFound { job_id: usize::MAX })
}

fn status_event_payload(
    job_id: usize,
    event_type: &'static str,
    status: &SolverStatus<HardSoftScore>,
) -> String {
    serialize_payload(JobEventPayload {
        id: job_id.to_string(),
        job_id: job_id.to_string(),
        event_type,
        event_sequence: status.event_sequence,
        lifecycle_state: lifecycle_state_label(status.lifecycle_state),
        terminal_reason: status.terminal_reason.map(terminal_reason_label),
        telemetry: telemetry_payload(status.telemetry),
        current_score: status.current_score.map(|score| score.to_string()),
        best_score: status.best_score.map(|score| score.to_string()),
        snapshot_revision: status.latest_snapshot_revision,
        solution: None,
        error: None,
    })
}

fn event_payload(
    job_id: usize,
    event_type: &'static str,
    metadata: &SolverEventMetadata<HardSoftScore>,
    solution: Option<&Plan>,
    error: Option<&str>,
) -> String {
    serialize_payload(JobEventPayload {
        id: job_id.to_string(),
        job_id: job_id.to_string(),
        event_type,
        event_sequence: metadata.event_sequence,
        lifecycle_state: lifecycle_state_label(metadata.lifecycle_state),
        terminal_reason: metadata.terminal_reason.map(terminal_reason_label),
        telemetry: telemetry_payload(metadata.telemetry),
        current_score: metadata.current_score.map(|score| score.to_string()),
        best_score: metadata.best_score.map(|score| score.to_string()),
        snapshot_revision: metadata.snapshot_revision,
        solution: solution.map(PlanDto::from_plan),
        error: error.map(ToOwned::to_owned),
    })
}

fn serialize_payload(payload: JobEventPayload) -> String {
    serde_json::to_string(&payload).expect("failed to serialize solver lifecycle payload")
}

fn telemetry_payload(telemetry: SolverTelemetry) -> TelemetryPayload {
    TelemetryPayload {
        elapsed_ms: telemetry.elapsed_ms,
        step_count: telemetry.step_count,
        moves_evaluated: telemetry.moves_evaluated,
        moves_accepted: telemetry.moves_accepted,
        score_calculations: telemetry.score_calculations,
        moves_per_second: telemetry.moves_per_second,
        acceptance_rate: telemetry.acceptance_rate,
    }
}

fn lifecycle_state_label(state: SolverLifecycleState) -> &'static str {
    match state {
        SolverLifecycleState::Solving => "SOLVING",
        SolverLifecycleState::PauseRequested => "PAUSE_REQUESTED",
        SolverLifecycleState::Paused => "PAUSED",
        SolverLifecycleState::Completed => "COMPLETED",
        SolverLifecycleState::Cancelled => "CANCELLED",
        SolverLifecycleState::Failed => "FAILED",
    }
}

fn terminal_reason_label(reason: SolverTerminalReason) -> &'static str {
    match reason {
        SolverTerminalReason::Completed => "completed",
        SolverTerminalReason::TerminatedByConfig => "terminated_by_config",
        SolverTerminalReason::Cancelled => "cancelled",
        SolverTerminalReason::Failed => "failed",
    }
}

impl Default for SolverService {
    fn default() -> Self {
        Self::new()
    }
}
