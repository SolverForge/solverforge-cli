use serde::{Deserialize, Serialize};
use solverforge::{
    CandidateTraceExternalDigest, HardSoftScore, QualifiedCandidateTraceRunProvenance,
    SolverLifecycleState, SolverSnapshot, SolverSnapshotAnalysis, SolverStatus,
    SolverTelemetryDetail, SolverTerminalReason,
};

use super::telemetry::{CandidateTraceDto, TelemetryDto};

use crate::domain::{Container, Item, Plan};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ItemDto {
    pub id: String,
    pub name: String,
}

impl From<&Item> for ItemDto {
    fn from(item: &Item) -> Self {
        Self {
            id: item.id.clone(),
            name: item.name.clone(),
        }
    }
}

impl ItemDto {
    pub fn to_item(&self) -> Item {
        Item::new(&self.id, &self.name)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContainerDto {
    pub id: String,
    pub name: String,
    pub items: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanDto {
    pub items: Vec<ItemDto>,
    pub containers: Vec<ContainerDto>,
    #[serde(default)]
    pub score: Option<String>,
}

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
        let items: Vec<ItemDto> = plan.item_facts.iter().map(ItemDto::from).collect();
        let containers: Vec<ContainerDto> = plan
            .containers
            .iter()
            .map(|container| ContainerDto {
                id: container.id.clone(),
                name: container.name.clone(),
                items: container
                    .items
                    .iter()
                    .filter_map(|&idx| plan.item_facts.get(idx))
                    .map(|item| item.id.clone())
                    .collect(),
            })
            .collect();
        Self {
            items,
            containers,
            score: plan.score.map(|score| score.to_string()),
        }
    }

    pub fn to_domain(&self) -> Result<Plan, String> {
        let item_facts: Vec<Item> = self.items.iter().map(ItemDto::to_item).collect();
        let id_to_idx: std::collections::HashMap<&str, usize> = item_facts
            .iter()
            .enumerate()
            .map(|(idx, item)| (item.id.as_str(), idx))
            .collect();
        let containers: Vec<Container> = self
            .containers
            .iter()
            .map(|container| {
                let items: Result<Vec<usize>, String> = container
                    .items
                    .iter()
                    .map(|id| {
                        id_to_idx
                            .get(id.as_str())
                            .copied()
                            .ok_or_else(|| format!("unknown item id '{id}'"))
                    })
                    .collect();
                Ok(Container {
                    id: container.id.clone(),
                    name: container.name.clone(),
                    items: items?,
                })
            })
            .collect::<Result<Vec<_>, String>>()?;
        Ok(Plan::new(item_facts, containers))
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
