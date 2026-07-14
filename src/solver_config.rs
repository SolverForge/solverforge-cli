use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use crate::app_spec::{AppSpec, ConflictRepairSpec, ScalarGroupSpec};
use crate::error::{CliError, CliResult};

pub(crate) const CONFIG_PATH: &str = "solver.toml";

const BEGIN_REGION: &str = "# @solverforge:begin solver-config";
const END_REGION: &str = "# @solverforge:end solver-config";
const OWNER_PREFIX: &str = "# @solverforge:owner ";
const LEGACY_BEGIN_PREFIX: &str = "# @solverforge:begin solver-config ";
const LEGACY_END_PREFIX: &str = "# @solverforge:end solver-config ";
const SCALAR_GROUP_KIND: &str = "scalar-group";
const CONFLICT_REPAIR_KIND: &str = "conflict-repair";
const CONSTRUCTION_ROLE: &str = "construction";
const SEARCH_ROLE: &str = "search";

#[derive(Debug, Clone)]
pub(crate) struct SolverConfigDocument {
    path: PathBuf,
    raw: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct SolverConfigIndex {
    scalar_group_construction_refs: BTreeMap<String, Vec<SolverConfigRef>>,
    scalar_group_search_refs: BTreeMap<String, Vec<SolverConfigRef>>,
    conflict_repair_constraint_refs: BTreeMap<String, Vec<SolverConfigRef>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SolverConfigLocation {
    pub(crate) path: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SolverConfigRef {
    location: SolverConfigLocation,
    owner: Option<SolverConfigOwner>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ManagedRegion {
    start_line: usize,
    end_line: usize,
    phase_owners: BTreeMap<usize, SolverConfigOwner>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SolverConfigOwner {
    kind: SolverConfigOwnerKind,
    id: String,
    role: SolverConfigOwnerRole,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SolverConfigOwnerKind {
    ScalarGroup,
    ConflictRepair,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SolverConfigOwnerRole {
    Construction,
    Search,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RefOwnerFilter {
    Any,
    Generated,
    User,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GeneratedPhaseKind {
    ScalarGroupConstruction,
    ScalarGroupSearch,
    ConflictRepairSearch,
}

impl SolverConfigDocument {
    pub(crate) fn load() -> CliResult<Self> {
        let path = Path::new(CONFIG_PATH);
        if !path.exists() {
            return Err(CliError::NotInProject {
                missing: CONFIG_PATH,
            });
        }
        Self::read(path)
    }

    pub(crate) fn load_if_present() -> CliResult<Option<Self>> {
        let path = Path::new(CONFIG_PATH);
        if !path.exists() {
            return Ok(None);
        }
        Self::read(path).map(Some)
    }

    pub(crate) fn write(&self) -> CliResult {
        fs::write(&self.path, self.to_toml_string()?).map_err(|err| CliError::IoError {
            context: format!("failed to write {}", self.path.display()),
            source: err,
        })
    }

    pub(crate) fn index(&self) -> CliResult<SolverConfigIndex> {
        SolverConfigIndex::from_toml_str(&self.raw)
    }

    pub(crate) fn validate_managed_blocks(&self) -> CliResult {
        parse_managed_region(&self.raw).map(|_| ())
    }

    pub(crate) fn plan_sync_model_resources(&self, spec: &AppSpec) -> CliResult<Self> {
        let raw_without_region = remove_managed_region(&self.raw)?;
        let user_index = SolverConfigIndex::from_toml_str(&raw_without_region)?;

        for group in spec.solver_config_scalar_groups() {
            let locations = user_index.scalar_group_locations(&group.name);
            if !locations.is_empty() {
                return Err(blocked_solver_config_generate(
                    "scalar group",
                    &group.name,
                    &locations,
                ));
            }
        }
        for repair in spec.solver_config_conflict_repairs() {
            let locations = user_index.conflict_repair_constraint_locations(&repair.constraint);
            if !locations.is_empty() {
                return Err(blocked_solver_config_generate(
                    "conflict repair",
                    &repair.constraint,
                    &locations,
                ));
            }
        }

        let region = render_model_resource_region(spec);
        let raw = insert_managed_region(&raw_without_region, &region)?;
        Self::from_raw(self.path.clone(), raw)
    }

    pub(crate) fn ensure_no_user_scalar_group_refs(&self, name: &str) -> CliResult {
        let locations = self.index()?.user_scalar_group_locations(name);
        if locations.is_empty() {
            return Ok(());
        }
        Err(blocked_solver_config_destroy(
            "scalar group",
            name,
            &locations,
        ))
    }

    pub(crate) fn ensure_no_user_conflict_repair_refs(&self, constraint: &str) -> CliResult {
        let locations = self
            .index()?
            .user_conflict_repair_constraint_locations(constraint);
        if locations.is_empty() {
            return Ok(());
        }
        Err(blocked_solver_config_destroy(
            "conflict repair",
            constraint,
            &locations,
        ))
    }

    fn read(path: &Path) -> CliResult<Self> {
        let raw = fs::read_to_string(path).map_err(|err| CliError::IoError {
            context: format!("failed to read {}", path.display()),
            source: err,
        })?;
        Self::from_raw(path.to_path_buf(), raw)
    }

    #[cfg(test)]
    fn from_toml_str(path: PathBuf, raw: &str) -> CliResult<Self> {
        Self::from_raw(path, raw.to_string())
    }

    fn from_raw(path: PathBuf, raw: String) -> CliResult<Self> {
        let doc = Self { path, raw };
        doc.validate_managed_blocks()?;
        validate_current_settings(&doc.raw)?;
        doc.index()?;
        Ok(doc)
    }

    fn to_toml_string(&self) -> CliResult<String> {
        self.validate_managed_blocks()?;
        Ok(self.raw.clone())
    }
}

impl SolverConfigIndex {
    pub(crate) fn from_toml_str(raw: &str) -> CliResult<Self> {
        let region = parse_managed_region(raw)?;
        let value = toml::from_str(raw)
            .map_err(|err| CliError::general(format!("failed to parse solver.toml: {err}")))?;
        Self::from_value(&value, region.as_ref())
    }

    fn from_value(value: &toml::Value, region: Option<&ManagedRegion>) -> CliResult<Self> {
        let mut index = Self::default();
        let table = value
            .as_table()
            .ok_or_else(|| CliError::general("solver.toml root is not a TOML table"))?;
        if let Some(phases) = table.get("phases") {
            collect_top_level_phase_array(phases, region, &mut index)?;
        }
        Ok(index)
    }

    pub(crate) fn scalar_group_refs(&self) -> BTreeSet<String> {
        self.scalar_group_construction_refs
            .keys()
            .chain(self.scalar_group_search_refs.keys())
            .cloned()
            .collect()
    }

    pub(crate) fn generated_scalar_group_construction_refs(&self) -> BTreeSet<String> {
        keys_for_refs(
            &self.scalar_group_construction_refs,
            RefOwnerFilter::Generated,
        )
    }

    pub(crate) fn generated_scalar_group_search_refs(&self) -> BTreeSet<String> {
        keys_for_refs(&self.scalar_group_search_refs, RefOwnerFilter::Generated)
    }

    pub(crate) fn conflict_repair_refs(&self) -> BTreeSet<String> {
        keys_for_refs(&self.conflict_repair_constraint_refs, RefOwnerFilter::Any)
    }

    pub(crate) fn generated_conflict_repair_refs(&self) -> BTreeSet<String> {
        keys_for_refs(
            &self.conflict_repair_constraint_refs,
            RefOwnerFilter::Generated,
        )
    }

    pub(crate) fn scalar_group_locations(&self, name: &str) -> Vec<SolverConfigLocation> {
        let mut locations = Vec::new();
        locations.extend(locations_for_refs(
            &self.scalar_group_construction_refs,
            name,
            RefOwnerFilter::Any,
        ));
        locations.extend(locations_for_refs(
            &self.scalar_group_search_refs,
            name,
            RefOwnerFilter::Any,
        ));
        locations
    }

    pub(crate) fn user_scalar_group_locations(&self, name: &str) -> Vec<SolverConfigLocation> {
        let mut locations = Vec::new();
        locations.extend(locations_for_refs(
            &self.scalar_group_construction_refs,
            name,
            RefOwnerFilter::User,
        ));
        locations.extend(locations_for_refs(
            &self.scalar_group_search_refs,
            name,
            RefOwnerFilter::User,
        ));
        locations
    }

    pub(crate) fn conflict_repair_constraint_locations(
        &self,
        constraint: &str,
    ) -> Vec<SolverConfigLocation> {
        locations_for_refs(
            &self.conflict_repair_constraint_refs,
            constraint,
            RefOwnerFilter::Any,
        )
    }

    pub(crate) fn user_conflict_repair_constraint_locations(
        &self,
        constraint: &str,
    ) -> Vec<SolverConfigLocation> {
        locations_for_refs(
            &self.conflict_repair_constraint_refs,
            constraint,
            RefOwnerFilter::User,
        )
    }

    #[cfg(test)]
    pub(crate) fn has_scalar_group_construction_ref(&self, name: &str) -> bool {
        self.scalar_group_construction_refs.contains_key(name)
    }

    #[cfg(test)]
    pub(crate) fn has_scalar_group_search_ref(&self, name: &str) -> bool {
        self.scalar_group_search_refs.contains_key(name)
    }

    fn add_scalar_group_construction_ref(
        &mut self,
        id: &str,
        path: String,
        owner: Option<SolverConfigOwner>,
    ) {
        self.scalar_group_construction_refs
            .entry(id.to_string())
            .or_default()
            .push(SolverConfigRef {
                location: SolverConfigLocation { path },
                owner,
            });
    }

    fn add_scalar_group_search_ref(
        &mut self,
        id: &str,
        path: String,
        owner: Option<SolverConfigOwner>,
    ) {
        self.scalar_group_search_refs
            .entry(id.to_string())
            .or_default()
            .push(SolverConfigRef {
                location: SolverConfigLocation { path },
                owner,
            });
    }

    fn add_conflict_repair_constraint_ref(
        &mut self,
        id: &str,
        path: String,
        owner: Option<SolverConfigOwner>,
    ) {
        self.conflict_repair_constraint_refs
            .entry(id.to_string())
            .or_default()
            .push(SolverConfigRef {
                location: SolverConfigLocation { path },
                owner,
            });
    }
}

impl AppSpec {
    pub(crate) fn solver_config_scalar_groups(&self) -> impl Iterator<Item = &ScalarGroupSpec> {
        self.scalar_groups
            .iter()
            .filter(|group| group.enabled && group.solver_config)
    }

    pub(crate) fn solver_config_conflict_repairs(
        &self,
    ) -> impl Iterator<Item = &ConflictRepairSpec> {
        self.conflict_repairs
            .iter()
            .filter(|repair| repair.enabled && repair.solver_config)
    }

    pub(crate) fn has_solver_config_resources(&self) -> bool {
        self.solver_config_scalar_groups().next().is_some()
            || self.solver_config_conflict_repairs().next().is_some()
    }
}

pub(crate) fn validate_managed_blocks(raw: &str) -> CliResult {
    parse_managed_region(raw).map(|_| ())
}

pub(crate) fn validate_current_settings(raw: &str) -> CliResult {
    let value: toml::Value = toml::from_str(raw)
        .map_err(|err| CliError::general(format!("failed to parse solver.toml: {err}")))?;
    let Some(candidate_trace) = value.get("candidate_trace") else {
        return Ok(());
    };
    let table = candidate_trace
        .as_table()
        .ok_or_else(|| CliError::general("solver.toml candidate_trace must be a TOML table"))?;
    let max_entries = table
        .get("max_entries")
        .and_then(toml::Value::as_integer)
        .ok_or_else(|| {
            CliError::general("solver.toml candidate_trace.max_entries must be a positive integer")
        })?;
    if max_entries <= 0 {
        return Err(CliError::general(
            "solver.toml candidate_trace.max_entries must be a positive integer",
        ));
    }
    Ok(())
}

fn parse_managed_region(raw: &str) -> CliResult<Option<ManagedRegion>> {
    let mut active_start = None;
    let mut region = None;
    let mut pending_owner: Option<(SolverConfigOwner, usize)> = None;
    let mut phase_owners = BTreeMap::new();
    let mut phase_index = 0usize;

    for (line_index, line) in raw.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed == BEGIN_REGION {
            if active_start.is_some() {
                return Err(CliError::general(format!(
                    "solver.toml opens nested managed solver config region at line {}",
                    line_index + 1
                )));
            }
            if region.is_some() {
                return Err(CliError::general(
                    "solver.toml contains multiple managed solver config regions",
                ));
            }
            active_start = Some(line_index);
            phase_owners.clear();
            pending_owner = None;
            continue;
        }
        if trimmed == END_REGION {
            let Some(start_line) = active_start.take() else {
                return Err(CliError::general(format!(
                    "solver.toml closes unmanaged solver config region at line {}",
                    line_index + 1
                )));
            };
            if let Some((owner, owner_line)) = pending_owner.take() {
                return Err(CliError::general(format!(
                    "solver.toml managed owner marker '{}' for '{}' at line {} is not followed by a phase",
                    owner.kind.as_str(),
                    owner.id,
                    owner_line + 1
                )));
            }
            region = Some(ManagedRegion {
                start_line,
                end_line: line_index,
                phase_owners: phase_owners.clone(),
            });
            continue;
        }
        if trimmed.starts_with(LEGACY_BEGIN_PREFIX) || trimmed.starts_with(LEGACY_END_PREFIX) {
            return Err(CliError::general(format!(
                "solver.toml contains unsupported per-resource solver config marker at line {}",
                line_index + 1
            )));
        }
        if let Some(rest) = trimmed.strip_prefix(OWNER_PREFIX) {
            if active_start.is_none() {
                return Err(CliError::general(format!(
                    "solver.toml managed owner marker appears outside the managed region at line {}",
                    line_index + 1
                )));
            }
            if let Some((owner, owner_line)) =
                pending_owner.replace((parse_owner_marker(rest, line_index + 1)?, line_index))
            {
                return Err(CliError::general(format!(
                    "solver.toml managed owner marker '{}' for '{}' at line {} is not followed by a phase",
                    owner.kind.as_str(),
                    owner.id,
                    owner_line + 1
                )));
            }
            continue;
        }
        if trimmed == "[[phases]]" {
            if active_start.is_some() {
                let Some((owner, _owner_line)) = pending_owner.take() else {
                    return Err(CliError::general(format!(
                        "solver.toml managed phase at line {} is missing an owner marker",
                        line_index + 1
                    )));
                };
                phase_owners.insert(phase_index, owner);
            }
            phase_index += 1;
        }
    }

    if let Some(start_line) = active_start {
        return Err(CliError::general(format!(
            "solver.toml managed solver config region opened at line {} is not closed",
            start_line + 1
        )));
    }

    Ok(region)
}

fn parse_owner_marker(rest: &str, line_number: usize) -> CliResult<SolverConfigOwner> {
    let mut parts = rest.split_whitespace();
    let Some(kind) = parts.next() else {
        return Err(invalid_owner_marker(line_number));
    };
    let Some(id) = parts.next() else {
        return Err(invalid_owner_marker(line_number));
    };
    let Some(role) = parts.next() else {
        return Err(invalid_owner_marker(line_number));
    };
    if parts.next().is_some() || id.is_empty() {
        return Err(invalid_owner_marker(line_number));
    }
    let kind = match kind {
        SCALAR_GROUP_KIND => SolverConfigOwnerKind::ScalarGroup,
        CONFLICT_REPAIR_KIND => SolverConfigOwnerKind::ConflictRepair,
        _ => return Err(invalid_owner_marker(line_number)),
    };
    let role = match role {
        CONSTRUCTION_ROLE if kind == SolverConfigOwnerKind::ScalarGroup => {
            SolverConfigOwnerRole::Construction
        }
        SEARCH_ROLE => SolverConfigOwnerRole::Search,
        _ => return Err(invalid_owner_marker(line_number)),
    };
    Ok(SolverConfigOwner {
        kind,
        id: id.to_string(),
        role,
    })
}

fn invalid_owner_marker(line_number: usize) -> CliError {
    CliError::general(format!(
        "invalid solver.toml managed owner marker at line {line_number}"
    ))
}

fn remove_managed_region(raw: &str) -> CliResult<String> {
    let Some(region) = parse_managed_region(raw)? else {
        return Ok(raw.to_string());
    };
    let lines = raw.lines().collect::<Vec<_>>();
    let mut kept = Vec::new();
    for (index, line) in lines.iter().enumerate() {
        if index < region.start_line || index > region.end_line {
            kept.push(*line);
        }
    }
    let mut rendered = kept.join("\n");
    if raw.ends_with('\n') && !rendered.is_empty() {
        rendered.push('\n');
    }
    Ok(rendered)
}

fn insert_managed_region(raw: &str, region: &str) -> CliResult<String> {
    if region.trim().is_empty() {
        return Ok(ensure_trailing_newline(raw.trim_end()));
    }

    let insert_line = managed_region_insert_line(raw)?;
    let lines = raw.lines().collect::<Vec<_>>();
    let before = lines[..insert_line].join("\n");
    let after = lines[insert_line..].join("\n");
    let mut rendered = String::new();

    if !before.trim().is_empty() {
        rendered.push_str(before.trim_end());
        rendered.push_str("\n\n");
    }
    rendered.push_str(region.trim_end());
    if !after.trim().is_empty() {
        rendered.push_str("\n\n");
        rendered.push_str(after.trim_start());
    }
    rendered.push('\n');
    Ok(rendered)
}

fn ensure_trailing_newline(raw: &str) -> String {
    if raw.is_empty() {
        String::new()
    } else {
        format!("{raw}\n")
    }
}

fn managed_region_insert_line(raw: &str) -> CliResult<usize> {
    let phases = top_level_phase_metadata(raw)?;
    phases
        .iter()
        .find(|phase| phase.phase_type.as_deref() != Some("construction_heuristic"))
        .map(|phase| phase.start_line)
        .map_or_else(|| Ok(raw.lines().count()), Ok)
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PhaseMeta {
    start_line: usize,
    phase_type: Option<String>,
}

fn top_level_phase_metadata(raw: &str) -> CliResult<Vec<PhaseMeta>> {
    let document = raw
        .parse::<toml_edit::Document<String>>()
        .map_err(|err| CliError::general(format!("failed to parse solver.toml: {err}")))?;
    let Some(phases) = document
        .as_table()
        .get("phases")
        .and_then(toml_edit::Item::as_array_of_tables)
    else {
        return Ok(Vec::new());
    };

    phases
        .iter()
        .map(|phase| {
            let span = phase
                .span()
                .ok_or_else(|| CliError::general("solver.toml phase source span is unavailable"))?;
            if span.start > raw.len() {
                return Err(CliError::general(
                    "solver.toml phase source span is invalid",
                ));
            }
            Ok(PhaseMeta {
                start_line: line_index_at_byte(raw, span.start),
                phase_type: phase
                    .get("type")
                    .and_then(toml_edit::Item::as_str)
                    .map(str::to_owned),
            })
        })
        .collect()
}

fn line_index_at_byte(raw: &str, byte: usize) -> usize {
    raw.as_bytes()[..byte]
        .iter()
        .filter(|byte| **byte == b'\n')
        .count()
}

fn render_model_resource_region(spec: &AppSpec) -> String {
    if !spec.has_solver_config_resources() {
        return String::new();
    }

    let mut sections = Vec::new();
    for group in spec.solver_config_scalar_groups() {
        sections.push(render_owned_phase(
            GeneratedPhaseKind::ScalarGroupConstruction,
            &group.name,
            &scalar_group_construction_phase(group),
        ));
    }
    for group in spec.solver_config_scalar_groups() {
        sections.push(render_owned_phase(
            GeneratedPhaseKind::ScalarGroupSearch,
            &group.name,
            &scalar_group_local_search_phase(group),
        ));
    }
    for repair in spec.solver_config_conflict_repairs() {
        sections.push(render_owned_phase(
            GeneratedPhaseKind::ConflictRepairSearch,
            &repair.constraint,
            &conflict_repair_local_search_phase(repair),
        ));
    }

    format!(
        "{BEGIN_REGION}\n{}\n{END_REGION}\n",
        sections.join("\n\n").trim_end()
    )
}

fn render_owned_phase(
    phase_kind: GeneratedPhaseKind,
    id: &str,
    table: &toml::map::Map<String, toml::Value>,
) -> String {
    format!(
        "{}\n{}",
        owner_marker(phase_kind, id),
        render_phase_table(table)
    )
}

fn owner_marker(phase_kind: GeneratedPhaseKind, id: &str) -> String {
    match phase_kind {
        GeneratedPhaseKind::ScalarGroupConstruction => {
            format!("# @solverforge:owner {SCALAR_GROUP_KIND} {id} {CONSTRUCTION_ROLE}")
        }
        GeneratedPhaseKind::ScalarGroupSearch => {
            format!("# @solverforge:owner {SCALAR_GROUP_KIND} {id} {SEARCH_ROLE}")
        }
        GeneratedPhaseKind::ConflictRepairSearch => {
            format!("# @solverforge:owner {CONFLICT_REPAIR_KIND} {id} {SEARCH_ROLE}")
        }
    }
}

fn collect_top_level_phase_array(
    phases: &toml::Value,
    region: Option<&ManagedRegion>,
    index: &mut SolverConfigIndex,
) -> CliResult {
    let phases = phases
        .as_array()
        .ok_or_else(|| CliError::general("solver.toml `phases` is not an array"))?;
    for (phase_index, phase) in phases.iter().enumerate() {
        let phase_path = format!("phases[{phase_index}]");
        let phase = phase.as_table().ok_or_else(|| {
            CliError::general(format!("solver.toml `{phase_path}` is not a TOML table"))
        })?;
        let owner = region.and_then(|region| region.phase_owners.get(&phase_index).cloned());
        collect_phase_refs(phase, &phase_path, index, owner)?;
    }
    Ok(())
}

fn collect_phase_array(
    phases: &toml::Value,
    path: &str,
    index: &mut SolverConfigIndex,
    owner: Option<SolverConfigOwner>,
) -> CliResult {
    let phases = phases
        .as_array()
        .ok_or_else(|| CliError::general(format!("solver.toml `{path}` is not an array")))?;
    for (phase_index, phase) in phases.iter().enumerate() {
        let phase_path = format!("{path}[{phase_index}]");
        let phase = phase.as_table().ok_or_else(|| {
            CliError::general(format!("solver.toml `{phase_path}` is not a TOML table"))
        })?;
        collect_phase_refs(phase, &phase_path, index, owner.clone())?;
    }
    Ok(())
}

fn collect_phase_refs(
    phase: &toml::map::Map<String, toml::Value>,
    path: &str,
    index: &mut SolverConfigIndex,
    owner: Option<SolverConfigOwner>,
) -> CliResult {
    match table_string(phase, "type").as_deref() {
        Some("construction_heuristic") => {
            if let Some(group_name) = table_string(phase, "group_name") {
                validate_owner(
                    owner.as_ref(),
                    SolverConfigOwnerKind::ScalarGroup,
                    SolverConfigOwnerRole::Construction,
                    &group_name,
                    &format!("{path}.group_name"),
                )?;
                index.add_scalar_group_construction_ref(
                    &group_name,
                    format!("{path}.group_name"),
                    owner,
                );
            }
        }
        Some("local_search") => {
            if let Some(selector) = phase.get("move_selector") {
                collect_selector_refs(
                    selector,
                    &format!("{path}.move_selector"),
                    index,
                    owner.clone(),
                )?;
            }
            if let Some(neighborhoods) = phase.get("neighborhoods") {
                let neighborhoods = neighborhoods.as_array().ok_or_else(|| {
                    CliError::general(format!(
                        "solver.toml `{path}.neighborhoods` is not an array"
                    ))
                })?;
                for (selector_index, selector) in neighborhoods.iter().enumerate() {
                    collect_selector_refs(
                        selector,
                        &format!("{path}.neighborhoods[{selector_index}]"),
                        index,
                        owner.clone(),
                    )?;
                }
            }
        }
        Some("partitioned_search") => {
            if let Some(child_phases) = phase.get("child_phases") {
                collect_phase_array(child_phases, &format!("{path}.child_phases"), index, owner)?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn collect_selector_refs(
    selector: &toml::Value,
    path: &str,
    index: &mut SolverConfigIndex,
    owner: Option<SolverConfigOwner>,
) -> CliResult {
    let selector = selector
        .as_table()
        .ok_or_else(|| CliError::general(format!("solver.toml `{path}` is not a TOML table")))?;
    match table_string(selector, "type").as_deref() {
        Some("grouped_scalar_move_selector") => {
            if let Some(group_name) = table_string(selector, "group_name") {
                validate_owner(
                    owner.as_ref(),
                    SolverConfigOwnerKind::ScalarGroup,
                    SolverConfigOwnerRole::Search,
                    &group_name,
                    &format!("{path}.group_name"),
                )?;
                index.add_scalar_group_search_ref(&group_name, format!("{path}.group_name"), owner);
            }
        }
        Some("conflict_repair_move_selector" | "compound_conflict_repair_move_selector") => {
            collect_selector_constraint_refs(selector, path, index, owner)?;
        }
        Some("limited_neighborhood") => {
            if let Some(child) = selector.get("selector") {
                collect_selector_refs(child, &format!("{path}.selector"), index, owner)?;
            }
        }
        Some("union_move_selector" | "cartesian_product_move_selector") => {
            if let Some(children) = selector.get("selectors") {
                let children = children.as_array().ok_or_else(|| {
                    CliError::general(format!("solver.toml `{path}.selectors` is not an array"))
                })?;
                for (child_index, child) in children.iter().enumerate() {
                    collect_selector_refs(
                        child,
                        &format!("{path}.selectors[{child_index}]"),
                        index,
                        owner.clone(),
                    )?;
                }
            }
        }
        _ => {}
    }
    Ok(())
}

fn collect_selector_constraint_refs(
    selector: &toml::map::Map<String, toml::Value>,
    path: &str,
    index: &mut SolverConfigIndex,
    owner: Option<SolverConfigOwner>,
) -> CliResult {
    let Some(constraints) = selector.get("constraints") else {
        return Ok(());
    };
    let constraints = constraints.as_array().ok_or_else(|| {
        CliError::general(format!("solver.toml `{path}.constraints` is not an array"))
    })?;
    for constraint in constraints {
        let constraint = constraint.as_str().ok_or_else(|| {
            CliError::general(format!(
                "solver.toml `{path}.constraints` contains a non-string value"
            ))
        })?;
        validate_owner(
            owner.as_ref(),
            SolverConfigOwnerKind::ConflictRepair,
            SolverConfigOwnerRole::Search,
            constraint,
            &format!("{path}.constraints"),
        )?;
        index.add_conflict_repair_constraint_ref(
            constraint,
            format!("{path}.constraints"),
            owner.clone(),
        );
    }
    Ok(())
}

fn validate_owner(
    owner: Option<&SolverConfigOwner>,
    kind: SolverConfigOwnerKind,
    role: SolverConfigOwnerRole,
    id: &str,
    path: &str,
) -> CliResult {
    let Some(owner) = owner else {
        return Ok(());
    };
    if owner.kind == kind && owner.role == role && owner.id == id {
        return Ok(());
    }
    Err(CliError::general(format!(
        "solver.toml managed owner '{}' for '{}' does not match reference '{}' at {}",
        owner.kind.as_str(),
        owner.id,
        id,
        path
    )))
}

fn scalar_group_construction_phase(group: &ScalarGroupSpec) -> toml::map::Map<String, toml::Value> {
    let mut phase = toml::map::Map::new();
    phase.insert(
        "type".to_string(),
        toml::Value::String("construction_heuristic".to_string()),
    );
    phase.insert(
        "construction_heuristic_type".to_string(),
        toml::Value::String("first_fit".to_string()),
    );
    phase.insert(
        "construction_obligation".to_string(),
        toml::Value::String("assign_when_candidate_exists".to_string()),
    );
    phase.insert(
        "group_name".to_string(),
        toml::Value::String(group.name.clone()),
    );
    insert_optional_usize(
        &mut phase,
        "value_candidate_limit",
        group.limits.value_candidate_limit,
    );
    insert_optional_usize(
        &mut phase,
        "group_candidate_limit",
        group.limits.group_candidate_limit,
    );
    phase
}

fn scalar_group_local_search_phase(group: &ScalarGroupSpec) -> toml::map::Map<String, toml::Value> {
    let mut selector = toml::map::Map::new();
    selector.insert(
        "type".to_string(),
        toml::Value::String("grouped_scalar_move_selector".to_string()),
    );
    selector.insert(
        "group_name".to_string(),
        toml::Value::String(group.name.clone()),
    );
    selector.insert(
        "require_hard_improvement".to_string(),
        toml::Value::Boolean(true),
    );
    insert_optional_usize(
        &mut selector,
        "value_candidate_limit",
        group.limits.value_candidate_limit,
    );
    insert_optional_usize(
        &mut selector,
        "max_moves_per_step",
        group.limits.max_moves_per_step,
    );

    let mut phase = toml::map::Map::new();
    phase.insert(
        "type".to_string(),
        toml::Value::String("local_search".to_string()),
    );
    phase.insert("move_selector".to_string(), toml::Value::Table(selector));
    phase
}

fn conflict_repair_local_search_phase(
    repair: &ConflictRepairSpec,
) -> toml::map::Map<String, toml::Value> {
    let selector_type = if repair.selector == "conflict" {
        "conflict_repair_move_selector"
    } else {
        "compound_conflict_repair_move_selector"
    };
    let mut selector = toml::map::Map::new();
    selector.insert(
        "type".to_string(),
        toml::Value::String(selector_type.to_string()),
    );
    selector.insert(
        "constraints".to_string(),
        toml::Value::Array(vec![toml::Value::String(repair.constraint.clone())]),
    );
    selector.insert(
        "require_hard_improvement".to_string(),
        toml::Value::Boolean(true),
    );
    if repair.include_soft_matches {
        selector.insert(
            "include_soft_matches".to_string(),
            toml::Value::Boolean(true),
        );
    }
    insert_optional_usize(
        &mut selector,
        "max_matches_per_step",
        repair.max_matches_per_step,
    );
    insert_optional_usize(
        &mut selector,
        "max_repairs_per_match",
        repair.max_repairs_per_match,
    );
    insert_optional_usize(
        &mut selector,
        "max_moves_per_step",
        repair.max_moves_per_step,
    );

    let mut phase = toml::map::Map::new();
    phase.insert(
        "type".to_string(),
        toml::Value::String("local_search".to_string()),
    );
    phase.insert("move_selector".to_string(), toml::Value::Table(selector));
    phase
}

fn render_phase_table(table: &toml::map::Map<String, toml::Value>) -> String {
    let mut root = toml::map::Map::new();
    root.insert(
        "phases".to_string(),
        toml::Value::Array(vec![toml::Value::Table(table.clone())]),
    );
    toml::to_string_pretty(&toml::Value::Table(root))
        .expect("generated solver config should serialize")
        .trim()
        .to_string()
}

fn blocked_solver_config_generate(
    kind: &str,
    name: &str,
    locations: &[SolverConfigLocation],
) -> CliError {
    CliError::with_hint(
        format!(
            "cannot generate {kind} '{name}' solver config because user-authored solver.toml already references it at {}",
            display_locations(locations)
        ),
        "remove or update those solver.toml references first",
    )
}

fn blocked_solver_config_destroy(
    kind: &str,
    name: &str,
    locations: &[SolverConfigLocation],
) -> CliError {
    CliError::with_hint(
        format!(
            "cannot destroy {kind} '{name}' because user-authored solver.toml still references it at {}",
            display_locations(locations)
        ),
        "remove or update those solver.toml references first",
    )
}

pub(crate) fn display_locations(locations: &[SolverConfigLocation]) -> String {
    locations
        .iter()
        .map(|location| location.path.as_str())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>()
        .join(", ")
}

fn keys_for_refs(
    refs: &BTreeMap<String, Vec<SolverConfigRef>>,
    filter: RefOwnerFilter,
) -> BTreeSet<String> {
    refs.iter()
        .filter(|(_, refs)| {
            refs.iter()
                .any(|reference| owner_matches(reference, filter))
        })
        .map(|(id, _)| id.clone())
        .collect()
}

fn locations_for_refs(
    refs: &BTreeMap<String, Vec<SolverConfigRef>>,
    id: &str,
    filter: RefOwnerFilter,
) -> Vec<SolverConfigLocation> {
    refs.get(id)
        .map(|refs| {
            refs.iter()
                .filter(|reference| owner_matches(reference, filter))
                .map(|reference| reference.location.clone())
                .collect()
        })
        .unwrap_or_default()
}

fn owner_matches(reference: &SolverConfigRef, filter: RefOwnerFilter) -> bool {
    match filter {
        RefOwnerFilter::Any => true,
        RefOwnerFilter::Generated => reference.owner.is_some(),
        RefOwnerFilter::User => reference.owner.is_none(),
    }
}

impl SolverConfigOwnerKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::ScalarGroup => SCALAR_GROUP_KIND,
            Self::ConflictRepair => CONFLICT_REPAIR_KIND,
        }
    }
}

fn insert_optional_usize(
    table: &mut toml::map::Map<String, toml::Value>,
    key: &str,
    value: Option<usize>,
) {
    if let Some(value) = value {
        table.insert(key.to_string(), toml::Value::Integer(value as i64));
    }
}

fn table_string(table: &toml::map::Map<String, toml::Value>, key: &str) -> Option<String> {
    table
        .get(key)
        .and_then(toml::Value::as_str)
        .map(ToString::to_string)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app_spec::{ScalarGroupLimitsSpec, ScalarGroupTargetSpec};

    fn doc(raw: &str) -> SolverConfigDocument {
        SolverConfigDocument::from_toml_str(PathBuf::from(CONFIG_PATH), raw)
            .expect("solver config should parse")
    }

    fn spec(groups: Vec<ScalarGroupSpec>, repairs: Vec<ConflictRepairSpec>) -> AppSpec {
        AppSpec {
            scalar_groups: groups,
            conflict_repairs: repairs,
            ..AppSpec::default()
        }
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
            solver_config: true,
            enabled: true,
            ..ScalarGroupSpec::default()
        }
    }

    fn candidate_group(name: &str) -> ScalarGroupSpec {
        ScalarGroupSpec {
            name: name.to_string(),
            kind: "candidates".to_string(),
            limits: ScalarGroupLimitsSpec {
                value_candidate_limit: Some(8),
                group_candidate_limit: Some(4),
                max_moves_per_step: Some(64),
                ..ScalarGroupLimitsSpec::default()
            },
            solver_config: true,
            enabled: true,
            ..ScalarGroupSpec::default()
        }
    }

    fn conflict_repair(name: &str) -> ConflictRepairSpec {
        ConflictRepairSpec {
            constraint: name.to_string(),
            provider: format!("repair_{name}"),
            selector: "compound".to_string(),
            solver_config: true,
            enabled: true,
            ..ConflictRepairSpec::default()
        }
    }

    #[test]
    fn candidate_scalar_group_gets_managed_construction_and_search_config() {
        let planned = doc("")
            .plan_sync_model_resources(&spec(vec![candidate_group("paired")], Vec::new()))
            .unwrap();
        let index = planned.index().unwrap();

        assert!(index.has_scalar_group_construction_ref("paired"));
        assert!(index.has_scalar_group_search_ref("paired"));
        assert!(index
            .generated_scalar_group_construction_refs()
            .contains("paired"));
        assert!(index
            .generated_scalar_group_search_refs()
            .contains("paired"));
        let rendered = planned.to_toml_string().unwrap();
        assert!(rendered.contains(BEGIN_REGION));
        assert!(rendered.contains("# @solverforge:owner scalar-group paired construction"));
        assert!(rendered.contains("# @solverforge:owner scalar-group paired search"));
        assert!(rendered.contains("construction_obligation = \"assign_when_candidate_exists\""));
        assert!(rendered.contains("group_candidate_limit = 4"));
    }

    #[test]
    fn generated_region_inserts_after_user_construction_before_user_local_search() {
        let planned = doc(r#"
[[phases]]
type = "construction_heuristic"
construction_heuristic_type = "first_fit"

[[phases]]
type = "local_search"
"#)
        .plan_sync_model_resources(&spec(vec![assignment_group("paired")], Vec::new()))
        .unwrap();
        let rendered = planned.to_toml_string().unwrap();

        let user_construction = rendered
            .find("construction_heuristic_type = \"first_fit\"")
            .unwrap();
        let generated_region = rendered.find(BEGIN_REGION).unwrap();
        let user_search = rendered.rfind("type = \"local_search\"").unwrap();
        assert!(user_construction < generated_region);
        assert!(generated_region < user_search);
    }

    #[test]
    fn generated_region_inserts_after_user_construction_with_inline_type_comment() {
        let planned = doc(r#"
[[phases]]
type = "construction_heuristic" # preserved user note
construction_heuristic_type = "first_fit"

[[phases]]
type = "local_search"
"#)
        .plan_sync_model_resources(&spec(vec![assignment_group("paired")], Vec::new()))
        .unwrap();
        let rendered = planned.to_toml_string().unwrap();

        let user_construction = rendered
            .find("construction_heuristic_type = \"first_fit\"")
            .unwrap();
        let generated_region = rendered.find(BEGIN_REGION).unwrap();
        let user_search = rendered.rfind("type = \"local_search\"").unwrap();
        assert!(user_construction < generated_region);
        assert!(generated_region < user_search);
    }

    #[test]
    fn generated_region_uses_parsed_phase_type_values() {
        let planned = doc(r#"
[[phases]]
type = 'construction_heuristic'
construction_heuristic_type = "first_fit"

[[phases]]
type = 'local_search'
"#)
        .plan_sync_model_resources(&spec(vec![assignment_group("paired")], Vec::new()))
        .unwrap();
        let rendered = planned.to_toml_string().unwrap();

        let user_construction = rendered
            .find("construction_heuristic_type = \"first_fit\"")
            .unwrap();
        let generated_region = rendered.find(BEGIN_REGION).unwrap();
        let user_search = rendered.rfind("type = 'local_search'").unwrap();
        assert!(user_construction < generated_region);
        assert!(generated_region < user_search);
    }

    #[test]
    fn sync_is_idempotent() {
        let spec = spec(
            vec![assignment_group("paired")],
            vec![conflict_repair("all_assigned")],
        );
        let first = doc("")
            .plan_sync_model_resources(&spec)
            .unwrap()
            .to_toml_string()
            .unwrap();
        let second = doc(&first)
            .plan_sync_model_resources(&spec)
            .unwrap()
            .to_toml_string()
            .unwrap();

        assert_eq!(first, second);
    }

    #[test]
    fn sync_removes_generated_refs_for_removed_resources() {
        let base = doc("")
            .plan_sync_model_resources(&spec(
                vec![assignment_group("remove_me"), assignment_group("keep_me")],
                Vec::new(),
            ))
            .unwrap();
        let planned = base
            .plan_sync_model_resources(&spec(vec![assignment_group("keep_me")], Vec::new()))
            .unwrap();
        let rendered = planned.to_toml_string().unwrap();
        let index = planned.index().unwrap();

        assert!(!index.scalar_group_refs().contains("remove_me"));
        assert!(index
            .generated_scalar_group_construction_refs()
            .contains("keep_me"));
        assert!(!rendered.contains("remove_me"));
        assert!(rendered.contains("keep_me"));
    }

    #[test]
    fn sync_blocks_when_user_authored_scalar_group_ref_already_exists() {
        let planned = doc(r#"
[[phases]]
type = "local_search"

[phases.move_selector]
type = "grouped_scalar_move_selector"
group_name = "paired"
"#)
        .plan_sync_model_resources(&spec(vec![candidate_group("paired")], Vec::new()));

        let err = planned.unwrap_err();
        assert!(err
            .to_string()
            .contains("phases[0].move_selector.group_name"));
    }

    #[test]
    fn user_authored_refs_for_skip_solver_config_groups_are_allowed() {
        let mut group = assignment_group("manual");
        group.solver_config = false;
        let planned = doc(r#"
[[phases]]
type = "local_search"

[phases.move_selector]
type = "grouped_scalar_move_selector"
group_name = "manual"
"#)
        .plan_sync_model_resources(&spec(vec![group], Vec::new()))
        .unwrap();

        assert!(planned
            .index()
            .unwrap()
            .user_scalar_group_locations("manual")
            .iter()
            .any(|location| location.path == "phases[0].move_selector.group_name"));
    }

    #[test]
    fn destroy_blocker_sees_nested_user_scalar_group_ref() {
        let planned = doc(r#"
[[phases]]
type = "local_search"

[phases.move_selector]
type = "union_move_selector"

[[phases.move_selector.selectors]]
type = "grouped_scalar_move_selector"
group_name = "paired"
"#)
        .ensure_no_user_scalar_group_refs("paired");

        let err = planned.unwrap_err();
        assert!(err
            .to_string()
            .contains("phases[0].move_selector.selectors[0].group_name"));
    }

    #[test]
    fn conflict_repair_uses_generated_region_instead_of_shared_selector_mutation() {
        let base = doc("")
            .plan_sync_model_resources(&spec(
                Vec::new(),
                vec![conflict_repair("a"), conflict_repair("b")],
            ))
            .unwrap();

        let planned = base
            .plan_sync_model_resources(&spec(Vec::new(), vec![conflict_repair("b")]))
            .unwrap();
        let rendered = planned.to_toml_string().unwrap();
        let index = planned.index().unwrap();

        assert!(!index.conflict_repair_refs().contains("a"));
        assert!(index.generated_conflict_repair_refs().contains("b"));
        assert!(!rendered.contains("conflict-repair a"));
        assert!(rendered.contains("# @solverforge:owner conflict-repair b search"));
    }

    #[test]
    fn destroy_blocker_sees_unmanaged_conflict_repair_ref() {
        let planned = doc(r#"
[[phases]]
type = "local_search"

[phases.move_selector]
type = "compound_conflict_repair_move_selector"
constraints = ["a", "b"]
require_hard_improvement = true
max_moves_per_step = 16
"#)
        .ensure_no_user_conflict_repair_refs("a");

        let err = planned.unwrap_err();
        assert!(err
            .to_string()
            .contains("phases[0].move_selector.constraints"));
    }

    #[test]
    fn destroy_blocker_sees_nested_conflict_repair_ref() {
        let planned = doc(r#"
[[phases]]
type = "local_search"

[phases.move_selector]
type = "limited_neighborhood"
selected_count_limit = 3

[phases.move_selector.selector]
type = "compound_conflict_repair_move_selector"
constraints = ["a"]
"#)
        .ensure_no_user_conflict_repair_refs("a");

        let err = planned.unwrap_err();
        assert!(err
            .to_string()
            .contains("phases[0].move_selector.selector.constraints"));
    }

    #[test]
    fn legacy_per_resource_managed_blocks_are_rejected() {
        let err = SolverConfigDocument::from_toml_str(
            PathBuf::from(CONFIG_PATH),
            r#"
# @solverforge:begin solver-config scalar-group paired
# @solverforge:end solver-config scalar-group paired
"#,
        )
        .unwrap_err();

        assert!(err
            .to_string()
            .contains("unsupported per-resource solver config marker"));
    }

    #[test]
    fn owner_markers_outside_region_are_rejected() {
        let err = SolverConfigDocument::from_toml_str(
            PathBuf::from(CONFIG_PATH),
            r#"
# @solverforge:owner scalar-group paired construction
"#,
        )
        .unwrap_err();

        assert!(err.to_string().contains("outside the managed region"));
    }

    #[test]
    fn owner_mismatch_is_rejected() {
        let err = SolverConfigDocument::from_toml_str(
            PathBuf::from(CONFIG_PATH),
            r#"
# @solverforge:begin solver-config
# @solverforge:owner scalar-group paired construction
[[phases]]
type = "construction_heuristic"
group_name = "other"
# @solverforge:end solver-config
"#,
        )
        .unwrap_err();

        assert!(err.to_string().contains("does not match reference"));
    }

    #[test]
    fn candidate_trace_requires_positive_capacity() {
        for raw in [
            "[candidate_trace]\n",
            "[candidate_trace]\nmax_entries = 0\n",
            "candidate_trace = \"enabled\"\n",
        ] {
            let err = validate_current_settings(raw).expect_err("invalid trace config should fail");
            assert!(err.to_string().contains("candidate_trace"));
        }
    }

    #[test]
    fn candidate_trace_accepts_bounded_capacity() {
        validate_current_settings("[candidate_trace]\nmax_entries = 128\n")
            .expect("positive trace capacity should validate");
    }
}
