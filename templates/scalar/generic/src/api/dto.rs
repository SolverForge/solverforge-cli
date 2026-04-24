use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use solverforge::{
    HardSoftScore, SolverLifecycleState, SolverSnapshot, SolverSnapshotAnalysis, SolverStatus,
    SolverTelemetry, SolverTerminalReason,
};
use std::time::Duration;

use crate::domain::Plan;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanDto {
    #[serde(flatten)]
    pub fields: Map<String, Value>,
    #[serde(default)]
    pub score: Option<String>,
}

/// Constraint analysis result.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConstraintAnalysisDto {
    pub name: String,
    pub weight: String,
    pub score: String,
    pub match_count: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalyzeResponse {
    pub score: String,
    pub constraints: Vec<ConstraintAnalysisDto>,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TelemetryDto {
    pub elapsed_ms: u64,
    pub step_count: u64,
    pub moves_generated: u64,
    pub moves_evaluated: u64,
    pub moves_accepted: u64,
    pub score_calculations: u64,
    pub generation_ms: u64,
    pub evaluation_ms: u64,
    pub moves_per_second: u64,
    pub acceptance_rate: f64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JobSummaryDto {
    pub id: String,
    pub job_id: String,
    pub lifecycle_state: &'static str,
    pub terminal_reason: Option<&'static str>,
    pub checkpoint_available: bool,
    pub event_sequence: u64,
    pub snapshot_revision: Option<u64>,
    pub current_score: Option<String>,
    pub best_score: Option<String>,
    pub telemetry: TelemetryDto,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JobSnapshotDto {
    pub id: String,
    pub job_id: String,
    pub snapshot_revision: u64,
    pub lifecycle_state: &'static str,
    pub terminal_reason: Option<&'static str>,
    pub current_score: Option<String>,
    pub best_score: Option<String>,
    pub telemetry: TelemetryDto,
    pub solution: PlanDto,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JobAnalysisDto {
    pub id: String,
    pub job_id: String,
    pub snapshot_revision: u64,
    pub lifecycle_state: &'static str,
    pub terminal_reason: Option<&'static str>,
    pub analysis: AnalyzeResponse,
}

impl PlanDto {
    pub fn from_plan(plan: &Plan) -> Self {
        let mut fields = match serde_json::to_value(plan).expect("failed to serialize plan") {
            Value::Object(map) => map,
            _ => Map::new(),
        };
        let score = fields.remove("score").and_then(|value| {
            if value.is_null() {
                None
            } else if let Some(score) = value.as_str() {
                Some(score.to_string())
            } else {
                Some(value.to_string())
            }
        });

        Self { fields, score }
    }

    pub fn to_domain(&self) -> Result<Plan, serde_json::Error> {
        let mut fields = self.fields.clone();
        let _ = &self.score;
        fields.insert("score".to_string(), Value::Null);
        serde_json::from_value(Value::Object(fields))
    }
}

impl TelemetryDto {
    pub fn from_runtime(telemetry: SolverTelemetry) -> Self {
        Self {
            elapsed_ms: duration_to_millis(telemetry.elapsed),
            step_count: telemetry.step_count,
            moves_generated: telemetry.moves_generated,
            moves_evaluated: telemetry.moves_evaluated,
            moves_accepted: telemetry.moves_accepted,
            score_calculations: telemetry.score_calculations,
            generation_ms: duration_to_millis(telemetry.generation_time),
            evaluation_ms: duration_to_millis(telemetry.evaluation_time),
            moves_per_second: whole_units_per_second(telemetry.moves_evaluated, telemetry.elapsed),
            acceptance_rate: derive_acceptance_rate(
                telemetry.moves_accepted,
                telemetry.moves_evaluated,
            ),
        }
    }
}

impl JobSummaryDto {
    pub fn from_status(job_id: usize, status: &SolverStatus<HardSoftScore>) -> Self {
        Self {
            id: job_id.to_string(),
            job_id: job_id.to_string(),
            lifecycle_state: lifecycle_state_label(status.lifecycle_state),
            terminal_reason: status.terminal_reason.map(terminal_reason_label),
            checkpoint_available: status.checkpoint_available,
            event_sequence: status.event_sequence,
            snapshot_revision: status.latest_snapshot_revision,
            current_score: status.current_score.map(|score| score.to_string()),
            best_score: status.best_score.map(|score| score.to_string()),
            telemetry: TelemetryDto::from_runtime(status.telemetry),
        }
    }
}

impl JobSnapshotDto {
    pub fn from_snapshot(snapshot: &SolverSnapshot<Plan>) -> Self {
        Self {
            id: snapshot.job_id.to_string(),
            job_id: snapshot.job_id.to_string(),
            snapshot_revision: snapshot.snapshot_revision,
            lifecycle_state: lifecycle_state_label(snapshot.lifecycle_state),
            terminal_reason: snapshot.terminal_reason.map(terminal_reason_label),
            current_score: snapshot.current_score.map(|score| score.to_string()),
            best_score: snapshot.best_score.map(|score| score.to_string()),
            telemetry: TelemetryDto::from_runtime(snapshot.telemetry),
            solution: PlanDto::from_plan(&snapshot.solution),
        }
    }
}

impl JobAnalysisDto {
    pub fn from_snapshot_analysis(
        snapshot: &SolverSnapshotAnalysis<HardSoftScore>,
        analysis: AnalyzeResponse,
    ) -> Self {
        Self {
            id: snapshot.job_id.to_string(),
            job_id: snapshot.job_id.to_string(),
            snapshot_revision: snapshot.snapshot_revision,
            lifecycle_state: lifecycle_state_label(snapshot.lifecycle_state),
            terminal_reason: snapshot.terminal_reason.map(terminal_reason_label),
            analysis,
        }
    }
}

pub fn analysis_response(analysis: &solverforge::ScoreAnalysis<HardSoftScore>) -> AnalyzeResponse {
    AnalyzeResponse {
        score: analysis.score.to_string(),
        constraints: analysis
            .constraints
            .iter()
            .map(|constraint| ConstraintAnalysisDto {
                name: constraint.name.clone(),
                weight: constraint.weight.to_string(),
                score: constraint.score.to_string(),
                match_count: constraint.match_count,
            })
            .collect(),
    }
}

pub fn lifecycle_state_label(state: SolverLifecycleState) -> &'static str {
    match state {
        SolverLifecycleState::Solving => "SOLVING",
        SolverLifecycleState::PauseRequested => "PAUSE_REQUESTED",
        SolverLifecycleState::Paused => "PAUSED",
        SolverLifecycleState::Completed => "COMPLETED",
        SolverLifecycleState::Cancelled => "CANCELLED",
        SolverLifecycleState::Failed => "FAILED",
    }
}

pub fn terminal_reason_label(reason: SolverTerminalReason) -> &'static str {
    match reason {
        SolverTerminalReason::Completed => "completed",
        SolverTerminalReason::TerminatedByConfig => "terminated_by_config",
        SolverTerminalReason::Cancelled => "cancelled",
        SolverTerminalReason::Failed => "failed",
    }
}

fn duration_to_millis(duration: Duration) -> u64 {
    duration.as_millis().min(u128::from(u64::MAX)) as u64
}

fn whole_units_per_second(count: u64, elapsed: Duration) -> u64 {
    let nanos = elapsed.as_nanos();
    if nanos == 0 {
        0
    } else {
        let per_second = u128::from(count)
            .saturating_mul(1_000_000_000)
            .checked_div(nanos)
            .unwrap_or(0);
        per_second.min(u128::from(u64::MAX)) as u64
    }
}

fn derive_acceptance_rate(moves_accepted: u64, moves_evaluated: u64) -> f64 {
    if moves_evaluated == 0 {
        0.0
    } else {
        moves_accepted as f64 / moves_evaluated as f64
    }
}
