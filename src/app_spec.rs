use serde::{Deserialize, Serialize};
use serde_json::json;
use std::fs;
use std::path::Path;

use crate::commands::generate_constraint::domain::{list_constraints, parse_domain};
use crate::error::{CliError, CliResult};
use crate::scalar_variable_hooks::ScalarVariableHooks;

const APP_SPEC_PATH: &str = "solverforge.app.toml";
const UI_MODEL_PATH: &str = "static/generated/ui-model.json";

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AppSpec {
    #[serde(default)]
    pub app: AppMeta,
    #[serde(default)]
    pub runtime: RuntimeMeta,
    #[serde(default)]
    pub demo: DemoMeta,
    #[serde(default)]
    pub solution: SolutionMeta,
    #[serde(default)]
    pub facts: Vec<CollectionSpec>,
    #[serde(default)]
    pub entities: Vec<CollectionSpec>,
    #[serde(default)]
    pub variables: Vec<VariableSpec>,
    #[serde(default)]
    pub constraints: Vec<ConstraintSpec>,
    #[serde(default)]
    pub scalar_groups: Vec<ScalarGroupSpec>,
    #[serde(default)]
    pub conflict_repairs: Vec<ConflictRepairSpec>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AppMeta {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub starter: String,
    #[serde(default)]
    pub shell: String,
    #[serde(default)]
    pub cli_version: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RuntimeMeta {
    #[serde(default)]
    pub target: String,
    #[serde(default)]
    pub runtime_source: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub ui_source: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SolutionMeta {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub score: String,
    #[serde(default)]
    pub scalar_groups_path: String,
    #[serde(default)]
    pub conflict_repairs_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DemoMeta {
    #[serde(default = "default_demo_size")]
    pub default_size: String,
    #[serde(default = "default_available_demo_sizes")]
    pub available_sizes: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CollectionSpec {
    pub name: String,
    pub plural: String,
    pub kind: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct VariableSpec {
    pub entity: String,
    pub entity_plural: String,
    pub field: String,
    pub kind: String,
    #[serde(default)]
    pub range: String,
    #[serde(default)]
    pub elements: String,
    #[serde(default)]
    pub allows_unassigned: bool,
    #[serde(default, flatten)]
    pub scalar_hooks: ScalarVariableHooks,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ConstraintSpec {
    pub name: String,
    pub module: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ScalarGroupSpec {
    pub name: String,
    pub kind: String,
    #[serde(default)]
    pub targets: Vec<ScalarGroupTargetSpec>,
    #[serde(default)]
    pub candidate_provider: String,
    #[serde(default)]
    pub assignment_hooks: ScalarGroupAssignmentHooks,
    #[serde(default)]
    pub limits: ScalarGroupLimitsSpec,
    #[serde(default = "default_true")]
    pub solver_config: bool,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ScalarGroupTargetSpec {
    pub entity: String,
    pub entity_plural: String,
    pub field: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ScalarGroupAssignmentHooks {
    #[serde(default)]
    pub required_entity: String,
    #[serde(default)]
    pub capacity_key: String,
    #[serde(default)]
    pub assignment_rule: String,
    #[serde(default)]
    pub position_key: String,
    #[serde(default)]
    pub sequence_key: String,
    #[serde(default)]
    pub entity_order: String,
    #[serde(default)]
    pub value_order: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ScalarGroupLimitsSpec {
    #[serde(default)]
    pub value_candidate_limit: Option<usize>,
    #[serde(default)]
    pub group_candidate_limit: Option<usize>,
    #[serde(default)]
    pub max_moves_per_step: Option<usize>,
    #[serde(default)]
    pub max_augmenting_depth: Option<usize>,
    #[serde(default)]
    pub max_rematch_size: Option<usize>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ConflictRepairSpec {
    pub constraint: String,
    pub provider: String,
    #[serde(default)]
    pub selector: String,
    #[serde(default)]
    pub max_matches_per_step: Option<usize>,
    #[serde(default)]
    pub max_repairs_per_match: Option<usize>,
    #[serde(default)]
    pub max_moves_per_step: Option<usize>,
    #[serde(default)]
    pub include_soft_matches: bool,
    #[serde(default = "default_true")]
    pub solver_config: bool,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

fn default_true() -> bool {
    true
}

fn variable_source_collection(variable: &VariableSpec) -> CliResult<(&str, &str)> {
    match variable.kind.as_str() {
        "scalar" => Ok(("scalar", &variable.range)),
        "list" => Ok(("list", &variable.elements)),
        other => Err(CliError::general(format!(
            "unsupported variable kind '{}' in {}",
            other, APP_SPEC_PATH
        ))),
    }
}

fn default_demo_size() -> String {
    "standard".to_string()
}

fn default_available_demo_sizes() -> Vec<String> {
    vec![
        "small".to_string(),
        "standard".to_string(),
        "large".to_string(),
    ]
}

impl Default for DemoMeta {
    fn default() -> Self {
        Self {
            default_size: default_demo_size(),
            available_sizes: default_available_demo_sizes(),
        }
    }
}

pub fn load() -> CliResult<AppSpec> {
    let path = Path::new(APP_SPEC_PATH);
    let raw = fs::read_to_string(path).map_err(|e| CliError::IoError {
        context: format!("failed to read {}", APP_SPEC_PATH),
        source: e,
    })?;
    toml::from_str(&raw)
        .map_err(|e| CliError::general(format!("failed to parse {}: {}", APP_SPEC_PATH, e)))
}

pub fn save(spec: &AppSpec) -> CliResult {
    let raw = toml::to_string_pretty(spec)
        .map_err(|e| CliError::general(format!("failed to serialize {}: {}", APP_SPEC_PATH, e)))?;
    fs::write(APP_SPEC_PATH, raw).map_err(|e| CliError::IoError {
        context: format!("failed to write {}", APP_SPEC_PATH),
        source: e,
    })?;
    Ok(())
}

pub fn sync_from_project() -> CliResult {
    let mut spec = if Path::new(APP_SPEC_PATH).exists() {
        load()?
    } else {
        AppSpec::default()
    };

    match parse_domain() {
        Ok(domain) => {
            let inferred_facts: Vec<CollectionSpec> = domain
                .facts
                .iter()
                .map(|fact| CollectionSpec {
                    name: snake_case(&fact.item_type),
                    plural: fact.field_name.clone(),
                    kind: "problem_fact".to_string(),
                })
                .collect();
            spec.solution.name = domain.solution_type;
            spec.solution.score = domain.score_type;
            spec.solution.scalar_groups_path = domain.scalar_groups_path.unwrap_or_default();
            spec.solution.conflict_repairs_path = domain.conflict_repairs_path.unwrap_or_default();
            spec.entities = domain
                .entities
                .iter()
                .map(|entity| CollectionSpec {
                    name: snake_case(&entity.item_type),
                    plural: entity.field_name.clone(),
                    kind: "planning_entity".to_string(),
                })
                .collect();
            spec.facts = inferred_facts.clone();
            let default_fact_plural = if inferred_facts.len() == 1 {
                inferred_facts[0].plural.clone()
            } else {
                String::new()
            };
            let mut variables = Vec::new();
            for entity in &domain.entities {
                let entity_name = snake_case(&entity.item_type);
                let entity_plural = entity.field_name.clone();
                for var in &entity.scalar_vars {
                    variables.push(VariableSpec {
                        entity: entity_name.clone(),
                        entity_plural: entity_plural.clone(),
                        field: var.field.clone(),
                        kind: "scalar".to_string(),
                        range: if var.value_range_provider.is_empty() {
                            default_fact_plural.clone()
                        } else {
                            var.value_range_provider.clone()
                        },
                        elements: String::new(),
                        allows_unassigned: var.allows_unassigned,
                        scalar_hooks: var.hooks.clone(),
                        enabled: true,
                    });
                }
                for var in &entity.list_vars {
                    variables.push(VariableSpec {
                        entity: entity_name.clone(),
                        entity_plural: entity_plural.clone(),
                        field: var.field.clone(),
                        kind: "list".to_string(),
                        range: String::new(),
                        elements: if var.element_collection.is_empty() {
                            default_fact_plural.clone()
                        } else {
                            var.element_collection.clone()
                        },
                        allows_unassigned: false,
                        scalar_hooks: ScalarVariableHooks::default(),
                        enabled: true,
                    });
                }
            }
            spec.variables = variables;
        }
        Err(err) if err.contains("requires exactly one #[planning_solution]") => {}
        Err(err) => return Err(CliError::general(err)),
    }

    spec.constraints = list_constraints(Path::new("src/constraints"))
        .into_iter()
        .map(|name| ConstraintSpec {
            module: name.clone(),
            name,
            enabled: true,
        })
        .collect();

    normalize_demo_meta(&mut spec.demo);

    save(&spec)?;
    write_ui_model(&spec)
}

pub fn set_demo_size(size: &str) -> CliResult {
    let mut spec = load()?;
    normalize_demo_meta(&mut spec.demo);
    spec.demo.default_size = size.to_string();
    if !spec.demo.available_sizes.iter().any(|value| value == size) {
        spec.demo.available_sizes.push(size.to_string());
    }
    normalize_demo_meta(&mut spec.demo);
    save(&spec)?;
    write_ui_model(&spec)
}

fn normalize_demo_meta(demo: &mut DemoMeta) {
    if demo.default_size.is_empty() {
        demo.default_size = default_demo_size();
    }
    if demo.available_sizes.is_empty() {
        demo.available_sizes = default_available_demo_sizes();
    }
    if !demo
        .available_sizes
        .iter()
        .any(|value| value == &demo.default_size)
    {
        demo.available_sizes.push(demo.default_size.clone());
    }
    demo.available_sizes.sort();
    demo.available_sizes.dedup();
    let mut ordered = Vec::new();
    for canonical in default_available_demo_sizes() {
        if demo.available_sizes.iter().any(|value| value == &canonical) {
            ordered.push(canonical);
        }
    }
    for value in &demo.available_sizes {
        if !ordered.iter().any(|existing| existing == value) {
            ordered.push(value.clone());
        }
    }
    demo.available_sizes = ordered;
}

fn write_ui_model(spec: &AppSpec) -> CliResult {
    if !uses_web_shell(spec) {
        return Ok(());
    }

    let path = Path::new(UI_MODEL_PATH);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| CliError::IoError {
            context: format!("failed to create {}", parent.display()),
            source: e,
        })?;
    }

    let entities = spec
        .entities
        .iter()
        .map(|entry| json!({"name": entry.name, "plural": entry.plural, "label": title_case(&entry.name)}))
        .collect::<Vec<_>>();
    let facts = spec
        .facts
        .iter()
        .map(|entry| json!({"name": entry.name, "plural": entry.plural, "label": title_case(&entry.name)}))
        .collect::<Vec<_>>();
    let constraints = spec
        .constraints
        .iter()
        .filter(|c| c.enabled)
        .map(|c| c.name.clone())
        .collect::<Vec<_>>();
    let views = spec
        .variables
        .iter()
        .filter(|v| v.enabled)
        .map(|variable| {
            let (kind, source_collection) = variable_source_collection(variable)?;
            let source_plural = resolve_collection_plural(&spec.facts, source_collection);
            Ok(json!({
                "id": format!("{}-{}", variable.entity, variable.field),
                "kind": kind,
                "label": format!("{} · {}", title_case(&variable.entity), variable.field),
                "entity": variable.entity,
                "entityPlural": variable.entity_plural,
                "sourcePlural": source_plural,
                "variableField": variable.field,
                "allowsUnassigned": variable.allows_unassigned,
                "scalarHooks": scalar_hook_metadata_json(&variable.scalar_hooks)
            }))
        })
        .collect::<CliResult<Vec<_>>>()?;

    let raw = serde_json::to_string_pretty(&json!({
        "entities": entities,
        "facts": facts,
        "constraints": constraints,
        "views": views,
        "scalarGroups": spec
            .scalar_groups
            .iter()
            .filter(|group| group.enabled)
            .map(|group| json!({
                "name": group.name,
                "kind": group.kind,
                "targets": group.targets,
                "solverConfig": group.solver_config
            }))
            .collect::<Vec<_>>(),
        "conflictRepairs": spec
            .conflict_repairs
            .iter()
            .filter(|repair| repair.enabled)
            .map(|repair| json!({
                "constraint": repair.constraint,
                "provider": repair.provider,
                "selector": repair.selector,
                "solverConfig": repair.solver_config
            }))
            .collect::<Vec<_>>()
    }))
    .map_err(|e| CliError::general(format!("failed to serialize {}: {}", UI_MODEL_PATH, e)))?;

    fs::write(path, raw).map_err(|e| CliError::IoError {
        context: format!("failed to write {}", UI_MODEL_PATH),
        source: e,
    })?;
    Ok(())
}

pub(crate) fn uses_web_shell(spec: &AppSpec) -> bool {
    spec.app.shell.is_empty() || spec.app.shell == "web"
}

fn scalar_hook_metadata_json(hooks: &ScalarVariableHooks) -> serde_json::Value {
    let mut value = serde_json::Map::new();
    for (name, hook) in hooks.entries() {
        if let Some(hook) = hook {
            value.insert(snake_to_camel(name), json!(hook));
        }
    }
    serde_json::Value::Object(value)
}

fn snake_to_camel(name: &str) -> String {
    let mut out = String::new();
    let mut uppercase_next = false;
    for ch in name.chars() {
        if ch == '_' {
            uppercase_next = true;
        } else if uppercase_next {
            out.push(ch.to_ascii_uppercase());
            uppercase_next = false;
        } else {
            out.push(ch);
        }
    }
    out
}

fn resolve_collection_plural(collections: &[CollectionSpec], raw: &str) -> String {
    collections
        .iter()
        .find(|entry| entry.plural == raw || entry.name == raw)
        .map(|entry| entry.plural.clone())
        .unwrap_or_else(|| raw.to_string())
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

fn title_case(name: &str) -> String {
    name.split('_')
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                Some(first) => format!("{}{}", first.to_ascii_uppercase(), chars.as_str()),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::{variable_source_collection, VariableSpec};

    #[test]
    fn variable_source_collection_rejects_unknown_kinds() {
        let err = variable_source_collection(&VariableSpec {
            kind: "not_a_variable_kind".to_string(),
            ..VariableSpec::default()
        })
        .expect_err("unsupported kind should fail");

        assert_eq!(
            err.to_string(),
            "unsupported variable kind 'not_a_variable_kind' in solverforge.app.toml"
        );
    }
}
