use serde::Serialize;
use solverforge::{
    AppliedMoveTelemetry, CandidatePullTelemetry, CandidateTraceCoordinate, CandidateTraceDigest,
    CandidateTraceDisposition, CandidateTraceExecutionPolicy, CandidateTraceExternalDigest,
    CandidateTraceHeader, CandidateTraceIdentity, CandidateTraceInputProvenance,
    CandidateTraceInputProvenanceStatus, CandidateTracePhaseAttribute, CandidateTracePhasePlan,
    CandidateTraceQualificationStatus, CandidateTraceSource, CandidateTraceTelemetry,
    MoveTelemetry, PhaseTelemetry, SelectorTelemetry, SolverTelemetry,
};
use std::time::Duration;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SelectorTelemetryDto {
    pub selector_index: usize,
    pub selector_label: String,
    pub moves_generated: u64,
    pub moves_evaluated: u64,
    pub moves_accepted: u64,
    pub moves_applied: u64,
    pub moves_not_doable: u64,
    pub moves_acceptor_rejected: u64,
    pub moves_forager_ignored: u64,
    pub moves_hard_improving: u64,
    pub moves_hard_neutral: u64,
    pub moves_hard_worse: u64,
    pub conflict_repair_provider_generated: u64,
    pub conflict_repair_duplicate_filtered: u64,
    pub conflict_repair_illegal_filtered: u64,
    pub conflict_repair_not_doable_filtered: u64,
    pub conflict_repair_hard_improving: u64,
    pub conflict_repair_exposed: u64,
    pub generation_ms: u64,
    pub evaluation_ms: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MoveTelemetryDto {
    pub move_label: String,
    pub moves_generated: u64,
    pub moves_evaluated: u64,
    pub moves_accepted: u64,
    pub moves_applied: u64,
    pub moves_not_doable: u64,
    pub moves_acceptor_rejected: u64,
    pub moves_forager_ignored: u64,
    pub moves_score_improving: u64,
    pub moves_applied_improving: u64,
    pub moves_score_equal: u64,
    pub moves_score_worse: u64,
    pub moves_rejected_improving: u64,
    pub applied_score_improvement: f64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PhaseTelemetryDto {
    pub phase_index: usize,
    pub phase_type: String,
    pub elapsed_ms: u64,
    pub step_count: u64,
    pub moves_generated: u64,
    pub moves_evaluated: u64,
    pub moves_accepted: u64,
    pub moves_applied: u64,
    pub moves_score_improving: u64,
    pub moves_applied_improving: u64,
    pub score_calculations: u64,
    pub generation_ms: u64,
    pub evaluation_ms: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppliedMoveTelemetryDto {
    pub step_index: u64,
    pub move_label: String,
    pub selected_candidate_index: usize,
    pub moves_generated_this_step: u64,
    pub moves_evaluated_this_step: u64,
    pub moves_accepted_this_step: u64,
    pub moves_forager_ignored_this_step: u64,
    pub score_before: f64,
    pub score_after: f64,
    pub score_delta: f64,
    pub hard_feasible_before: bool,
    pub hard_feasible_after: bool,
}

/// Compact control-plane telemetry. Candidate pulls are intentionally exposed
/// only through `CandidateTraceDto` and the dedicated diagnostics endpoint.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TelemetryDto {
    pub elapsed_ms: u64,
    pub step_count: u64,
    pub moves_generated: u64,
    pub moves_evaluated: u64,
    pub moves_accepted: u64,
    pub moves_applied: u64,
    pub moves_score_improving: u64,
    pub moves_applied_improving: u64,
    pub moves_not_doable: u64,
    pub moves_acceptor_rejected: u64,
    pub moves_forager_ignored: u64,
    pub moves_hard_improving: u64,
    pub moves_hard_neutral: u64,
    pub moves_hard_worse: u64,
    pub conflict_repair_provider_generated: u64,
    pub conflict_repair_duplicate_filtered: u64,
    pub conflict_repair_illegal_filtered: u64,
    pub conflict_repair_not_doable_filtered: u64,
    pub conflict_repair_hard_improving: u64,
    pub conflict_repair_exposed: u64,
    pub score_calculations: u64,
    pub construction_slots_assigned: u64,
    pub construction_slots_kept: u64,
    pub construction_slots_no_doable: u64,
    pub scalar_assignment_required_remaining: u64,
    pub generation_ms: u64,
    pub evaluation_ms: u64,
    pub moves_per_second: u64,
    pub acceptance_rate: f64,
    pub phase: Option<PhaseTelemetryDto>,
    pub selector_telemetry: Vec<SelectorTelemetryDto>,
    pub move_telemetry: Vec<MoveTelemetryDto>,
    pub applied_move_trace: Vec<AppliedMoveTelemetryDto>,
}

impl TelemetryDto {
    pub fn from_runtime(telemetry: &SolverTelemetry) -> Self {
        Self {
            elapsed_ms: duration_to_millis(telemetry.elapsed),
            step_count: telemetry.step_count,
            moves_generated: telemetry.moves_generated,
            moves_evaluated: telemetry.moves_evaluated,
            moves_accepted: telemetry.moves_accepted,
            moves_applied: telemetry.moves_applied,
            moves_score_improving: telemetry.moves_score_improving,
            moves_applied_improving: telemetry.moves_applied_improving,
            moves_not_doable: telemetry.moves_not_doable,
            moves_acceptor_rejected: telemetry.moves_acceptor_rejected,
            moves_forager_ignored: telemetry.moves_forager_ignored,
            moves_hard_improving: telemetry.moves_hard_improving,
            moves_hard_neutral: telemetry.moves_hard_neutral,
            moves_hard_worse: telemetry.moves_hard_worse,
            conflict_repair_provider_generated: telemetry.conflict_repair_provider_generated,
            conflict_repair_duplicate_filtered: telemetry.conflict_repair_duplicate_filtered,
            conflict_repair_illegal_filtered: telemetry.conflict_repair_illegal_filtered,
            conflict_repair_not_doable_filtered: telemetry.conflict_repair_not_doable_filtered,
            conflict_repair_hard_improving: telemetry.conflict_repair_hard_improving,
            conflict_repair_exposed: telemetry.conflict_repair_exposed,
            score_calculations: telemetry.score_calculations,
            construction_slots_assigned: telemetry.construction_slots_assigned,
            construction_slots_kept: telemetry.construction_slots_kept,
            construction_slots_no_doable: telemetry.construction_slots_no_doable,
            scalar_assignment_required_remaining: telemetry.scalar_assignment_required_remaining,
            generation_ms: duration_to_millis(telemetry.generation_time),
            evaluation_ms: duration_to_millis(telemetry.evaluation_time),
            moves_per_second: whole_units_per_second(telemetry.moves_evaluated, telemetry.elapsed),
            acceptance_rate: derive_acceptance_rate(
                telemetry.moves_accepted,
                telemetry.moves_evaluated,
            ),
            phase: telemetry
                .phase
                .as_ref()
                .map(PhaseTelemetryDto::from_runtime),
            selector_telemetry: telemetry
                .selector_telemetry
                .iter()
                .map(SelectorTelemetryDto::from_runtime)
                .collect(),
            move_telemetry: telemetry
                .move_telemetry
                .iter()
                .map(MoveTelemetryDto::from_runtime)
                .collect(),
            applied_move_trace: telemetry
                .applied_move_trace
                .iter()
                .map(AppliedMoveTelemetryDto::from_runtime)
                .collect(),
        }
    }
}

impl SelectorTelemetryDto {
    fn from_runtime(telemetry: &SelectorTelemetry) -> Self {
        Self {
            selector_index: telemetry.selector_index,
            selector_label: telemetry.selector_label.clone(),
            moves_generated: telemetry.moves_generated,
            moves_evaluated: telemetry.moves_evaluated,
            moves_accepted: telemetry.moves_accepted,
            moves_applied: telemetry.moves_applied,
            moves_not_doable: telemetry.moves_not_doable,
            moves_acceptor_rejected: telemetry.moves_acceptor_rejected,
            moves_forager_ignored: telemetry.moves_forager_ignored,
            moves_hard_improving: telemetry.moves_hard_improving,
            moves_hard_neutral: telemetry.moves_hard_neutral,
            moves_hard_worse: telemetry.moves_hard_worse,
            conflict_repair_provider_generated: telemetry.conflict_repair_provider_generated,
            conflict_repair_duplicate_filtered: telemetry.conflict_repair_duplicate_filtered,
            conflict_repair_illegal_filtered: telemetry.conflict_repair_illegal_filtered,
            conflict_repair_not_doable_filtered: telemetry.conflict_repair_not_doable_filtered,
            conflict_repair_hard_improving: telemetry.conflict_repair_hard_improving,
            conflict_repair_exposed: telemetry.conflict_repair_exposed,
            generation_ms: duration_to_millis(telemetry.generation_time),
            evaluation_ms: duration_to_millis(telemetry.evaluation_time),
        }
    }
}

impl MoveTelemetryDto {
    fn from_runtime(telemetry: &MoveTelemetry) -> Self {
        Self {
            move_label: telemetry.move_label.clone(),
            moves_generated: telemetry.moves_generated,
            moves_evaluated: telemetry.moves_evaluated,
            moves_accepted: telemetry.moves_accepted,
            moves_applied: telemetry.moves_applied,
            moves_not_doable: telemetry.moves_not_doable,
            moves_acceptor_rejected: telemetry.moves_acceptor_rejected,
            moves_forager_ignored: telemetry.moves_forager_ignored,
            moves_score_improving: telemetry.moves_score_improving,
            moves_applied_improving: telemetry.moves_applied_improving,
            moves_score_equal: telemetry.moves_score_equal,
            moves_score_worse: telemetry.moves_score_worse,
            moves_rejected_improving: telemetry.moves_rejected_improving,
            applied_score_improvement: telemetry.applied_score_improvement,
        }
    }
}

impl PhaseTelemetryDto {
    fn from_runtime(telemetry: &PhaseTelemetry) -> Self {
        Self {
            phase_index: telemetry.phase_index,
            phase_type: telemetry.phase_type.clone(),
            elapsed_ms: duration_to_millis(telemetry.elapsed),
            step_count: telemetry.step_count,
            moves_generated: telemetry.moves_generated,
            moves_evaluated: telemetry.moves_evaluated,
            moves_accepted: telemetry.moves_accepted,
            moves_applied: telemetry.moves_applied,
            moves_score_improving: telemetry.moves_score_improving,
            moves_applied_improving: telemetry.moves_applied_improving,
            score_calculations: telemetry.score_calculations,
            generation_ms: duration_to_millis(telemetry.generation_time),
            evaluation_ms: duration_to_millis(telemetry.evaluation_time),
        }
    }
}

impl AppliedMoveTelemetryDto {
    fn from_runtime(telemetry: &AppliedMoveTelemetry) -> Self {
        Self {
            step_index: telemetry.step_index,
            move_label: telemetry.move_label.to_string(),
            selected_candidate_index: telemetry.selected_candidate_index,
            moves_generated_this_step: telemetry.moves_generated_this_step,
            moves_evaluated_this_step: telemetry.moves_evaluated_this_step,
            moves_accepted_this_step: telemetry.moves_accepted_this_step,
            moves_forager_ignored_this_step: telemetry.moves_forager_ignored_this_step,
            score_before: telemetry.score_before,
            score_after: telemetry.score_after,
            score_delta: telemetry.score_delta,
            hard_feasible_before: telemetry.hard_feasible_before,
            hard_feasible_after: telemetry.hard_feasible_after,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CandidateTraceDigestDto {
    pub first_hex: String,
    pub second_hex: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CandidateTracePhaseAttributeDto {
    pub key: String,
    pub value: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CandidateTracePhasePlanDto {
    pub kind: String,
    pub attributes: Vec<CandidateTracePhaseAttributeDto>,
    pub opaque: bool,
    pub children: Vec<CandidateTracePhasePlanDto>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CandidateTraceExecutionPolicyDto {
    pub kind: String,
    pub attributes: Vec<CandidateTracePhaseAttributeDto>,
    pub opaque: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CandidateTraceInputProvenanceDto {
    pub schema_digest_sha256: String,
    pub instance_digest_sha256: String,
    pub initial_state_digest_sha256: String,
    pub core_tree_digest_sha256: Option<String>,
    pub build_digest_sha256: Option<String>,
    pub attestation_producer: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CandidateTraceHeaderDto {
    pub format_version: u32,
    pub configured_input: String,
    pub configured_input_digest: CandidateTraceDigestDto,
    pub execution_policy: CandidateTraceExecutionPolicyDto,
    pub execution_policy_digest: CandidateTraceDigestDto,
    pub execution_policy_complete: bool,
    pub input_provenance: Option<CandidateTraceInputProvenanceDto>,
    pub input_provenance_digest: Option<CandidateTraceDigestDto>,
    pub qualified_run_provenance: Option<CandidateTraceInputProvenanceDto>,
    pub resolved_phase_plan: CandidateTracePhasePlanDto,
    pub resolved_phase_plan_digest: CandidateTraceDigestDto,
    pub resolved_phase_plan_complete: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CandidateTraceProvenanceStatusDto {
    pub execution_policy_complete: bool,
    pub resolved_phase_plan_complete: bool,
    pub input_provenance: &'static str,
    pub qualification: &'static str,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CandidateTraceConstructionTargetDto {
    pub descriptor_index: usize,
    pub entity_index: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(
    tag = "kind",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
pub enum CandidateTraceCoordinateDto {
    Unsigned { value: u64 },
    Absent,
    Text { value: String },
    Bytes { value_hex: String },
}

#[derive(Debug, Clone, Serialize)]
#[serde(
    tag = "kind",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
pub enum CandidateTraceIdentityDto {
    Operation {
        descriptor_index: usize,
        variable_name: Option<String>,
        operation: String,
        components: Vec<CandidateTraceCoordinateDto>,
    },
    Composite {
        operation: String,
        children: Vec<CandidateTraceIdentityDto>,
    },
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CandidatePullTelemetryDto {
    pub ordinal: u64,
    pub source: &'static str,
    pub phase_index: usize,
    pub phase_type: String,
    pub step_index: u64,
    pub selector_index: Option<usize>,
    pub candidate_index: usize,
    pub construction_target: Option<CandidateTraceConstructionTargetDto>,
    pub identity: Option<CandidateTraceIdentityDto>,
    pub dispositions: Vec<&'static str>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CandidateTraceDto {
    pub header: CandidateTraceHeaderDto,
    pub max_entries: usize,
    pub total_pulls: u64,
    pub pulls: Vec<CandidatePullTelemetryDto>,
    pub truncated: bool,
    pub prefix_digest: CandidateTraceDigestDto,
    pub unencoded_identity_count: u64,
    pub complete: bool,
    pub complete_execution_provenance: bool,
    pub provenance_status: CandidateTraceProvenanceStatusDto,
}

impl CandidateTraceDto {
    pub fn from_runtime(trace: &CandidateTraceTelemetry) -> Self {
        let provenance = trace.provenance_status();
        Self {
            header: CandidateTraceHeaderDto::from_runtime(&trace.header),
            max_entries: trace.max_entries,
            total_pulls: trace.total_pulls,
            pulls: trace
                .pulls
                .iter()
                .map(CandidatePullTelemetryDto::from_runtime)
                .collect(),
            truncated: trace.truncated,
            prefix_digest: CandidateTraceDigestDto::from_runtime(trace.prefix_digest),
            unencoded_identity_count: trace.unencoded_identity_count,
            complete: trace.is_complete(),
            complete_execution_provenance: trace.has_complete_execution_provenance(),
            provenance_status: CandidateTraceProvenanceStatusDto {
                execution_policy_complete: provenance.execution_policy_complete,
                resolved_phase_plan_complete: provenance.resolved_phase_plan_complete,
                input_provenance: input_provenance_status_label(provenance.input_provenance),
                qualification: qualification_status_label(provenance.qualification),
            },
        }
    }
}

impl CandidateTraceHeaderDto {
    fn from_runtime(header: &CandidateTraceHeader) -> Self {
        Self {
            format_version: header.format_version,
            configured_input: header.configured_input.clone(),
            configured_input_digest: CandidateTraceDigestDto::from_runtime(
                header.configured_input_digest,
            ),
            execution_policy: CandidateTraceExecutionPolicyDto::from_runtime(
                &header.execution_policy,
            ),
            execution_policy_digest: CandidateTraceDigestDto::from_runtime(
                header.execution_policy_digest,
            ),
            execution_policy_complete: header.execution_policy_complete,
            input_provenance: header
                .input_provenance
                .as_ref()
                .map(CandidateTraceInputProvenanceDto::from_runtime),
            input_provenance_digest: header
                .input_provenance_digest
                .map(CandidateTraceDigestDto::from_runtime),
            qualified_run_provenance: header.qualified_run_provenance.as_ref().map(|qualified| {
                CandidateTraceInputProvenanceDto::from_runtime(qualified.input_provenance())
            }),
            resolved_phase_plan: CandidateTracePhasePlanDto::from_runtime(
                &header.resolved_phase_plan,
            ),
            resolved_phase_plan_digest: CandidateTraceDigestDto::from_runtime(
                header.resolved_phase_plan_digest,
            ),
            resolved_phase_plan_complete: header.resolved_phase_plan_complete,
        }
    }
}

impl CandidateTraceDigestDto {
    fn from_runtime(digest: CandidateTraceDigest) -> Self {
        Self {
            first_hex: format!("{:016x}", digest.first),
            second_hex: format!("{:016x}", digest.second),
        }
    }
}

impl CandidateTraceExecutionPolicyDto {
    fn from_runtime(policy: &CandidateTraceExecutionPolicy) -> Self {
        Self {
            kind: policy.kind.clone(),
            attributes: policy
                .attributes
                .iter()
                .map(CandidateTracePhaseAttributeDto::from_runtime)
                .collect(),
            opaque: policy.opaque,
        }
    }
}

impl CandidateTracePhasePlanDto {
    fn from_runtime(plan: &CandidateTracePhasePlan) -> Self {
        Self {
            kind: plan.kind.clone(),
            attributes: plan
                .attributes
                .iter()
                .map(CandidateTracePhaseAttributeDto::from_runtime)
                .collect(),
            opaque: plan.opaque,
            children: plan
                .children
                .iter()
                .map(CandidateTracePhasePlanDto::from_runtime)
                .collect(),
        }
    }
}

impl CandidateTracePhaseAttributeDto {
    fn from_runtime(attribute: &CandidateTracePhaseAttribute) -> Self {
        Self {
            key: attribute.key.clone(),
            value: attribute.value.clone(),
        }
    }
}

impl CandidateTraceInputProvenanceDto {
    fn from_runtime(provenance: &CandidateTraceInputProvenance) -> Self {
        Self {
            schema_digest_sha256: external_digest_hex(provenance.schema_digest),
            instance_digest_sha256: external_digest_hex(provenance.instance_digest),
            initial_state_digest_sha256: external_digest_hex(provenance.initial_state_digest),
            core_tree_digest_sha256: provenance.core_tree_digest.map(external_digest_hex),
            build_digest_sha256: provenance.build_digest.map(external_digest_hex),
            attestation_producer: provenance.attestation.external_producer().to_string(),
        }
    }
}

impl CandidatePullTelemetryDto {
    fn from_runtime(pull: &CandidatePullTelemetry) -> Self {
        Self {
            ordinal: pull.ordinal,
            source: candidate_trace_source_label(pull.source),
            phase_index: pull.phase_index,
            phase_type: pull.phase_type.clone(),
            step_index: pull.step_index,
            selector_index: pull.selector_index,
            candidate_index: pull.candidate_index,
            construction_target: pull.construction_target.map(|target| {
                CandidateTraceConstructionTargetDto {
                    descriptor_index: target.descriptor_index,
                    entity_index: target.entity_index,
                }
            }),
            identity: pull
                .identity
                .as_ref()
                .map(CandidateTraceIdentityDto::from_runtime),
            dispositions: pull
                .dispositions
                .iter()
                .copied()
                .map(candidate_trace_disposition_label)
                .collect(),
        }
    }
}

impl CandidateTraceIdentityDto {
    fn from_runtime(identity: &CandidateTraceIdentity) -> Self {
        match identity {
            CandidateTraceIdentity::Operation(operation) => Self::Operation {
                descriptor_index: operation.descriptor_index,
                variable_name: operation.variable_name.clone(),
                operation: operation.operation.clone(),
                components: operation
                    .components
                    .iter()
                    .map(CandidateTraceCoordinateDto::from_runtime)
                    .collect(),
            },
            CandidateTraceIdentity::Composite(composite) => Self::Composite {
                operation: composite.operation.clone(),
                children: composite
                    .children
                    .iter()
                    .map(CandidateTraceIdentityDto::from_runtime)
                    .collect(),
            },
        }
    }
}

impl CandidateTraceCoordinateDto {
    fn from_runtime(coordinate: &CandidateTraceCoordinate) -> Self {
        match coordinate {
            CandidateTraceCoordinate::Unsigned(value) => Self::Unsigned { value: *value },
            CandidateTraceCoordinate::Absent => Self::Absent,
            CandidateTraceCoordinate::Text(value) => Self::Text {
                value: value.clone(),
            },
            CandidateTraceCoordinate::Bytes(value) => Self::Bytes {
                value_hex: bytes_hex(value),
            },
        }
    }
}

fn candidate_trace_source_label(source: CandidateTraceSource) -> &'static str {
    match source {
        CandidateTraceSource::Construction => "construction",
        CandidateTraceSource::LocalSearch => "local_search",
        CandidateTraceSource::VariableNeighborhoodDescent => "variable_neighborhood_descent",
        CandidateTraceSource::KOpt => "k_opt",
        CandidateTraceSource::ListRoundRobinConstruction => "list_round_robin_construction",
        CandidateTraceSource::ListCheapestInsertionTrial => "list_cheapest_insertion_trial",
        CandidateTraceSource::ListRegretInsertionTrial => "list_regret_insertion_trial",
        CandidateTraceSource::ListClarkeWrightSavings => "list_clarke_wright_savings",
        CandidateTraceSource::ListClarkeWrightMerge => "list_clarke_wright_merge",
        CandidateTraceSource::ListClarkeWrightCompletionInsertion => {
            "list_clarke_wright_completion_insertion"
        }
        CandidateTraceSource::ListKOptReconnection => "list_k_opt_reconnection",
        CandidateTraceSource::ListRegretOwnerAppend => "list_regret_owner_append",
    }
}

fn candidate_trace_disposition_label(disposition: CandidateTraceDisposition) -> &'static str {
    match disposition {
        CandidateTraceDisposition::InterruptedBeforeEvaluation => "interrupted_before_evaluation",
        CandidateTraceDisposition::Evaluated => "evaluated",
        CandidateTraceDisposition::NotDoable => "not_doable",
        CandidateTraceDisposition::RejectedByHardImprovement => "rejected_by_hard_improvement",
        CandidateTraceDisposition::RejectedByScoreImprovement => "rejected_by_score_improvement",
        CandidateTraceDisposition::AcceptorRejected => "acceptor_rejected",
        CandidateTraceDisposition::ForagerIgnored => "forager_ignored",
        CandidateTraceDisposition::Selected => "selected",
        CandidateTraceDisposition::Applied => "applied",
    }
}

fn input_provenance_status_label(status: CandidateTraceInputProvenanceStatus) -> &'static str {
    match status {
        CandidateTraceInputProvenanceStatus::Absent => "absent",
        CandidateTraceInputProvenanceStatus::ExternallyAttested => "externally_attested",
    }
}

fn qualification_status_label(status: CandidateTraceQualificationStatus) -> &'static str {
    match status {
        CandidateTraceQualificationStatus::NotRequested => "not_requested",
        CandidateTraceQualificationStatus::Qualified => "qualified",
    }
}

fn external_digest_hex(digest: CandidateTraceExternalDigest) -> String {
    bytes_hex(&digest.bytes)
}

fn bytes_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0x0f) as usize] as char);
    }
    out
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
