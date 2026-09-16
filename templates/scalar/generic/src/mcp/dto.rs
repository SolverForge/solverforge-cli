//! MCP-only tool result DTOs.
//!
//! These live with the delivery surface rather than the shared DTO contract so
//! shells without an MCP server do not compile them.

use serde::Serialize;
use solverforge::{HardSoftScore, SolverStatus};

use crate::api::dto::{JobSnapshotDto, JobSummaryDto};

/// Result of the `solve` tool. Clients without task support receive it
/// immediately and task-capable clients receive the same shape as the terminal
/// task payload, so the advertised output schema holds on both paths.
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct SolveResultDto {
    #[serde(flatten)]
    pub status: JobSummaryDto,
    pub snapshot: Option<JobSnapshotDto>,
    pub snapshot_error: Option<String>,
}

impl SolveResultDto {
    pub fn from_status(job_id: usize, status: &SolverStatus<HardSoftScore>) -> Self {
        Self {
            status: JobSummaryDto::from_status(job_id, status),
            snapshot: None,
            snapshot_error: None,
        }
    }
}
