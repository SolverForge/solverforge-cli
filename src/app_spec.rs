use serde::{Deserialize, Serialize};
use serde_json::json;
use std::fs;
use std::path::Path;

use crate::commands::generate_constraint::domain::{list_constraints, parse_domain};
use crate::error::{CliError, CliResult};

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
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AppMeta {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub starter: String,
    #[serde(default)]
    pub cli_version: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RuntimeMeta {
    #[serde(default)]
    pub target: String,
    #[serde(default)]
    pub runtime_source: String,
    #[serde(default)]
    pub ui_source: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SolutionMeta {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub score: String,
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

fn default_true() -> bool {
    true
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

    if let Some(domain) = parse_domain() {
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
            for var in &entity.planning_vars {
                variables.push(VariableSpec {
                    entity: entity_name.clone(),
                    entity_plural: entity_plural.clone(),
                    field: var.field.clone(),
                    kind: "standard".to_string(),
                    range: if var.value_range.is_empty() {
                        default_fact_plural.clone()
                    } else {
                        var.value_range.clone()
                    },
                    elements: String::new(),
                    allows_unassigned: var.allows_unassigned,
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
                    enabled: true,
                });
            }
        }
        spec.variables = variables;
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
            let source_plural = if variable.kind == "standard" {
                resolve_collection_plural(&spec.facts, &variable.range)
            } else {
                resolve_collection_plural(&spec.facts, &variable.elements)
            };
            json!({
                "id": format!("{}-{}", variable.entity, variable.field),
                "kind": variable.kind,
                "label": format!("{} · {}", title_case(&variable.entity), variable.field),
                "entity": variable.entity,
                "entityPlural": variable.entity_plural,
                "sourcePlural": source_plural,
                "variableField": variable.field,
                "allowsUnassigned": variable.allows_unassigned
            })
        })
        .collect::<Vec<_>>();

    let raw = serde_json::to_string_pretty(&json!({
        "entities": entities,
        "facts": facts,
        "constraints": constraints,
        "views": views
    }))
    .map_err(|e| CliError::general(format!("failed to serialize {}: {}", UI_MODEL_PATH, e)))?;

    fs::write(path, raw).map_err(|e| CliError::IoError {
        context: format!("failed to write {}", UI_MODEL_PATH),
        source: e,
    })?;
    Ok(())
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
