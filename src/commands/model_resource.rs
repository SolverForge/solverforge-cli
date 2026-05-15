use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use crate::app_spec::{
    self, ConflictRepairSpec, ScalarGroupAssignmentHooks, ScalarGroupLimitsSpec, ScalarGroupSpec,
    ScalarGroupTargetSpec,
};
use crate::commands::generate_constraint::domain::DomainModel;
use crate::commands::generate_constraint::{parse_domain, validate_name};
use crate::commands::generate_domain::find_file_for_type;
use crate::error::{CliError, CliResult};
use crate::managed_block;
use crate::model_contract::{self, ScalarTargetRef};
use crate::model_id::{ConflictRepairId, ScalarGroupId};
use crate::output;
use crate::solver_config;

const SCALAR_GROUPS_PATH: &str = "scalar_groups";
const CONFLICT_REPAIRS_PATH: &str = "conflict_repairs";
const SCALAR_GROUPS_BLOCK: &str = "scalar-groups";
const SCALAR_GROUP_HOOK_STUBS_BLOCK: &str = "scalar-group-hook-stubs";
const CONFLICT_REPAIRS_BLOCK: &str = "conflict-repairs";
const CONFLICT_REPAIR_HOOK_STUBS_BLOCK: &str = "conflict-repair-hook-stubs";

#[derive(Debug, Clone)]
pub(crate) struct ScalarGroupRequest {
    pub name: String,
    pub assignment: Option<String>,
    pub candidates: Option<String>,
    pub targets: Vec<String>,
    pub required_entity: Option<String>,
    pub capacity_key: Option<String>,
    pub assignment_rule: Option<String>,
    pub position_key: Option<String>,
    pub sequence_key: Option<String>,
    pub entity_order: Option<String>,
    pub value_order: Option<String>,
    pub value_candidate_limit: Option<usize>,
    pub group_candidate_limit: Option<usize>,
    pub max_moves_per_step: Option<usize>,
    pub max_augmenting_depth: Option<usize>,
    pub max_rematch_size: Option<usize>,
    pub skip_solver_config: bool,
}

