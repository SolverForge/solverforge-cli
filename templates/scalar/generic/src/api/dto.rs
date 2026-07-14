use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use solverforge::{
    CandidateTraceExternalDigest, HardSoftScore, QualifiedCandidateTraceRunProvenance,
    SolverLifecycleState, SolverSnapshot, SolverSnapshotAnalysis, SolverStatus,
    SolverTelemetryDetail, SolverTerminalReason,
};

use super::telemetry::{CandidateTraceDto, TelemetryDto};

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

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JobTelemetryDetailDto {
    pub id: String,
    pub job_id: String,
    pub status: JobSummaryDto,
    pub candidate_trace: Option<CandidateTraceDto>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QualifiedJobRequestDto {
    pub plan: PlanDto,
    pub provenance: QualifiedCandidateTraceProvenanceDto,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QualifiedCandidateTraceProvenanceDto {
    pub schema_digest_sha256: String,
    pub instance_digest_sha256: String,
    pub initial_state_digest_sha256: String,
    pub core_tree_digest_sha256: String,
    pub build_digest_sha256: String,
    pub producer: String,
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
            telemetry: TelemetryDto::from_runtime(&status.telemetry),
        }
    }
}

impl JobTelemetryDetailDto {
    pub fn from_runtime(detail: &SolverTelemetryDetail<HardSoftScore>) -> Self {
        let job_id = detail.status.job_id;
        Self {
            id: job_id.to_string(),
            job_id: job_id.to_string(),
            status: JobSummaryDto::from_status(job_id, &detail.status),
            candidate_trace: detail
                .candidate_trace
                .as_ref()
                .map(CandidateTraceDto::from_runtime),
        }
    }
}

impl QualifiedCandidateTraceProvenanceDto {
    pub fn to_runtime(&self) -> Result<QualifiedCandidateTraceRunProvenance, String> {
        QualifiedCandidateTraceRunProvenance::externally_attested(
            parse_sha256("schemaDigestSha256", &self.schema_digest_sha256)?,
            parse_sha256("instanceDigestSha256", &self.instance_digest_sha256)?,
            parse_sha256(
                "initialStateDigestSha256",
                &self.initial_state_digest_sha256,
            )?,
            parse_sha256("coreTreeDigestSha256", &self.core_tree_digest_sha256)?,
            parse_sha256("buildDigestSha256", &self.build_digest_sha256)?,
            self.producer.clone(),
        )
        .map_err(|error| error.to_string())
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
            telemetry: TelemetryDto::from_runtime(&snapshot.telemetry),
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

fn parse_sha256(name: &str, value: &str) -> Result<CandidateTraceExternalDigest, String> {
    if value.len() != 64 {
        return Err(format!(
            "{name} must contain exactly 64 hexadecimal characters"
        ));
    }
    let mut bytes = [0_u8; 32];
    for (index, pair) in value.as_bytes().chunks_exact(2).enumerate() {
        let high = hex_nibble(pair[0]).ok_or_else(|| format!("{name} must be hexadecimal"))?;
        let low = hex_nibble(pair[1]).ok_or_else(|| format!("{name} must be hexadecimal"))?;
        bytes[index] = (high << 4) | low;
    }
    Ok(CandidateTraceExternalDigest::sha256(bytes))
}

fn hex_nibble(value: u8) -> Option<u8> {
    match value {
        b'0'..=b'9' => Some(value - b'0'),
        b'a'..=b'f' => Some(value - b'a' + 10),
        b'A'..=b'F' => Some(value - b'A' + 10),
        _ => None,
    }
}
