use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use crate::app_spec::{
    self, AppSpec, ConflictRepairSpec, ScalarGroupAssignmentHooks, ScalarGroupSpec,
};
use crate::commands::generate_constraint::domain::{DomainModel, EntityInfo};
use crate::commands::generate_constraint::{parse_domain, validate_constraint_mod_source};
use crate::error::{CliError, CliResult};
use crate::model_id::ConstraintId;
use crate::solver_config::{self, SolverConfigDocument, SolverConfigIndex};

const APP_SPEC_PATH: &str = "solverforge.app.toml";
const CONSTRAINTS_DIR: &str = "src/constraints";
const SCALAR_GROUPS_PATH: &str = "scalar_groups";
const CONFLICT_REPAIRS_PATH: &str = "conflict_repairs";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ContractSeverity {
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ContractDiagnostic {
    pub severity: ContractSeverity,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ScalarTargetRef {
    pub entity: String,
    pub entity_type: String,
    pub entity_plural: String,
    pub field: String,
    pub allows_unassigned: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ConstraintRef {
    pub id: String,
    pub declared_name: Option<String>,
}

#[derive(Debug)]
pub(crate) struct ModelContract {
    domain: DomainModel,
    app_spec: AppSpec,
    solver_config: Option<SolverConfigIndex>,
    constraints: Vec<ConstraintRef>,
}

impl ModelContract {
    pub(crate) fn load() -> CliResult<Self> {
        let domain = parse_domain().map_err(CliError::general)?;
        let app_spec = load_app_spec_or_default()?;
        let solver_config = SolverConfigDocument::load_if_present()?
            .map(|doc| doc.index())
            .transpose()?;
        let constraints = load_constraint_refs(Path::new(CONSTRAINTS_DIR))?;
        Ok(Self {
            domain,
            app_spec,
            solver_config,
            constraints,
        })
    }

    #[cfg(test)]
    pub(crate) fn from_parts(domain: DomainModel, app_spec: AppSpec) -> Self {
        Self {
            domain,
            app_spec,
            solver_config: None,
            constraints: Vec::new(),
        }
    }

    #[cfg(test)]
    pub(crate) fn with_constraints(mut self, constraints: Vec<ConstraintRef>) -> Self {
        self.constraints = constraints;
        self
    }

    #[cfg(test)]
    pub(crate) fn with_solver_config(mut self, solver_config: toml::Value) -> Self {
        let raw = toml::to_string_pretty(&solver_config).expect("solver config should serialize");
        self.solver_config =
            Some(SolverConfigIndex::from_toml_str(&raw).expect("solver config should index"));
        self
    }

    pub(crate) fn scalar_groups_for_entity(&self, raw_entity: &str) -> Vec<String> {
        let Some(entity) = self.resolve_entity(raw_entity) else {
            return Vec::new();
        };
        let entity_name = snake_case(&entity.item_type);
        collect_scalar_group_names(&self.app_spec.scalar_groups, |group| {
            group
                .targets
                .iter()
                .any(|target| target.entity == entity_name)
        })
    }

    pub(crate) fn scalar_groups_for_variable(&self, raw_entity: &str, field: &str) -> Vec<String> {
        let Some(entity) = self.resolve_entity(raw_entity) else {
            return Vec::new();
        };
        let entity_name = snake_case(&entity.item_type);
        collect_scalar_group_names(&self.app_spec.scalar_groups, |group| {
            group
                .targets
                .iter()
                .any(|target| target.entity == entity_name && target.field == field)
        })
    }

    pub(crate) fn conflict_repair_refs_for_constraint(&self, raw_constraint: &str) -> Vec<String> {
        let Ok(constraint_id) = ConstraintId::parse(raw_constraint) else {
            return Vec::new();
        };
        let Some(reference) = self.resolve_constraint(constraint_id.as_str()) else {
            return Vec::new();
        };
        let mut refs = BTreeSet::new();
        for repair in self
            .app_spec
            .conflict_repairs
            .iter()
            .filter(|repair| repair.enabled)
        {
            if repair.constraint == reference.id {
                refs.insert(format!("conflict repair {}", repair.provider));
            }
        }
        if let Some(index) = &self.solver_config {
            for location in index.conflict_repair_constraint_locations(&reference.id) {
                refs.insert(format!("solver.toml selector at {}", location.path));
            }
        }
        refs.into_iter().collect()
    }

    pub(crate) fn validate_model_resources(&self) -> Vec<ContractDiagnostic> {
        let mut diagnostics = Vec::new();
        let scalar_groups_by_name = self.scalar_group_names();
        let scalar_config_refs = self.scalar_group_solver_config_refs();
        let scalar_generated_construction_config_refs =
            self.generated_scalar_group_construction_solver_config_refs();
        let scalar_generated_search_config_refs =
            self.generated_scalar_group_search_solver_config_refs();
        let conflict_repair_generated_config_refs =
            self.generated_conflict_repair_solver_config_refs();
        let conflict_repair_config_refs = self.conflict_repair_solver_config_refs();

        for constraint in &self.constraints {
            if constraint.declared_name.as_deref() != Some(constraint.id.as_str()) {
                diagnostics.push(error(format!(
                    "constraint module '{}' must declare .named(\"{}\")",
                    constraint.id, constraint.id
                )));
            }
        }

        self.validate_solution_resource_paths(&mut diagnostics);

        for group in self
            .app_spec
            .scalar_groups
            .iter()
            .filter(|group| group.enabled)
        {
            validate_scalar_group(group, self, &mut diagnostics);
        }
        for repair in &self.app_spec.conflict_repairs {
            validate_conflict_repair(repair, self, &mut diagnostics);
        }

        for group_name in &scalar_config_refs {
            if !scalar_groups_by_name.contains(group_name) {
                diagnostics.push(error(format!(
                    "solver.toml references missing scalar group '{group_name}' at {}",
                    self.scalar_group_solver_config_locations(group_name)
                )));
            }
        }
        for constraint in &conflict_repair_config_refs {
            match self.resolve_constraint(constraint) {
                Some(_) if self.enabled_conflict_repair_targets().contains(constraint) => {}
                Some(_) => diagnostics.push(error(format!(
                    "solver.toml references conflict-repair constraint '{constraint}' at {} but no enabled conflict repair provides it",
                    self.conflict_repair_solver_config_locations(constraint)
                ))),
                None => diagnostics.push(error(format!(
                    "solver.toml references missing conflict-repair constraint '{constraint}' at {}",
                    self.conflict_repair_solver_config_locations(constraint)
                ))),
            }
        }

        for group in self
            .app_spec
            .scalar_groups
            .iter()
            .filter(|group| group.enabled && group.solver_config)
        {
            if self.solver_config.is_none() {
                diagnostics.push(error(format!(
                    "scalar group '{}' is marked solver_config = true but solver.toml is missing",
                    group.name
                )));
                continue;
            }
            let user_locations = self.user_scalar_group_solver_config_locations(&group.name);
            if !user_locations.is_empty() {
                diagnostics.push(error(format!(
                    "scalar group '{}' is marked solver_config = true but solver.toml also has user-authored refs at {}",
                    group.name,
                    solver_config::display_locations(&user_locations)
                )));
            }
            if !scalar_generated_construction_config_refs.contains(&group.name) {
                diagnostics.push(error(format!(
                    "scalar group '{}' is marked solver_config = true but solver.toml has no grouped construction phase for it",
                    group.name
                )));
            }
            if !scalar_generated_search_config_refs.contains(&group.name) {
                diagnostics.push(error(format!(
                    "scalar group '{}' is marked solver_config = true but solver.toml has no grouped local-search selector for it",
                    group.name
                )));
            }
        }
        for repair in self
            .app_spec
            .conflict_repairs
            .iter()
            .filter(|repair| repair.enabled && repair.solver_config)
        {
            if self.solver_config.is_none() {
                diagnostics.push(error(format!(
                    "conflict repair '{}' is marked solver_config = true but solver.toml is missing",
                    repair.constraint
                )));
                continue;
            }
            let user_locations =
                self.user_conflict_repair_solver_config_locations(&repair.constraint);
            if !user_locations.is_empty() {
                diagnostics.push(error(format!(
                    "conflict repair '{}' is marked solver_config = true but solver.toml also has user-authored refs at {}",
                    repair.constraint,
                    solver_config::display_locations(&user_locations)
                )));
            }
            if !conflict_repair_generated_config_refs.contains(&repair.constraint) {
                diagnostics.push(error(format!(
                    "conflict repair '{}' is marked solver_config = true but solver.toml has no conflict-repair selector for it",
                    repair.constraint
                )));
            }
        }

        diagnostics
    }

    fn resolve_entity(&self, raw_entity: &str) -> Option<&EntityInfo> {
        let raw_snake = raw_entity.to_lowercase().replace('-', "_");
        self.domain.entities.iter().find(|entity| {
            entity.item_type == raw_entity
                || snake_case(&entity.item_type) == raw_entity
                || snake_case(&entity.item_type) == raw_snake
                || entity.field_name == raw_entity
                || entity.field_name == raw_snake
        })
    }

    fn resolve_constraint(&self, raw_constraint: &str) -> Option<&ConstraintRef> {
        ConstraintId::parse(raw_constraint).ok().and_then(|id| {
            self.constraints
                .iter()
                .find(|reference| reference.id == id.as_str())
        })
    }

    fn scalar_group_names(&self) -> BTreeSet<String> {
        self.app_spec
            .scalar_groups
            .iter()
            .filter(|group| group.enabled)
            .map(|group| group.name.clone())
            .collect()
    }

    fn scalar_group_solver_config_refs(&self) -> BTreeSet<String> {
        self.solver_config
            .as_ref()
            .map(SolverConfigIndex::scalar_group_refs)
            .unwrap_or_default()
    }

    fn generated_scalar_group_construction_solver_config_refs(&self) -> BTreeSet<String> {
        self.solver_config
            .as_ref()
            .map(SolverConfigIndex::generated_scalar_group_construction_refs)
            .unwrap_or_default()
    }

    fn generated_scalar_group_search_solver_config_refs(&self) -> BTreeSet<String> {
        self.solver_config
            .as_ref()
            .map(SolverConfigIndex::generated_scalar_group_search_refs)
            .unwrap_or_default()
    }

    fn conflict_repair_solver_config_refs(&self) -> BTreeSet<String> {
        self.solver_config
            .as_ref()
            .map(SolverConfigIndex::conflict_repair_refs)
            .unwrap_or_default()
    }

    fn generated_conflict_repair_solver_config_refs(&self) -> BTreeSet<String> {
        self.solver_config
            .as_ref()
            .map(SolverConfigIndex::generated_conflict_repair_refs)
            .unwrap_or_default()
    }

    fn scalar_group_solver_config_locations(&self, name: &str) -> String {
        self.solver_config
            .as_ref()
            .map(|index| solver_config::display_locations(&index.scalar_group_locations(name)))
            .unwrap_or_default()
    }

    fn user_scalar_group_solver_config_locations(
        &self,
        name: &str,
    ) -> Vec<solver_config::SolverConfigLocation> {
        self.solver_config
            .as_ref()
            .map(|index| index.user_scalar_group_locations(name))
            .unwrap_or_default()
    }

    fn conflict_repair_solver_config_locations(&self, constraint: &str) -> String {
        self.solver_config
            .as_ref()
            .map(|index| {
                solver_config::display_locations(
                    &index.conflict_repair_constraint_locations(constraint),
                )
            })
            .unwrap_or_default()
    }

    fn user_conflict_repair_solver_config_locations(
        &self,
        constraint: &str,
    ) -> Vec<solver_config::SolverConfigLocation> {
        self.solver_config
            .as_ref()
            .map(|index| index.user_conflict_repair_constraint_locations(constraint))
            .unwrap_or_default()
    }

    fn enabled_conflict_repair_targets(&self) -> BTreeSet<String> {
        self.app_spec
            .conflict_repairs
            .iter()
            .filter(|repair| repair.enabled)
            .map(|repair| repair.constraint.clone())
            .collect()
    }

    fn validate_solution_resource_paths(&self, diagnostics: &mut Vec<ContractDiagnostic>) {
        if self
            .app_spec
            .scalar_groups
            .iter()
            .any(|group| group.enabled)
        {
            validate_solution_resource_path(
                "scalar groups",
                "scalar_groups",
                SCALAR_GROUPS_PATH,
                self.domain.scalar_groups_path.as_deref(),
                diagnostics,
            );
        }
        if self
            .app_spec
            .conflict_repairs
            .iter()
            .any(|repair| repair.enabled)
        {
            validate_solution_resource_path(
                "conflict repairs",
                "conflict_repairs",
                CONFLICT_REPAIRS_PATH,
                self.domain.conflict_repairs_path.as_deref(),
                diagnostics,
            );
        }
    }
}

pub(crate) fn resolve_scalar_target(domain: &DomainModel, raw: &str) -> CliResult<ScalarTargetRef> {
    let Some((entity_name, field)) = raw.split_once('.') else {
        return Err(CliError::general(format!(
            "invalid scalar target '{}': expected Entity.field",
            raw
        )));
    };
    let entity_name = entity_name.trim();
    let field = field.trim();
    if entity_name.is_empty() || field.is_empty() {
        return Err(CliError::general(format!(
            "invalid scalar target '{}': expected Entity.field",
            raw
        )));
    }

    let entity = domain
        .entities
        .iter()
        .find(|entity| {
            entity.item_type == entity_name || snake_case(&entity.item_type) == entity_name
        })
        .ok_or_else(|| CliError::ResourceNotFound {
            kind: "planning entity",
            name: entity_name.to_string(),
        })?;
    let scalar_var = entity
        .scalar_vars
        .iter()
        .find(|var| var.field == field)
        .ok_or_else(|| CliError::ResourceNotFound {
            kind: "scalar variable",
            name: raw.to_string(),
        })?;
    Ok(ScalarTargetRef {
        entity: snake_case(&entity.item_type),
        entity_type: entity.item_type.clone(),
        entity_plural: entity.field_name.clone(),
        field: scalar_var.field.clone(),
        allows_unassigned: scalar_var.allows_unassigned,
    })
}

pub(crate) fn ensure_entity_has_no_scalar_group_targets(raw_entity: &str) -> CliResult {
    let contract = ModelContract::load()?;
    let groups = contract.scalar_groups_for_entity(raw_entity);
    if groups.is_empty() {
        return Ok(());
    }
    Err(blocked_destroy_error("entity", raw_entity, &groups))
}

pub(crate) fn ensure_variable_has_no_scalar_group_targets(
    raw_entity: &str,
    field: &str,
) -> CliResult {
    let contract = ModelContract::load()?;
    let groups = contract.scalar_groups_for_variable(raw_entity, field);
    if groups.is_empty() {
        return Ok(());
    }
    Err(blocked_destroy_error(
        "variable",
        &format!("{raw_entity}.{field}"),
        &groups,
    ))
}

pub(crate) fn resolve_constraint_ref(raw_constraint: &str) -> CliResult<ConstraintRef> {
    let constraint_id = ConstraintId::parse(raw_constraint)?;
    let reference = load_constraint_refs(Path::new(CONSTRAINTS_DIR))?
        .into_iter()
        .find(|reference| reference.id == constraint_id.as_str())
        .ok_or_else(|| CliError::ResourceNotFound {
            kind: "constraint",
            name: raw_constraint.to_string(),
        })?;
    if reference.declared_name.as_deref() != Some(reference.id.as_str()) {
        return Err(CliError::general(format!(
            "constraint module '{}' must declare .named(\"{}\")",
            reference.id, reference.id
        )));
    }
    Ok(reference)
}

pub(crate) fn ensure_constraint_has_no_conflict_repair_refs(raw_constraint: &str) -> CliResult {
    let contract = ModelContract::load()?;
    let refs = contract.conflict_repair_refs_for_constraint(raw_constraint);
    if refs.is_empty() {
        return Ok(());
    }
    Err(CliError::with_hint(
        format!(
            "cannot destroy constraint '{raw_constraint}' because conflict repairs still reference it: {}",
            refs.join(", ")
        ),
        "destroy dependent conflict repairs first with `solverforge destroy --yes conflict-repair <constraint_id>`",
    ))
}

fn validate_scalar_group(
    group: &ScalarGroupSpec,
    contract: &ModelContract,
    diagnostics: &mut Vec<ContractDiagnostic>,
) {
    match group.kind.as_str() {
        "assignment" => validate_assignment_scalar_group(group, contract, diagnostics),
        "candidates" => validate_candidate_scalar_group(group, contract, diagnostics),
        other => diagnostics.push(error(format!(
            "scalar group '{}' has unsupported kind '{}'",
            group.name, other
        ))),
    }
}

fn validate_solution_resource_path(
    label: &str,
    key: &str,
    expected: &str,
    actual: Option<&str>,
    diagnostics: &mut Vec<ContractDiagnostic>,
) {
    if actual == Some(expected) {
        return;
    }
    diagnostics.push(error(format!(
        "enabled {label} require #[planning_solution(...)] to declare {key} = \"{expected}\""
    )));
}

fn validate_assignment_scalar_group(
    group: &ScalarGroupSpec,
    contract: &ModelContract,
    diagnostics: &mut Vec<ContractDiagnostic>,
) {
    if group.targets.len() != 1 {
        diagnostics.push(error(format!(
            "assignment scalar group '{}' must target exactly one scalar variable",
            group.name
        )));
    }
    validate_assignment_hooks(group, diagnostics);
    validate_scalar_group_targets(group, contract, true, diagnostics);
}

fn validate_candidate_scalar_group(
    group: &ScalarGroupSpec,
    contract: &ModelContract,
    diagnostics: &mut Vec<ContractDiagnostic>,
) {
    if group.candidate_provider.trim().is_empty() {
        diagnostics.push(error(format!(
            "candidate scalar group '{}' is missing candidate_provider",
            group.name
        )));
    }
    if group.targets.is_empty() {
        diagnostics.push(error(format!(
            "candidate scalar group '{}' must target at least one scalar variable",
            group.name
        )));
    }
    if assignment_hooks_are_set(&group.assignment_hooks) {
        diagnostics.push(error(format!(
            "candidate scalar group '{}' must not carry assignment hooks",
            group.name
        )));
    }
    validate_scalar_group_targets(group, contract, false, diagnostics);
}

fn validate_conflict_repair(
    repair: &ConflictRepairSpec,
    contract: &ModelContract,
    diagnostics: &mut Vec<ContractDiagnostic>,
) {
    if !repair.enabled {
        return;
    }
    if repair.constraint.trim().is_empty() {
        diagnostics.push(error(format!(
            "conflict repair '{}' is missing a constraint ID",
            repair.provider
        )));
        return;
    }
    if repair.provider.trim().is_empty() {
        diagnostics.push(error(format!(
            "conflict repair for '{}' is missing a provider",
            repair.constraint
        )));
    }
    if !matches!(repair.selector.as_str(), "" | "compound" | "conflict") {
        diagnostics.push(error(format!(
            "conflict repair for '{}' has unsupported selector '{}'",
            repair.constraint, repair.selector
        )));
    }
    if ConstraintId::parse(&repair.constraint).is_err() {
        diagnostics.push(error(format!(
            "conflict repair provider '{}' references invalid constraint ID '{}'",
            repair.provider, repair.constraint
        )));
        return;
    }
    match contract.resolve_constraint(&repair.constraint) {
        Some(_) => {}
        None => diagnostics.push(error(format!(
            "conflict repair provider '{}' references missing constraint '{}'",
            repair.provider, repair.constraint
        ))),
    }
}

fn validate_assignment_hooks(group: &ScalarGroupSpec, diagnostics: &mut Vec<ContractDiagnostic>) {
    if !group.assignment_hooks.assignment_rule.is_empty()
        && group.assignment_hooks.sequence_key.is_empty()
    {
        diagnostics.push(error(format!(
            "assignment scalar group '{}' uses assignment_rule without sequence_key",
            group.name
        )));
    }
}

fn validate_scalar_group_targets(
    group: &ScalarGroupSpec,
    contract: &ModelContract,
    require_unassigned: bool,
    diagnostics: &mut Vec<ContractDiagnostic>,
) {
    let entity_by_name = contract
        .domain
        .entities
        .iter()
        .map(|entity| (snake_case(&entity.item_type), entity))
        .collect::<BTreeMap<_, _>>();

    for target in &group.targets {
        let Some(entity) = entity_by_name.get(&target.entity) else {
            diagnostics.push(error(format!(
                "scalar group '{}' targets missing entity '{}'",
                group.name, target.entity
            )));
            continue;
        };
        let Some(variable) = entity
            .scalar_vars
            .iter()
            .find(|variable| variable.field == target.field)
        else {
            diagnostics.push(error(format!(
                "scalar group '{}' targets missing scalar variable '{}.{}'",
                group.name, entity.item_type, target.field
            )));
            continue;
        };
        if require_unassigned && !variable.allows_unassigned {
            diagnostics.push(error(format!(
                "assignment scalar group '{}' targets '{}.{}' but the scalar variable does not allow unassigned values",
                group.name, entity.item_type, target.field
            )));
        }
    }
}

fn blocked_destroy_error(kind: &str, name: &str, groups: &[String]) -> CliError {
    CliError::with_hint(
        format!(
            "cannot destroy {kind} '{name}' because scalar groups still target it: {}",
            groups.join(", ")
        ),
        "destroy dependent scalar groups first with `solverforge destroy --yes scalar-group <name>`",
    )
}

fn collect_scalar_group_names(
    groups: &[ScalarGroupSpec],
    mut predicate: impl FnMut(&ScalarGroupSpec) -> bool,
) -> Vec<String> {
    groups
        .iter()
        .filter(|group| group.enabled && predicate(group))
        .map(|group| group.name.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn assignment_hooks_are_set(hooks: &ScalarGroupAssignmentHooks) -> bool {
    !hooks.required_entity.is_empty()
        || !hooks.capacity_key.is_empty()
        || !hooks.assignment_rule.is_empty()
        || !hooks.position_key.is_empty()
        || !hooks.sequence_key.is_empty()
        || !hooks.entity_order.is_empty()
        || !hooks.value_order.is_empty()
}

fn load_app_spec_or_default() -> CliResult<AppSpec> {
    if Path::new(APP_SPEC_PATH).exists() {
        app_spec::load()
    } else {
        Ok(AppSpec::default())
    }
}

fn load_constraint_refs(dir: &Path) -> CliResult<Vec<ConstraintRef>> {
    if !dir.exists() {
        return Ok(Vec::new());
    }
    let mod_path = dir.join("mod.rs");
    if !mod_path.exists() {
        return Ok(Vec::new());
    }
    let mod_src = fs::read_to_string(&mod_path).map_err(|err| CliError::IoError {
        context: format!("failed to read {}", mod_path.display()),
        source: err,
    })?;
    let modules = validate_constraint_mod_source(&mod_src)
        .map_err(|err| CliError::general(format!("{}: {err}", mod_path.display())))?;
    let mut refs = Vec::new();
    for id in modules {
        ConstraintId::parse(&id)?;
        let path = dir.join(format!("{id}.rs"));
        refs.push(ConstraintRef {
            id,
            declared_name: read_declared_constraint_id(&path),
        });
    }
    refs.sort_by(|left, right| left.id.cmp(&right.id));
    Ok(refs)
}

fn read_declared_constraint_id(path: &Path) -> Option<String> {
    let raw = fs::read_to_string(path).ok()?;
    let file = syn::parse_file(&raw).ok()?;
    let mut visitor = NamedConstraintVisitor { names: Vec::new() };
    syn::visit::visit_file(&mut visitor, &file);
    visitor.names.into_iter().next()
}

struct NamedConstraintVisitor {
    names: Vec<String>,
}

impl<'ast> syn::visit::Visit<'ast> for NamedConstraintVisitor {
    fn visit_expr_method_call(&mut self, node: &'ast syn::ExprMethodCall) {
        if node.method == "named" {
            if let Some(syn::Expr::Lit(syn::ExprLit {
                lit: syn::Lit::Str(name),
                ..
            })) = node.args.first()
            {
                self.names.push(name.value());
            }
        }
        syn::visit::visit_expr_method_call(self, node);
    }
}

fn error(message: String) -> ContractDiagnostic {
    ContractDiagnostic {
        severity: ContractSeverity::Error,
        message,
    }
}

fn snake_case(name: &str) -> String {
    let mut out = String::new();
    for (idx, ch) in name.chars().enumerate() {
        if ch.is_uppercase() {
            if idx > 0 {
                out.push('_');
            }
            out.push(ch.to_ascii_lowercase());
        } else {
            out.push(ch);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app_spec::{
        ConflictRepairSpec, ScalarGroupAssignmentHooks, ScalarGroupTargetSpec, VariableSpec,
    };
    use crate::commands::generate_constraint::domain::{EntityInfo, ScalarVarInfo};

    fn contract_with_groups(groups: Vec<ScalarGroupSpec>) -> ModelContract {
        ModelContract::from_parts(
            DomainModel {
                solution_type: "Plan".to_string(),
                score_type: "HardSoftScore".to_string(),
                scalar_groups_path: Some("scalar_groups".to_string()),
                conflict_repairs_path: None,
                entities: vec![EntityInfo {
                    field_name: "tasks".to_string(),
                    item_type: "Task".to_string(),
                    scalar_vars: vec![ScalarVarInfo {
                        field: "resource_idx".to_string(),
                        value_range_provider: "resources".to_string(),
                        countable_range: None,
                        allows_unassigned: true,
                        hooks: Default::default(),
                    }],
                    list_vars: vec![],
                }],
                facts: vec![],
            },
            AppSpec {
                scalar_groups: groups,
                variables: vec![VariableSpec {
                    entity: "task".to_string(),
                    entity_plural: "tasks".to_string(),
                    field: "resource_idx".to_string(),
                    kind: "scalar".to_string(),
                    allows_unassigned: true,
                    ..VariableSpec::default()
                }],
                ..AppSpec::default()
            },
        )
    }

    fn assignment_group(name: &str) -> ScalarGroupSpec {
        ScalarGroupSpec {
            name: name.to_string(),
            kind: "assignment".to_string(),
            targets: vec![ScalarGroupTargetSpec {
                entity: "task".to_string(),
                entity_plural: "tasks".to_string(),
                field: "resource_idx".to_string(),
            }],
            enabled: true,
            solver_config: true,
            ..ScalarGroupSpec::default()
        }
    }

    fn candidate_group(name: &str) -> ScalarGroupSpec {
        ScalarGroupSpec {
            name: name.to_string(),
            kind: "candidates".to_string(),
            targets: assignment_group("unused").targets,
            candidate_provider: "candidate_provider".to_string(),
            enabled: true,
            solver_config: true,
            ..ScalarGroupSpec::default()
        }
    }

    fn required_assignment_constraint() -> ConstraintRef {
        ConstraintRef {
            id: "required_assignment".to_string(),
            declared_name: Some("required_assignment".to_string()),
        }
    }

    #[test]
    fn scalar_groups_for_entity_returns_dependent_groups() {
        let contract = contract_with_groups(vec![assignment_group("required_assignment")]);

        assert_eq!(
            contract.scalar_groups_for_entity("Task"),
            vec!["required_assignment".to_string()]
        );
    }

    #[test]
    fn scalar_groups_for_variable_matches_exact_target() {
        let contract = contract_with_groups(vec![assignment_group("required_assignment")]);

        assert_eq!(
            contract.scalar_groups_for_variable("task", "resource_idx"),
            vec!["required_assignment".to_string()]
        );
        assert!(contract
            .scalar_groups_for_variable("task", "other_idx")
            .is_empty());
    }

    #[test]
    fn contract_rejects_assignment_rule_without_sequence_key() {
        let mut group = assignment_group("ordered_assignment");
        group.assignment_hooks.assignment_rule = "compatible_pair".to_string();
        let contract = contract_with_groups(vec![group]);

        let errors = contract.validate_model_resources();

        assert!(errors.iter().any(|diagnostic| diagnostic
            .message
            .contains("uses assignment_rule without sequence_key")));
    }

    #[test]
    fn contract_rejects_candidate_assignment_hooks() {
        let group = ScalarGroupSpec {
            name: "candidate_group".to_string(),
            kind: "candidates".to_string(),
            targets: assignment_group("unused").targets,
            candidate_provider: "candidate_provider".to_string(),
            assignment_hooks: ScalarGroupAssignmentHooks {
                required_entity: "required_task".to_string(),
                ..ScalarGroupAssignmentHooks::default()
            },
            enabled: true,
            ..ScalarGroupSpec::default()
        };
        let contract = contract_with_groups(vec![group]);

        let errors = contract.validate_model_resources();

        assert!(errors.iter().any(|diagnostic| diagnostic
            .message
            .contains("must not carry assignment hooks")));
    }

    #[test]
    fn contract_rejects_enabled_scalar_group_without_solution_path() {
        let mut group = assignment_group("required_assignment");
        group.solver_config = false;
        let mut contract = contract_with_groups(vec![group]);
        contract.domain.scalar_groups_path = None;

        let errors = contract.validate_model_resources();

        assert!(errors.iter().any(|diagnostic| diagnostic.message.contains(
            "enabled scalar groups require #[planning_solution(...)] to declare scalar_groups = \"scalar_groups\""
        )));
    }

    #[test]
    fn contract_rejects_invalid_conflict_repair_constraint_id() {
        let mut contract = contract_with_groups(Vec::new()).with_constraints(vec![]);
        contract.app_spec.conflict_repairs = vec![ConflictRepairSpec {
            constraint: "Missing Constraint".to_string(),
            provider: "repair_missing".to_string(),
            enabled: true,
            ..ConflictRepairSpec::default()
        }];

        let errors = contract.validate_model_resources();

        assert!(errors.iter().any(|diagnostic| diagnostic.message.contains(
            "conflict repair provider 'repair_missing' references invalid constraint ID 'Missing Constraint'"
        )));
    }

    #[test]
    fn contract_accepts_canonical_conflict_repair_constraint_id() {
        let mut contract = contract_with_groups(Vec::new())
            .with_constraints(vec![required_assignment_constraint()]);
        contract.domain.conflict_repairs_path = Some("conflict_repairs".to_string());
        contract.app_spec.conflict_repairs = vec![ConflictRepairSpec {
            constraint: "required_assignment".to_string(),
            provider: "repair_required_assignment".to_string(),
            enabled: true,
            solver_config: false,
            ..ConflictRepairSpec::default()
        }];

        let errors = contract.validate_model_resources();

        assert!(errors.is_empty(), "unexpected diagnostics: {errors:?}");
    }

    #[test]
    fn contract_rejects_enabled_conflict_repair_without_solution_path() {
        let mut contract = contract_with_groups(Vec::new())
            .with_constraints(vec![required_assignment_constraint()]);
        contract.app_spec.conflict_repairs = vec![ConflictRepairSpec {
            constraint: "required_assignment".to_string(),
            provider: "repair_required_assignment".to_string(),
            enabled: true,
            solver_config: false,
            ..ConflictRepairSpec::default()
        }];

        let errors = contract.validate_model_resources();

        assert!(errors.iter().any(|diagnostic| diagnostic.message.contains(
            "enabled conflict repairs require #[planning_solution(...)] to declare conflict_repairs = \"conflict_repairs\""
        )));
    }

    #[test]
    fn contract_rejects_constraint_declared_name_drift() {
        let contract = contract_with_groups(Vec::new()).with_constraints(vec![ConstraintRef {
            id: "required_assignment".to_string(),
            declared_name: Some("Required Assignment".to_string()),
        }]);

        let errors = contract.validate_model_resources();

        assert!(errors.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("constraint module 'required_assignment' must declare .named(\"required_assignment\")")
        }));
    }

    #[test]
    fn contract_rejects_stale_conflict_repair_solver_config() {
        let mut contract = contract_with_groups(Vec::new())
            .with_constraints(vec![required_assignment_constraint()]);
        contract.app_spec.conflict_repairs = vec![ConflictRepairSpec {
            constraint: "required_assignment".to_string(),
            provider: "repair_required_assignment".to_string(),
            enabled: true,
            ..ConflictRepairSpec::default()
        }];
        contract = contract.with_solver_config(
            toml::from_str(
                r#"
[[phases]]
type = "local_search"

[phases.move_selector]
type = "compound_conflict_repair_move_selector"
constraints = ["Missing Constraint"]
"#,
            )
            .expect("solver config should parse"),
        );

        let errors = contract.validate_model_resources();

        assert!(errors.iter().any(|diagnostic| diagnostic.message.contains(
            "solver.toml references missing conflict-repair constraint 'Missing Constraint'"
        )));
    }

    #[test]
    fn contract_rejects_candidate_scalar_group_missing_construction_config() {
        let contract = contract_with_groups(vec![candidate_group("candidate_group")])
            .with_solver_config(
                toml::from_str(
                    r#"
[[phases]]
type = "local_search"

[phases.move_selector]
type = "grouped_scalar_move_selector"
group_name = "candidate_group"
require_hard_improvement = true
"#,
                )
                .expect("solver config should parse"),
            );

        let errors = contract.validate_model_resources();

        assert!(errors.iter().any(|diagnostic| diagnostic
            .message
            .contains("has no grouped construction phase")));
    }

    #[test]
    fn contract_reports_stale_nested_solver_config_locations() {
        let contract = contract_with_groups(Vec::new()).with_solver_config(
            toml::from_str(
                r#"
[[phases]]
type = "local_search"

[phases.move_selector]
type = "union_move_selector"

[[phases.move_selector.selectors]]
type = "grouped_scalar_move_selector"
group_name = "missing_group"
"#,
            )
            .expect("solver config should parse"),
        );

        let errors = contract.validate_model_resources();

        assert!(errors.iter().any(|diagnostic| diagnostic
            .message
            .contains("phases[0].move_selector.selectors[0].group_name")));
    }
}