#[derive(Debug, Clone)]
pub(crate) struct ConflictRepairRequest {
    pub constraint: String,
    pub provider: String,
    pub selector: String,
    pub max_matches_per_step: Option<usize>,
    pub max_repairs_per_match: Option<usize>,
    pub max_moves_per_step: Option<usize>,
    pub include_soft_matches: bool,
    pub skip_solver_config: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ScalarGroupKind {
    Assignment,
    Candidates,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum HookStubKind {
    RequiredEntity,
    CapacityKey,
    AssignmentRule,
    PositionKey,
    SequenceKey,
    EntityOrder,
    ValueOrder,
    CandidateProvider,
    RepairProvider,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct HookStub {
    name: String,
    kind: HookStubKind,
}

pub(crate) fn run_scalar_group(request: ScalarGroupRequest) -> CliResult {
    let group_id = ScalarGroupId::parse(&request.name)?;
    validate_name(group_id.as_str())?;
    validate_scalar_group_request(&request)?;

    let domain = parse_domain().map_err(CliError::general)?;
    let solution_file = solution_file(&domain)?;
    let mut spec = sync_and_load_app_spec()?;

    if spec
        .scalar_groups
        .iter()
        .any(|group| group.name == request.name)
    {
        return Err(CliError::ResourceExists {
            kind: "scalar group",
            name: request.name,
        });
    }

    let group = build_scalar_group_spec(&domain, request)?;
    spec.scalar_groups.push(group.clone());

    let solution_src = fs::read_to_string(&solution_file).map_err(|err| CliError::IoError {
        context: format!("failed to read {}", solution_file.display()),
        source: err,
    })?;
    let solution_src =
        ensure_solution_attribute_path(&solution_src, "scalar_groups", SCALAR_GROUPS_PATH)?;
    let solution_src = ensure_scalar_group_stubs(&solution_src, &domain.solution_type, &group)?;
    let solution_src = write_managed_block(
        &solution_src,
        SCALAR_GROUPS_BLOCK,
        &render_scalar_groups(&domain.solution_type, &spec.scalar_groups),
    )?;
    syn::parse_file(&solution_src).map_err(|err| {
        CliError::general(format!(
            "generating scalar group '{}' would leave invalid Rust in {}: {err}",
            group.name,
            solution_file.display()
        ))
    })?;
    let solver_config_plan = if group.solver_config {
        Some(solver_config::SolverConfigDocument::load()?.plan_sync_model_resources(&spec)?)
    } else {
        None
    };
    let app_spec_raw = serialize_app_spec(&spec)?;

    fs::write(&solution_file, solution_src).map_err(|err| CliError::IoError {
        context: format!("failed to write {}", solution_file.display()),
        source: err,
    })?;
    if let Some(doc) = &solver_config_plan {
        doc.write()?;
    }
    write_app_spec_raw(&app_spec_raw)?;
    app_spec::sync_from_project()?;

    output::print_update(&output::display_path(&solution_file));
    if group.solver_config {
        output::print_update(solver_config::CONFIG_PATH);
    }
    output::print_update("solverforge.app.toml");
    Ok(())
}

pub(crate) fn run_conflict_repair(request: ConflictRepairRequest) -> CliResult {
    validate_conflict_repair_request(&request)?;
    let constraint_ref = model_contract::resolve_constraint_ref(&request.constraint)?;

    let domain = parse_domain().map_err(CliError::general)?;
    let solution_file = solution_file(&domain)?;
    let mut spec = sync_and_load_app_spec()?;

    if spec
        .conflict_repairs
        .iter()
        .any(|repair| repair.constraint == constraint_ref.id.as_str())
    {
        return Err(CliError::ResourceExists {
            kind: "conflict repair",
            name: constraint_ref.id.clone(),
        });
    }

    let repair = ConflictRepairSpec {
        constraint: constraint_ref.id,
        provider: request.provider,
        selector: request.selector,
        max_matches_per_step: request.max_matches_per_step,
        max_repairs_per_match: request.max_repairs_per_match,
        max_moves_per_step: request.max_moves_per_step,
        include_soft_matches: request.include_soft_matches,
        solver_config: !request.skip_solver_config,
        enabled: true,
    };
    spec.conflict_repairs.push(repair.clone());

    let solution_src = fs::read_to_string(&solution_file).map_err(|err| CliError::IoError {
        context: format!("failed to read {}", solution_file.display()),
        source: err,
    })?;
    let solution_src =
        ensure_solution_attribute_path(&solution_src, "conflict_repairs", CONFLICT_REPAIRS_PATH)?;
    let solution_src = ensure_conflict_repair_stubs(&solution_src, &domain.solution_type, &repair)?;
    let solution_src = write_managed_block(
        &solution_src,
        CONFLICT_REPAIRS_BLOCK,
        &render_conflict_repairs(&domain.solution_type, &spec.conflict_repairs),
    )?;
    syn::parse_file(&solution_src).map_err(|err| {
        CliError::general(format!(
            "generating conflict repair '{}' would leave invalid Rust in {}: {err}",
            repair.constraint,
            solution_file.display()
        ))
    })?;
    let solver_config_plan = if repair.solver_config {
        Some(solver_config::SolverConfigDocument::load()?.plan_sync_model_resources(&spec)?)
    } else {
        None
    };
    let app_spec_raw = serialize_app_spec(&spec)?;

    fs::write(&solution_file, solution_src).map_err(|err| CliError::IoError {
        context: format!("failed to write {}", solution_file.display()),
        source: err,
    })?;
    if let Some(doc) = &solver_config_plan {
        doc.write()?;
    }
    write_app_spec_raw(&app_spec_raw)?;
    app_spec::sync_from_project()?;

    output::print_update(&output::display_path(&solution_file));
    if repair.solver_config {
        output::print_update(solver_config::CONFIG_PATH);
    }
    output::print_update("solverforge.app.toml");
    Ok(())
}

pub(crate) fn destroy_scalar_group(name: &str) -> CliResult {
    let group_id = ScalarGroupId::parse(name)?;
    let domain = parse_domain().map_err(CliError::general)?;
    let solution_file = solution_file(&domain)?;
    let mut spec = app_spec::load()?;
    let Some(index) = spec
        .scalar_groups
        .iter()
        .position(|group| group.name == group_id.as_str())
    else {
        return Err(CliError::ResourceNotFound {
            kind: "scalar group",
            name: name.to_string(),
        });
    };
    let removed = spec.scalar_groups.remove(index);

    let solution_src = fs::read_to_string(&solution_file).map_err(|err| CliError::IoError {
        context: format!("failed to read {}", solution_file.display()),
        source: err,
    })?;
    let solution_src = if spec.scalar_groups.is_empty() {
        remove_solution_attribute_path(&solution_src, "scalar_groups", SCALAR_GROUPS_PATH)?
    } else {
        solution_src
    };
    let solution_src = write_managed_block(
        &solution_src,
        SCALAR_GROUPS_BLOCK,
        &render_scalar_groups(&domain.solution_type, &spec.scalar_groups),
    )?;
    let removable_stubs = removable_hook_stubs(
        scalar_group_hook_stubs(&removed, &domain.solution_type),
        scalar_group_hook_stubs_for_all(&spec.scalar_groups, &domain.solution_type),
    );
    let solution_src = remove_hook_stubs(
        &solution_src,
        SCALAR_GROUP_HOOK_STUBS_BLOCK,
        &removable_stubs,
    )?;
    syn::parse_file(&solution_src).map_err(|err| {
        CliError::general(format!(
            "destroying scalar group '{}' would leave invalid Rust in {}: {err}",
            removed.name,
            solution_file.display()
        ))
    })?;
    let solver_config_plan = solver_config::SolverConfigDocument::load_if_present()?
        .map(|doc| {
            doc.ensure_no_user_scalar_group_refs(&removed.name)?;
            doc.plan_sync_model_resources(&spec)
        })
        .transpose()?;
    let app_spec_raw = serialize_app_spec(&spec)?;

    fs::write(&solution_file, solution_src).map_err(|err| CliError::IoError {
        context: format!("failed to write {}", solution_file.display()),
        source: err,
    })?;
    if let Some(doc) = &solver_config_plan {
        doc.write()?;
    }
    write_app_spec_raw(&app_spec_raw)?;
    app_spec::sync_from_project()?;

    output::print_remove(&format!("scalar group {}", removed.name));
    output::print_update(&output::display_path(&solution_file));
    output::print_update(solver_config::CONFIG_PATH);
    output::print_update("solverforge.app.toml");
    Ok(())
}

pub(crate) fn destroy_conflict_repair(name: &str) -> CliResult {
    let repair_id = ConflictRepairId::parse(name)?;
    let domain = parse_domain().map_err(CliError::general)?;
    let solution_file = solution_file(&domain)?;
    let mut spec = app_spec::load()?;
    let Some(index) = spec
        .conflict_repairs
        .iter()
        .position(|repair| repair.constraint == repair_id.as_str())
    else {
        return Err(CliError::ResourceNotFound {
            kind: "conflict repair",
            name: name.to_string(),
        });
    };
    let removed = spec.conflict_repairs.remove(index);

    let solution_src = fs::read_to_string(&solution_file).map_err(|err| CliError::IoError {
        context: format!("failed to read {}", solution_file.display()),
        source: err,
    })?;
    let solution_src = if spec.conflict_repairs.is_empty() {
        remove_solution_attribute_path(&solution_src, "conflict_repairs", CONFLICT_REPAIRS_PATH)?
    } else {
        solution_src
    };
    let solution_src = write_managed_block(
        &solution_src,
        CONFLICT_REPAIRS_BLOCK,
        &render_conflict_repairs(&domain.solution_type, &spec.conflict_repairs),
    )?;
    let removable_stubs = removable_hook_stubs(
        conflict_repair_hook_stubs(&removed, &domain.solution_type),
        conflict_repair_hook_stubs_for_all(&spec.conflict_repairs, &domain.solution_type),
    );
    let solution_src = remove_hook_stubs(
        &solution_src,
        CONFLICT_REPAIR_HOOK_STUBS_BLOCK,
        &removable_stubs,
    )?;
    syn::parse_file(&solution_src).map_err(|err| {
        CliError::general(format!(
            "destroying conflict repair '{}' would leave invalid Rust in {}: {err}",
            removed.constraint,
            solution_file.display()
        ))
    })?;
    let solver_config_plan = solver_config::SolverConfigDocument::load_if_present()?
        .map(|doc| {
            doc.ensure_no_user_conflict_repair_refs(&removed.constraint)?;
            doc.plan_sync_model_resources(&spec)
        })
        .transpose()?;
    let app_spec_raw = serialize_app_spec(&spec)?;

    fs::write(&solution_file, solution_src).map_err(|err| CliError::IoError {
        context: format!("failed to write {}", solution_file.display()),
        source: err,
    })?;
    if let Some(doc) = &solver_config_plan {
        doc.write()?;
    }
    write_app_spec_raw(&app_spec_raw)?;
    app_spec::sync_from_project()?;

    output::print_remove(&format!("conflict repair {}", removed.constraint));
    output::print_update(&output::display_path(&solution_file));
    output::print_update(solver_config::CONFIG_PATH);
    output::print_update("solverforge.app.toml");
    Ok(())
}

fn validate_scalar_group_request(request: &ScalarGroupRequest) -> CliResult {
    let has_assignment = request.assignment.is_some();
    let has_candidates = request.candidates.is_some();
    match (has_assignment, has_candidates) {
        (true, false) => {
            if !request.targets.is_empty() {
                return Err(CliError::general(
                    "assignment scalar groups use --assignment, not --target",
                ));
            }
        }
        (false, true) => {
            if request.targets.is_empty() {
                return Err(CliError::general(
                    "candidate scalar groups require at least one --target Entity.field",
                ));
            }
            if request.has_assignment_hooks() {
                return Err(CliError::general(concat!(
                    "candidate scalar groups do not accept assignment hook flags: ",
                    "--required-entity, --capacity-key, --assignment-rule, --position-key, ",
                    "--sequence-key, --entity-order, --value-order"
                )));
            }
        }
        _ => {
            return Err(CliError::general(
                "scalar groups require exactly one of --assignment or --candidates",
            ));
        }
    }

    validate_optional_path(request.candidates.as_deref(), "--candidates")?;
    validate_optional_path(request.required_entity.as_deref(), "--required-entity")?;
    validate_optional_path(request.capacity_key.as_deref(), "--capacity-key")?;
    validate_optional_path(request.assignment_rule.as_deref(), "--assignment-rule")?;
    validate_optional_path(request.position_key.as_deref(), "--position-key")?;
    validate_optional_path(request.sequence_key.as_deref(), "--sequence-key")?;
    validate_optional_path(request.entity_order.as_deref(), "--entity-order")?;
    validate_optional_path(request.value_order.as_deref(), "--value-order")?;
    if request.assignment_rule.is_some() && request.sequence_key.is_none() {
        return Err(CliError::general(
            "assignment scalar groups require --sequence-key when --assignment-rule is set",
        ));
    }
    Ok(())
}

impl ScalarGroupRequest {
    fn has_assignment_hooks(&self) -> bool {
        self.required_entity.is_some()
            || self.capacity_key.is_some()
            || self.assignment_rule.is_some()
            || self.position_key.is_some()
            || self.sequence_key.is_some()
            || self.entity_order.is_some()
            || self.value_order.is_some()
    }
}

fn validate_conflict_repair_request(request: &ConflictRepairRequest) -> CliResult {
    ConflictRepairId::parse(&request.constraint)?;
    validate_required_path(&request.provider, "--provider")?;
    match request.selector.as_str() {
        "compound" | "conflict" => Ok(()),
        _ => Err(CliError::general(
            "valid conflict repair selectors are: compound, conflict",
        )),
    }
}

fn build_scalar_group_spec(
    domain: &DomainModel,
    request: ScalarGroupRequest,
) -> CliResult<ScalarGroupSpec> {
    let (kind, targets) = if let Some(target) = request.assignment {
        let target = resolve_scalar_target(domain, &target)?;
        if !target.allows_unassigned {
            return Err(CliError::general(format!(
                "assignment scalar group target '{}.{}' must allow unassigned values",
                target.entity_type, target.field
            )));
        }
        (
            ScalarGroupKind::Assignment,
            vec![scalar_group_target_spec(&target)],
        )
    } else {
        let targets = request
            .targets
            .iter()
            .map(|target| {
                resolve_scalar_target(domain, target)
                    .map(|resolved| scalar_group_target_spec(&resolved))
            })
            .collect::<CliResult<Vec<_>>>()?;
        (ScalarGroupKind::Candidates, targets)
    };

    let assignment_hooks = match kind {
        ScalarGroupKind::Assignment => ScalarGroupAssignmentHooks {
            required_entity: request.required_entity.unwrap_or_default(),
            capacity_key: request.capacity_key.unwrap_or_default(),
            assignment_rule: request.assignment_rule.unwrap_or_default(),
            position_key: request.position_key.unwrap_or_default(),
            sequence_key: request.sequence_key.unwrap_or_default(),
            entity_order: request.entity_order.unwrap_or_default(),
            value_order: request.value_order.unwrap_or_default(),
        },
        ScalarGroupKind::Candidates => ScalarGroupAssignmentHooks::default(),
    };

    Ok(ScalarGroupSpec {
        name: request.name,
        kind: match kind {
            ScalarGroupKind::Assignment => "assignment".to_string(),
            ScalarGroupKind::Candidates => "candidates".to_string(),
        },
        targets,
        candidate_provider: request.candidates.unwrap_or_default(),
        assignment_hooks,
        limits: ScalarGroupLimitsSpec {
            value_candidate_limit: request.value_candidate_limit,
            group_candidate_limit: request.group_candidate_limit,
            max_moves_per_step: request.max_moves_per_step,
            max_augmenting_depth: request.max_augmenting_depth,
            max_rematch_size: request.max_rematch_size,
        },
        solver_config: !request.skip_solver_config,
        enabled: true,
    })
}

fn resolve_scalar_target(domain: &DomainModel, raw: &str) -> CliResult<ScalarTargetRef> {
    model_contract::resolve_scalar_target(domain, raw)
}

fn scalar_group_target_spec(target: &ScalarTargetRef) -> ScalarGroupTargetSpec {
    ScalarGroupTargetSpec {
        entity: target.entity.clone(),
        entity_plural: target.entity_plural.clone(),
        field: target.field.clone(),
    }
}

fn solution_file(domain: &DomainModel) -> CliResult<PathBuf> {
    find_file_for_type(Path::new("src/domain"), &domain.solution_type).map_err(CliError::general)
}

fn sync_and_load_app_spec() -> CliResult<app_spec::AppSpec> {
    app_spec::sync_from_project()?;
    app_spec::load()
}

fn serialize_app_spec(spec: &app_spec::AppSpec) -> CliResult<String> {
    toml::to_string_pretty(spec).map_err(|err| {
        CliError::general(format!("failed to serialize solverforge.app.toml: {err}"))
    })
}

fn write_app_spec_raw(raw: &str) -> CliResult {
    fs::write("solverforge.app.toml", raw).map_err(|err| CliError::IoError {
        context: "failed to write solverforge.app.toml".to_string(),
        source: err,
    })
}

fn ensure_scalar_group_stubs(
    src: &str,
    solution_type: &str,
    group: &ScalarGroupSpec,
) -> CliResult<String> {
    ensure_hook_stubs(
        src,
        SCALAR_GROUP_HOOK_STUBS_BLOCK,
        solution_type,
        &scalar_group_hook_stubs(group, solution_type),
    )
}

fn ensure_conflict_repair_stubs(
    src: &str,
    solution_type: &str,
    repair: &ConflictRepairSpec,
) -> CliResult<String> {
    ensure_hook_stubs(
        src,
        CONFLICT_REPAIR_HOOK_STUBS_BLOCK,
        solution_type,
        &conflict_repair_hook_stubs(repair, solution_type),
    )
}

fn scalar_group_hook_stubs(group: &ScalarGroupSpec, _solution_type: &str) -> Vec<HookStub> {
    let mut stubs = Vec::new();
    if group.kind == "candidates" {
        push_stub_if_local(
            &mut stubs,
            &group.candidate_provider,
            HookStubKind::CandidateProvider,
        );
    }
    push_stub_if_local(
        &mut stubs,
        &group.assignment_hooks.required_entity,
        HookStubKind::RequiredEntity,
    );
    push_stub_if_local(
        &mut stubs,
        &group.assignment_hooks.capacity_key,
        HookStubKind::CapacityKey,
    );
    push_stub_if_local(
        &mut stubs,
        &group.assignment_hooks.assignment_rule,
        HookStubKind::AssignmentRule,
    );
    push_stub_if_local(
        &mut stubs,
        &group.assignment_hooks.position_key,
        HookStubKind::PositionKey,
    );
    push_stub_if_local(
        &mut stubs,
        &group.assignment_hooks.sequence_key,
        HookStubKind::SequenceKey,
    );
    push_stub_if_local(
        &mut stubs,
        &group.assignment_hooks.entity_order,
        HookStubKind::EntityOrder,
    );
    push_stub_if_local(
        &mut stubs,
        &group.assignment_hooks.value_order,
        HookStubKind::ValueOrder,
    );
    stubs
}

fn conflict_repair_hook_stubs(repair: &ConflictRepairSpec, _solution_type: &str) -> Vec<HookStub> {
    let mut stubs = Vec::new();
    push_stub_if_local(&mut stubs, &repair.provider, HookStubKind::RepairProvider);
    stubs
}

fn scalar_group_hook_stubs_for_all(
    groups: &[ScalarGroupSpec],
    solution_type: &str,
) -> BTreeSet<HookStub> {
    groups
        .iter()
        .flat_map(|group| scalar_group_hook_stubs(group, solution_type))
        .collect()
}

fn conflict_repair_hook_stubs_for_all(
    repairs: &[ConflictRepairSpec],
    solution_type: &str,
) -> BTreeSet<HookStub> {
    repairs
        .iter()
        .flat_map(|repair| conflict_repair_hook_stubs(repair, solution_type))
        .collect()
}

fn removable_hook_stubs(
    removed: Vec<HookStub>,
    remaining_required: BTreeSet<HookStub>,
) -> Vec<HookStub> {
    let mut unique_removed = BTreeSet::new();
    removed
        .into_iter()
        .filter(|stub| !remaining_required.contains(stub) && unique_removed.insert(stub.clone()))
        .collect()
}

fn push_stub_if_local(stubs: &mut Vec<HookStub>, path: &str, kind: HookStubKind) {
    if let Some(name) = local_fn_name(path) {
        stubs.push(HookStub { name, kind });
    }
}

fn ensure_hook_stubs(
    src: &str,
    label: &str,
    solution_type: &str,
    stubs: &[HookStub],
) -> CliResult<String> {
    let mut missing = Vec::new();
    let mut seen = BTreeSet::new();
    for stub in stubs {
        if seen.insert(stub.name.clone()) && !source_declares_fn(src, &stub.name) {
            missing.push(render_hook_stub(stub, solution_type));
        }
    }
    if missing.is_empty() {
        return Ok(src.to_string());
    }

    let existing = managed_block::read_block(src, label).unwrap_or_default();
    let content = if existing.trim().is_empty() {
        missing.join("\n\n")
    } else {
        format!("{}\n\n{}", existing.trim_end(), missing.join("\n\n"))
    };
    write_managed_block(src, label, &content)
}

fn remove_hook_stubs(src: &str, label: &str, stubs: &[HookStub]) -> CliResult<String> {
    let Ok(block) = managed_block::read_block(src, label) else {
        return Ok(src.to_string());
    };
    let mut lines = block.lines().map(ToString::to_string).collect::<Vec<_>>();
    for stub in stubs {
        remove_untouched_stub_lines(&mut lines, stub);
    }
    let content = lines.join("\n");
    write_managed_block(src, label, content.trim())
}

fn remove_untouched_stub_lines(lines: &mut Vec<String>, stub: &HookStub) {
    let Some(start) = lines.iter().position(|line| {
        let trimmed = line.trim_start();
        trimmed.starts_with(&format!("fn {}(", stub.name))
            || trimmed.starts_with(&format!("fn {}<", stub.name))
    }) else {
        return;
    };

    let mut depth = 0isize;
    let mut end = None;
    for (offset, line) in lines[start..].iter().enumerate() {
        for ch in line.chars() {
            match ch {
                '{' => depth += 1,
                '}' => depth -= 1,
                _ => {}
            }
        }
        if depth == 0 && line.contains('}') {
            end = Some(start + offset);
            break;
        }
    }
    let Some(end) = end else {
        return;
    };
    let candidate = lines[start..=end].join("\n");
    if candidate.contains(&format!("todo!(\"implement {}\")", stub.name)) {
        lines.drain(start..=end);
        while start < lines.len() && lines[start].trim().is_empty() {
            lines.remove(start);
        }
    }
}

fn render_hook_stub(stub: &HookStub, solution_type: &str) -> String {
    let name = &stub.name;
    match stub.kind {
        HookStubKind::RequiredEntity => format!(
            "fn {name}(_solution: &{solution_type}, _entity_idx: usize) -> bool {{\n    todo!(\"implement {name}\")\n}}"
        ),
        HookStubKind::CapacityKey => format!(
            "fn {name}(\n    _solution: &{solution_type},\n    _entity_idx: usize,\n    _value_idx: usize,\n) -> Option<usize> {{\n    todo!(\"implement {name}\")\n}}"
        ),
        HookStubKind::AssignmentRule => format!(
            "fn {name}(\n    _solution: &{solution_type},\n    _left_entity_idx: usize,\n    _left_value_idx: usize,\n    _right_entity_idx: usize,\n    _right_value_idx: usize,\n) -> bool {{\n    todo!(\"implement {name}\")\n}}"
        ),
        HookStubKind::PositionKey | HookStubKind::EntityOrder => format!(
            "fn {name}(_solution: &{solution_type}, _entity_idx: usize) -> i64 {{\n    todo!(\"implement {name}\")\n}}"
        ),
        HookStubKind::SequenceKey => format!(
            "fn {name}(\n    _solution: &{solution_type},\n    _entity_idx: usize,\n    _value_idx: usize,\n) -> Option<usize> {{\n    todo!(\"implement {name}\")\n}}"
        ),
        HookStubKind::ValueOrder => format!(
            "fn {name}(\n    _solution: &{solution_type},\n    _entity_idx: usize,\n    _value_idx: usize,\n) -> i64 {{\n    todo!(\"implement {name}\")\n}}"
        ),
        HookStubKind::CandidateProvider => format!(
            "fn {name}(\n    _solution: &{solution_type},\n    _limits: ScalarGroupLimits,\n) -> Vec<ScalarCandidate<{solution_type}>> {{\n    todo!(\"implement {name}\")\n}}"
        ),
        HookStubKind::RepairProvider => format!(
            "fn {name}(\n    _solution: &{solution_type},\n    _limits: RepairLimits,\n) -> Vec<RepairCandidate<{solution_type}>> {{\n    todo!(\"implement {name}\")\n}}"
        ),
    }
}

fn render_scalar_groups(solution_type: &str, groups: &[ScalarGroupSpec]) -> String {
    let entries = groups
        .iter()
        .filter(|group| group.enabled)
        .map(|group| render_scalar_group_entry(solution_type, group))
        .collect::<Vec<_>>();

    if entries.is_empty() {
        return String::new();
    }

    format!(
        "pub(super) fn scalar_groups() -> Vec<ScalarGroup<{solution_type}>> {{\n    vec![\n{}\n    ]\n}}",
        entries.join("\n")
    )
}

fn render_scalar_group_entry(solution_type: &str, group: &ScalarGroupSpec) -> String {
    let mut entry = if group.kind == "assignment" {
        let target = group
            .targets
            .first()
            .expect("assignment scalar group has one target");
        format!(
            "        ScalarGroup::assignment(\n            \"{}\",\n            {solution_type}::{}().scalar(\"{}\"),\n        )",
            group.name, target.entity_plural, target.field
        )
    } else {
        let targets = group
            .targets
            .iter()
            .map(|target| {
                format!(
                    "                {solution_type}::{}().scalar(\"{}\"),",
                    target.entity_plural, target.field
                )
            })
            .collect::<Vec<_>>()
            .join("\n");
        format!(
            "        ScalarGroup::candidates(\n            \"{}\",\n            vec![\n{}\n            ],\n            {},\n        )",
            group.name, targets, group.candidate_provider
        )
    };

    if group.kind == "assignment" {
        for (method, path) in [
            (
                "with_required_entity",
                group.assignment_hooks.required_entity.as_str(),
            ),
            (
                "with_capacity_key",
                group.assignment_hooks.capacity_key.as_str(),
            ),
            (
                "with_assignment_rule",
                group.assignment_hooks.assignment_rule.as_str(),
            ),
            (
                "with_position_key",
                group.assignment_hooks.position_key.as_str(),
            ),
            (
                "with_sequence_key",
                group.assignment_hooks.sequence_key.as_str(),
            ),
            (
                "with_entity_order",
                group.assignment_hooks.entity_order.as_str(),
            ),
            (
                "with_value_order",
                group.assignment_hooks.value_order.as_str(),
            ),
        ] {
            if !path.is_empty() {
                entry.push_str(&format!("\n            .{method}({path})"));
            }
        }
    }
    if scalar_limits_are_set(&group.limits) {
        entry.push_str(&format!(
            "\n            .with_limits({})",
            render_scalar_group_limits(&group.limits, 12)
        ));
    }
    entry.push(',');
    entry
}

fn render_conflict_repairs(solution_type: &str, repairs: &[ConflictRepairSpec]) -> String {
    let entries = repairs
        .iter()
        .filter(|repair| repair.enabled)
        .map(|repair| {
            format!(
                "        ConflictRepair::new(\"{}\", {}),",
                repair.constraint, repair.provider
            )
        })
        .collect::<Vec<_>>();

    if entries.is_empty() {
        return String::new();
    }

    format!(
        "pub(super) fn conflict_repairs() -> Vec<ConflictRepair<{solution_type}>> {{\n    vec![\n{}\n    ]\n}}",
        entries.join("\n")
    )
}

fn render_scalar_group_limits(limits: &ScalarGroupLimitsSpec, indent: usize) -> String {
    let pad = " ".repeat(indent);
    let value = option_usize_literal(limits.value_candidate_limit);
    let group = option_usize_literal(limits.group_candidate_limit);
    let moves = option_usize_literal(limits.max_moves_per_step);
    let depth = option_usize_literal(limits.max_augmenting_depth);
    let rematch = option_usize_literal(limits.max_rematch_size);
    format!(
        "ScalarGroupLimits {{\n{pad}    value_candidate_limit: {value},\n{pad}    group_candidate_limit: {group},\n{pad}    max_moves_per_step: {moves},\n{pad}    max_augmenting_depth: {depth},\n{pad}    max_rematch_size: {rematch},\n{pad}}}"
    )
}

fn scalar_limits_are_set(limits: &ScalarGroupLimitsSpec) -> bool {
    limits.value_candidate_limit.is_some()
        || limits.group_candidate_limit.is_some()
        || limits.max_moves_per_step.is_some()
        || limits.max_augmenting_depth.is_some()
        || limits.max_rematch_size.is_some()
}

fn option_usize_literal(value: Option<usize>) -> String {
    value
        .map(|value| format!("Some({value})"))
        .unwrap_or_else(|| "None".to_string())
}

fn write_managed_block(src: &str, label: &str, content: &str) -> CliResult<String> {
    match managed_block::replace_block(src, label, content) {
        Ok(updated) => Ok(updated),
        Err(_) if content.trim().is_empty() => Ok(src.to_string()),
        Err(_) => {
            let trimmed = src.trim_end();
            Ok(format!(
                "{trimmed}\n\n{}\n{}\n{}\n",
                managed_block::begin_marker(label),
                content.trim_end(),
                managed_block::end_marker(label)
            ))
        }
    }
}

fn ensure_solution_attribute_path(src: &str, key: &str, path: &str) -> CliResult<String> {
    rewrite_solution_attribute_path(src, key, path, true)
}

fn remove_solution_attribute_path(src: &str, key: &str, path: &str) -> CliResult<String> {
    rewrite_solution_attribute_path(src, key, path, false)
}

fn rewrite_solution_attribute_path(
    src: &str,
    key: &str,
    path: &str,
    ensure: bool,
) -> CliResult<String> {
    let mut lines = src.lines().map(ToString::to_string).collect::<Vec<_>>();
    let Some(start) = lines
        .iter()
        .position(|line| line.contains("#[planning_solution("))
    else {
        return Err(CliError::general(
            "solution file is missing #[planning_solution(...)]",
        ));
    };
    let Some(end) = lines[start..]
        .iter()
        .position(|line| line.trim() == ")]")
        .map(|offset| start + offset)
    else {
        return Err(CliError::general(
            "solution #[planning_solution(...)] attribute must be multi-line and end with `)]`",
        ));
    };

    let needle = format!("{key} = ");
    let existing = (start..=end).find(|idx| lines[*idx].trim_start().starts_with(&needle));
    if let Some(existing) = existing {
        let expected = format!("{key} = \"{path}\"");
        if !lines[existing].trim().trim_end_matches(',').eq(&expected) {
            return Err(CliError::general(format!(
                "planning solution already declares a different {key} path"
            )));
        }
        if ensure {
            return Ok(join_lines(lines));
        }
        lines.remove(existing);
        return Ok(join_lines(lines));
    }

    if !ensure {
        return Ok(src.to_string());
    }

    if end > start {
        let last_attr_line = end - 1;
        let trimmed = lines[last_attr_line].trim_end();
        if !trimmed.ends_with(',') && !trimmed.ends_with('(') {
            lines[last_attr_line].push(',');
        }
    }
    lines.insert(end, format!("    {key} = \"{path}\""));
    Ok(join_lines(lines))
}

fn join_lines(lines: Vec<String>) -> String {
    if lines.is_empty() {
        String::new()
    } else {
        lines.join("\n") + "\n"
    }
}

fn validate_required_path(path: &str, flag: &str) -> CliResult {
    validate_optional_path(Some(path), flag)
}

fn validate_optional_path(path: Option<&str>, flag: &str) -> CliResult {
    let Some(path) = path else {
        return Ok(());
    };
    if path.trim().is_empty() {
        return Err(CliError::general(format!("{flag} cannot be empty")));
    }
    syn::parse_str::<syn::Path>(path)
        .map_err(|err| CliError::general(format!("{flag} must be a Rust function path: {err}")))?;
    Ok(())
}

fn local_fn_name(path: &str) -> Option<String> {
    let parsed = syn::parse_str::<syn::Path>(path).ok()?;
    if parsed.segments.len() == 1 {
        Some(parsed.segments.first()?.ident.to_string())
    } else {
        None
    }
}

fn source_declares_fn(src: &str, name: &str) -> bool {
    syn::parse_file(src).ok().is_some_and(|file| {
        file.items.iter().any(|item| match item {
            syn::Item::Fn(item_fn) => item_fn.sig.ident == name,
            _ => false,
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app_spec::{ScalarGroupAssignmentHooks, ScalarGroupLimitsSpec};

    #[test]
    fn solution_attribute_path_is_added_with_comma() {
        let src = r#"#[planning_solution(
    constraints = "crate::constraints::create_constraints",
    solver_toml = "../../solver.toml"
)]
pub struct Plan {}
"#;

        let updated =
            ensure_solution_attribute_path(src, "scalar_groups", "scalar_groups").unwrap();

        assert!(updated.contains("solver_toml = \"../../solver.toml\","));
        assert!(updated.contains("scalar_groups = \"scalar_groups\""));
    }

    #[test]
    fn scalar_group_entry_renders_assignment_hooks_and_limits() {
        let group = ScalarGroupSpec {
            name: "required_assignment".to_string(),
            kind: "assignment".to_string(),
            targets: vec![ScalarGroupTargetSpec {
                entity: "task".to_string(),
                entity_plural: "tasks".to_string(),
                field: "resource_idx".to_string(),
            }],
            assignment_hooks: ScalarGroupAssignmentHooks {
                required_entity: "required_task".to_string(),
                value_order: "resource_priority".to_string(),
                ..ScalarGroupAssignmentHooks::default()
            },
            limits: ScalarGroupLimitsSpec {
                value_candidate_limit: Some(8),
                max_moves_per_step: Some(64),
                ..ScalarGroupLimitsSpec::default()
            },
            solver_config: true,
            enabled: true,
            ..ScalarGroupSpec::default()
        };

        let rendered = render_scalar_group_entry("Plan", &group);

        assert!(rendered.contains("ScalarGroup::assignment("));
        assert!(rendered.contains("Plan::tasks().scalar(\"resource_idx\")"));
        assert!(rendered.contains(".with_required_entity(required_task)"));
        assert!(rendered.contains(".with_value_order(resource_priority)"));
        assert!(rendered.contains("value_candidate_limit: Some(8)"));
        assert!(rendered.contains("max_moves_per_step: Some(64)"));
    }

    #[test]
    fn candidate_scalar_group_rejects_assignment_hook_flags() {
        let request = ScalarGroupRequest {
            name: "candidate_group".to_string(),
            assignment: None,
            candidates: Some("candidate_provider".to_string()),
            targets: vec!["Task.resource_idx".to_string()],
            required_entity: Some("required_task".to_string()),
            capacity_key: None,
            assignment_rule: None,
            position_key: None,
            sequence_key: None,
            entity_order: None,
            value_order: None,
            value_candidate_limit: None,
            group_candidate_limit: None,
            max_moves_per_step: None,
            max_augmenting_depth: None,
            max_rematch_size: None,
            skip_solver_config: false,
        };

        let error = validate_scalar_group_request(&request).unwrap_err();

        assert!(
            error
                .to_string()
                .contains("candidate scalar groups do not accept assignment hook flags"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn assignment_scalar_group_rejects_assignment_rule_without_sequence_key() {
        let request = ScalarGroupRequest {
            name: "ordered_assignment".to_string(),
            assignment: Some("Task.resource_idx".to_string()),
            candidates: None,
            targets: Vec::new(),
            required_entity: None,
            capacity_key: None,
            assignment_rule: Some("compatible_pair".to_string()),
            position_key: None,
            sequence_key: None,
            entity_order: None,
            value_order: None,
            value_candidate_limit: None,
            group_candidate_limit: None,
            max_moves_per_step: None,
            max_augmenting_depth: None,
            max_rematch_size: None,
            skip_solver_config: false,
        };

        let error = validate_scalar_group_request(&request).unwrap_err();

        assert!(
            error
                .to_string()
                .contains("require --sequence-key when --assignment-rule is set"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn candidate_scalar_group_entry_ignores_stale_assignment_hooks() {
        let group = ScalarGroupSpec {
            name: "paired_assignment".to_string(),
            kind: "candidates".to_string(),
            targets: vec![ScalarGroupTargetSpec {
                entity: "task".to_string(),
                entity_plural: "tasks".to_string(),
                field: "resource_idx".to_string(),
            }],
            candidate_provider: "paired_candidates".to_string(),
            assignment_hooks: ScalarGroupAssignmentHooks {
                required_entity: "required_task".to_string(),
                value_order: "resource_priority".to_string(),
                ..ScalarGroupAssignmentHooks::default()
            },
            enabled: true,
            ..ScalarGroupSpec::default()
        };

        let rendered = render_scalar_group_entry("Plan", &group);

        assert!(rendered.contains("ScalarGroup::candidates("));
        assert!(!rendered.contains(".with_required_entity("));
        assert!(!rendered.contains(".with_value_order("));
    }

    #[test]
    fn removable_scalar_group_stubs_keep_hooks_required_by_remaining_groups() {
        let removed = ScalarGroupSpec {
            name: "removed_assignment".to_string(),
            kind: "assignment".to_string(),
            assignment_hooks: ScalarGroupAssignmentHooks {
                required_entity: "required_task".to_string(),
                value_order: "removed_value_order".to_string(),
                ..ScalarGroupAssignmentHooks::default()
            },
            ..ScalarGroupSpec::default()
        };
        let remaining = ScalarGroupSpec {
            name: "remaining_assignment".to_string(),
            kind: "assignment".to_string(),
            assignment_hooks: ScalarGroupAssignmentHooks {
                required_entity: "required_task".to_string(),
                ..ScalarGroupAssignmentHooks::default()
            },
            ..ScalarGroupSpec::default()
        };

        let removable = removable_hook_stubs(
            scalar_group_hook_stubs(&removed, "Plan"),
            scalar_group_hook_stubs_for_all(&[remaining], "Plan"),
        );

        assert_eq!(
            removable,
            vec![HookStub {
                name: "removed_value_order".to_string(),
                kind: HookStubKind::ValueOrder,
            }]
        );
    }

    #[test]
    fn removable_conflict_repair_stubs_keep_providers_required_by_remaining_repairs() {
        let removed = ConflictRepairSpec {
            constraint: "First constraint".to_string(),
            provider: "shared_repair".to_string(),
            ..ConflictRepairSpec::default()
        };
        let remaining = ConflictRepairSpec {
            constraint: "Second constraint".to_string(),
            provider: "shared_repair".to_string(),
            ..ConflictRepairSpec::default()
        };

        let removable = removable_hook_stubs(
            conflict_repair_hook_stubs(&removed, "Plan"),
            conflict_repair_hook_stubs_for_all(&[remaining], "Plan"),
        );

        assert!(removable.is_empty());
    }
}
