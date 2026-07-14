use parking_lot::RwLock;
use serde::Serialize;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{broadcast, mpsc};

use solverforge::{
    HardSoftScore, QualifiedCandidateTraceRunProvenance, SolverEvent, SolverEventMetadata,
    SolverLifecycleState, SolverManager, SolverManagerError, SolverSnapshot,
    SolverSnapshotAnalysis, SolverStatus, SolverTelemetryDetail, SolverTerminalReason,
};

use crate::api::{PlanDto, TelemetryDto};
use crate::domain::Plan;

// Static manager — must be 'static for retained job execution.
static MANAGER: SolverManager<Plan> = SolverManager::new();

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct JobEventPayload {
    id: String,
    job_id: String,
    event_type: &'static str,
    event_sequence: u64,
    lifecycle_state: &'static str,
    terminal_reason: Option<&'static str>,
    telemetry: TelemetryDto,
    current_score: Option<String>,
    best_score: Option<String>,
    snapshot_revision: Option<u64>,
    solution: Option<PlanDto>,
    error: Option<String>,
}

struct JobState {
    sse_tx: broadcast::Sender<String>,
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
        Ok(self.retain_job(job_id, receiver))
    }

    pub fn start_qualified_job(
        &self,
        plan: Plan,
        provenance: QualifiedCandidateTraceRunProvenance,
    ) -> Result<String, SolverManagerError> {
        let (job_id, receiver) =
            MANAGER.solve_with_qualified_candidate_trace_provenance(plan, provenance)?;
        Ok(self.retain_job(job_id, receiver))
    }

    fn retain_job(
        &self,
        job_id: usize,
        receiver: mpsc::UnboundedReceiver<SolverEvent<Plan>>,
    ) -> String {
        let (sse_tx, _) = broadcast::channel(64);

        self.jobs.write().insert(
            job_id,
            JobState {
                sse_tx: sse_tx.clone(),
            },
        );

        let jobs = Arc::clone(&self.jobs);
        tokio::spawn(async move {
            drain_receiver(jobs, job_id, sse_tx, receiver).await;
        });

        job_id.to_string()
    }

    pub fn subscribe(&self, id: &str) -> Option<broadcast::Receiver<String>> {
        let job_id = parse_job_id(id).ok()?;
        self.jobs
            .read()
            .get(&job_id)
            .map(|state| state.sse_tx.subscribe())
    }

    pub fn bootstrap_event(&self, id: &str) -> Result<String, SolverManagerError> {
        let job_id = parse_job_id(id)?;
        let status = MANAGER.get_status(job_id)?;
        if let Some(revision) = status.latest_snapshot_revision {
            let snapshot = MANAGER.get_snapshot(job_id, Some(revision))?;
            return Ok(snapshot_status_event_payload(
                job_id,
                bootstrap_snapshot_event_type(status.lifecycle_state),
                &status,
                &snapshot,
            ));
        }

        Ok(status_event_payload(
            job_id,
            bootstrap_event_type(status.lifecycle_state),
            &status,
        ))
    }

    pub fn get_status(&self, id: &str) -> Result<SolverStatus<HardSoftScore>, SolverManagerError> {
        let job_id = parse_job_id(id)?;
        MANAGER.get_status(job_id)
    }

    pub fn get_telemetry_detail(
        &self,
        id: &str,
    ) -> Result<SolverTelemetryDetail<HardSoftScore>, SolverManagerError> {
        MANAGER.get_telemetry_detail(parse_job_id(id)?)
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

        if !jobs.read().contains_key(&job_id) {
            return;
        }

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
        telemetry: TelemetryDto::from_runtime(&status.telemetry),
        current_score: status.current_score.map(|score| score.to_string()),
        best_score: status.best_score.map(|score| score.to_string()),
        snapshot_revision: status.latest_snapshot_revision,
        solution: None,
        error: None,
    })
}

fn snapshot_status_event_payload(
    job_id: usize,
    event_type: &'static str,
    status: &SolverStatus<HardSoftScore>,
    snapshot: &SolverSnapshot<Plan>,
) -> String {
    serialize_payload(JobEventPayload {
        id: job_id.to_string(),
        job_id: job_id.to_string(),
        event_type,
        event_sequence: status.event_sequence,
        lifecycle_state: lifecycle_state_label(status.lifecycle_state),
        terminal_reason: status.terminal_reason.map(terminal_reason_label),
        telemetry: TelemetryDto::from_runtime(&status.telemetry),
        current_score: status
            .current_score
            .or(snapshot.current_score)
            .map(|score| score.to_string()),
        best_score: status
            .best_score
            .or(snapshot.best_score)
            .map(|score| score.to_string()),
        snapshot_revision: Some(snapshot.snapshot_revision),
        solution: Some(PlanDto::from_plan(&snapshot.solution)),
        error: None,
    })
}

fn bootstrap_event_type(state: SolverLifecycleState) -> &'static str {
    match state {
        SolverLifecycleState::Solving => "progress",
        SolverLifecycleState::PauseRequested => "pause_requested",
        SolverLifecycleState::Paused => "paused",
        SolverLifecycleState::Completed => "completed",
        SolverLifecycleState::Cancelled => "cancelled",
        SolverLifecycleState::Failed => "failed",
    }
}

fn bootstrap_snapshot_event_type(state: SolverLifecycleState) -> &'static str {
    match state {
        SolverLifecycleState::Solving => "best_solution",
        other => bootstrap_event_type(other),
    }
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
        telemetry: TelemetryDto::from_runtime(&metadata.telemetry),
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
