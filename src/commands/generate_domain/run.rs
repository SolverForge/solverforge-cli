use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use crate::app_spec;
use crate::commands::generate_constraint::parse_domain;
use crate::commands::generate_constraint::validate_name;
use crate::error::{CliError, CliResult};
use crate::output;

use super::generators::{generate_entity, generate_fact, generate_solution};
use super::utils::{ensure_domain_dir, find_file_for_type, snake_to_pascal, validate_score_type};
use super::wiring::{
    inject_list_variable, inject_standard_variable, remove_variable_field, replace_score_type,
    update_domain_mod, wire_collection_into_solution,
};

pub fn run_entity(
    name: &str,
    planning_variable: Option<&str>,
    fields: &[String],
    force: bool,
    pretend: bool,
) -> CliResult {
    validate_name(name)?;
    if let Some(var) = planning_variable {
        validate_name(var)?;
    }
    let parsed_fields = parse_fields(fields)?;

    let domain_dir = Path::new("src/domain");
    ensure_domain_dir(domain_dir)?;

    let pascal = snake_to_pascal(name);
    let file_path = domain_dir.join(format!("{}.rs", name));

    if file_path.exists() && !force {
        return Err(CliError::ResourceExists {
            kind: "entity",
            name: name.to_string(),
        });
    }

    let src = generate_entity(&pascal, planning_variable, &parsed_fields);

    if pretend {
        println!("Would create {}", file_path.display());
        println!("Would update src/domain/mod.rs");
        println!("Would wire into planning solution");
        return Ok(());
    }

    fs::write(&file_path, &src).map_err(|e| CliError::IoError {
        context: format!("failed to write {}", file_path.display()),
        source: e,
    })?;

    update_domain_mod(name, &pascal)?;
    wire_collection_into_solution(name, &pascal, "planning_entity_collection")?;
    let plural = super::utils::pluralize(name);
    crate::commands::sf_config::add_entity(name, &pascal, &plural)?;

    output::print_create(file_path.to_str().unwrap());
    print_diff_verbose("", &src);
    output::print_update("src/domain/mod.rs");
    sync_project_metadata()?;
    Ok(())
}

pub fn run_fact(name: &str, fields: &[String], force: bool, pretend: bool) -> CliResult {
    validate_name(name)?;
    let parsed_fields = parse_fields(fields)?;

    let domain_dir = Path::new("src/domain");
    ensure_domain_dir(domain_dir)?;

    let pascal = snake_to_pascal(name);
    let file_path = domain_dir.join(format!("{}.rs", name));

    if file_path.exists() && !force {
        return Err(CliError::ResourceExists {
            kind: "fact",
            name: name.to_string(),
        });
    }

    let src = generate_fact(&pascal, &parsed_fields);

    if pretend {
        println!("Would create {}", file_path.display());
        println!("Would update src/domain/mod.rs");
        println!("Would wire into planning solution");
        return Ok(());
    }

    fs::write(&file_path, &src).map_err(|e| CliError::IoError {
        context: format!("failed to write {}", file_path.display()),
        source: e,
    })?;

    update_domain_mod(name, &pascal)?;
    wire_collection_into_solution(name, &pascal, "problem_fact_collection")?;
    let plural = super::utils::pluralize(name);
    crate::commands::sf_config::add_fact(name, &pascal, &plural)?;

    output::print_create(file_path.to_str().unwrap());
    print_diff_verbose("", &src);
    output::print_update("src/domain/mod.rs");
    sync_project_metadata()?;
    Ok(())
}

// Parse "name:Type" field specifications.
fn parse_fields(fields: &[String]) -> CliResult<Vec<(String, String)>> {
    fields
        .iter()
        .map(|f| {
            let mut parts = f.splitn(2, ':');
            let field_name = parts.next().unwrap_or("").trim().to_string();
            let field_type = parts.next().unwrap_or("").trim().to_string();
            if field_name.is_empty() || field_type.is_empty() {
                Err(CliError::general(format!(
                    "invalid --field '{}': expected 'name:Type'",
                    f
                )))
            } else {
                Ok((field_name, field_type))
            }
        })
        .collect()
}

// Print lines of new_src prefixed with '+' when verbose.
fn print_diff_verbose(before: &str, after: &str) {
    if !output::is_verbose() {
        return;
    }
    let before_lines: Vec<&str> = if before.is_empty() {
        vec![]
    } else {
        before.lines().collect()
    };
    let after_lines: Vec<&str> = after.lines().collect();
    for line in &after_lines {
        if !before_lines.contains(line) {
            println!("+ {}", line);
        }
    }
}

pub fn run_solution(name: &str, score: &str) -> CliResult {
    validate_name(name)?;
    validate_score_type(score)?;

    let domain_dir = Path::new("src/domain");
    ensure_domain_dir(domain_dir)?;
    let pascal = snake_to_pascal(name);

    // Check if any solution already exists
    if let Some(domain) = parse_domain() {
        if is_default_scaffold()? {
            remove_default_scaffold()?;
        } else {
            return Err(CliError::with_hint(
                format!(
                    "a planning solution '{}' already exists",
                    domain.solution_type
                ),
                "use `solverforge destroy solution` then `solverforge generate solution` to replace it",
            ));
        }
    }

    let file_path = domain_dir.join(format!("{}.rs", name));
    if file_path.exists() {
        return Err(CliError::ResourceExists {
            kind: "solution file",
            name: file_path.display().to_string(),
        });
    }

    let src = generate_solution(&pascal, score);
    fs::write(&file_path, src).map_err(|e| CliError::IoError {
        context: format!("failed to write {}", file_path.display()),
        source: e,
    })?;

    update_domain_mod(name, &pascal)?;

    output::print_create(file_path.to_str().unwrap());
    output::print_update("src/domain/mod.rs");
    sync_project_metadata()?;
    Ok(())
}

pub fn run_variable(
    field: &str,
    entity: &str,
    kind: &str,
    range: Option<&str>,
    elements: Option<&str>,
    allows_unassigned: bool,
) -> CliResult {
    validate_name(field)?;

    let domain_dir = Path::new("src/domain");
    if !domain_dir.exists() {
        return Err(CliError::NotInProject {
            missing: "src/domain/",
        });
    }

    let entity_file = find_file_for_type(domain_dir, entity)?;

    let src = fs::read_to_string(&entity_file).map_err(|e| CliError::IoError {
        context: format!("failed to read {}", entity_file.display()),
        source: e,
    })?;

    let new_src = match kind {
        "standard" => {
            let range = range.ok_or_else(|| {
                CliError::general("standard variables require --range <fact_collection>")
            })?;
            inject_standard_variable(&src, entity, field, range, allows_unassigned)?
        }
        "list" => {
            let elements = elements.ok_or_else(|| {
                CliError::general("list variables require --elements <fact_collection>")
            })?;
            inject_list_variable(&src, entity, field, elements)?
        }
        _ => return Err(CliError::general("unsupported variable kind")),
    };
    fs::write(&entity_file, new_src).map_err(|e| CliError::IoError {
        context: format!("failed to write {}", entity_file.display()),
        source: e,
    })?;

    output::print_update(entity_file.to_str().unwrap());
    sync_project_metadata()?;
    Ok(())
}

pub fn destroy_variable(field: &str, entity: &str) -> CliResult {
    validate_name(field)?;

    let domain_dir = Path::new("src/domain");
    if !domain_dir.exists() {
        return Err(CliError::NotInProject {
            missing: "src/domain/",
        });
    }

    let entity_file = find_file_for_type(domain_dir, entity)?;
    let src = fs::read_to_string(&entity_file).map_err(|e| CliError::IoError {
        context: format!("failed to read {}", entity_file.display()),
        source: e,
    })?;
    let new_src = remove_variable_field(&src, field).map_err(CliError::general)?;
    fs::write(&entity_file, new_src).map_err(|e| CliError::IoError {
        context: format!("failed to write {}", entity_file.display()),
        source: e,
    })?;
    output::print_update(entity_file.to_str().unwrap());
    sync_project_metadata()?;
    Ok(())
}

pub fn run_score(score_type: &str) -> CliResult {
    validate_score_type(score_type)?;

    let domain_dir = Path::new("src/domain");
    if !domain_dir.exists() {
        return Err(CliError::NotInProject {
            missing: "src/domain/",
        });
    }

    let domain = parse_domain().ok_or(CliError::NotInProject {
        missing: "src/domain/ (no planning solution found)",
    })?;
    let solution_file = find_file_for_type(domain_dir, &domain.solution_type)?;

    let src = fs::read_to_string(&solution_file).map_err(|e| CliError::IoError {
        context: format!("failed to read {}", solution_file.display()),
        source: e,
    })?;

    let new_src = replace_score_type(&src, &domain.score_type, score_type)?;
    fs::write(&solution_file, new_src).map_err(|e| CliError::IoError {
        context: format!("failed to write {}", solution_file.display()),
        source: e,
    })?;

    output::print_update(solution_file.to_str().unwrap());
    sync_project_metadata()?;
    Ok(())
}

pub fn run_data(mode: &str) -> CliResult {
    render_data_seed(mode, true)
}

fn is_default_scaffold() -> CliResult<bool> {
    let plan_path = Path::new("src/domain/plan.rs");
    if !plan_path.exists() {
        return Ok(false);
    }

    let content = fs::read_to_string(plan_path).map_err(|e| CliError::IoError {
        context: "failed to read plan.rs".to_string(),
        source: e,
    })?;

    Ok(content.contains("Rename this to something domain-specific"))
}

pub(crate) fn remove_default_scaffold() -> CliResult {
    let domain_files = ["plan.rs", "task.rs", "resource.rs"];
    for file in &domain_files {
        let path = Path::new("src/domain").join(file);
        if path.exists() {
            fs::remove_file(&path).map_err(|e| CliError::IoError {
                context: format!("failed to remove {}", file),
                source: e,
            })?;
        }
    }

    let domain_mod = Path::new("src/domain/mod.rs");
    if domain_mod.exists() {
        fs::write(domain_mod, "// Domain module\n").map_err(|e| CliError::IoError {
            context: "failed to clear domain/mod.rs".to_string(),
            source: e,
        })?;
    }

    let all_assigned = Path::new("src/constraints/all_assigned.rs");
    if all_assigned.exists() {
        fs::remove_file(all_assigned).map_err(|e| CliError::IoError {
            context: "failed to remove all_assigned.rs".to_string(),
            source: e,
        })?;
    }

    let constraints_mod = Path::new("src/constraints/mod.rs");
    if constraints_mod.exists() {
        let content = fs::read_to_string(constraints_mod).map_err(|e| CliError::IoError {
            context: "failed to read constraints/mod.rs".to_string(),
            source: e,
        })?;

        let lines: Vec<&str> = content
            .lines()
            .filter(|line| !line.contains("all_assigned"))
            .collect();

        let new_content = lines.join("\n");
        fs::write(constraints_mod, new_content).map_err(|e| CliError::IoError {
            context: "failed to update constraints/mod.rs".to_string(),
            source: e,
        })?;
    }

    let data_mod = Path::new("src/data/mod.rs");
    if data_mod.exists() {
        let stub_content = generate_data_loader_stub();
        fs::write(data_mod, stub_content).map_err(|e| CliError::IoError {
            context: "failed to stub data/mod.rs".to_string(),
            source: e,
        })?;
    }

    output::print_remove("default scaffold");
    Ok(())
}

pub(crate) fn generate_data_loader_stub() -> &'static str {
    r#"/* Data loading module.

   Replace `load()` with code that reads your real inputs and constructs the
   domain objects your API or solver layer needs. */

pub fn load() -> Result<(), Box<dyn std::error::Error>> {
    Ok(())
}
"#
}

const GENERATED_DATA_WRAPPER_MARKER: &str = "@generated by solverforge-cli: data-wrapper v1";

fn sync_project_metadata() -> CliResult {
    app_spec::sync_from_project().and_then(|_| sync_demo_data_module())
}

fn sync_demo_data_module() -> CliResult {
    render_data_seed("sample", false)
}

fn render_data_seed(mode: &str, announce: bool) -> CliResult {
    let spec = app_spec::load()?;
    let domain = parse_domain().ok_or(CliError::NotInProject {
        missing: "src/domain/ (no planning solution found)",
    })?;

    migrate_data_wrapper_if_cli_owned(announce)?;
    ensure_generated_mod()?;

    let rendered = build_data_seed_source(&spec, &domain, mode)?;
    let data_seed_path = Path::new("src/generated/data_seed.rs");
    if let Some(parent) = data_seed_path.parent() {
        fs::create_dir_all(parent).map_err(|e| CliError::IoError {
            context: format!("failed to create {}", parent.display()),
            source: e,
        })?;
    }
    fs::write(data_seed_path, rendered).map_err(|e| CliError::IoError {
        context: "failed to write src/generated/data_seed.rs".to_string(),
        source: e,
    })?;
    if announce {
        output::print_update("src/generated/data_seed.rs");
    }
    Ok(())
}

fn ensure_generated_mod() -> CliResult {
    let generated_mod = Path::new("src/generated/mod.rs");
    if let Some(parent) = generated_mod.parent() {
        fs::create_dir_all(parent).map_err(|e| CliError::IoError {
            context: format!("failed to create {}", parent.display()),
            source: e,
        })?;
    }
    let content = "pub mod data_seed;\n";
    let current = fs::read_to_string(generated_mod).unwrap_or_default();
    if current != content {
        fs::write(generated_mod, content).map_err(|e| CliError::IoError {
            context: "failed to write src/generated/mod.rs".to_string(),
            source: e,
        })?;
    }
    Ok(())
}

fn migrate_data_wrapper_if_cli_owned(announce: bool) -> CliResult {
    let data_mod = Path::new("src/data/mod.rs");
    let content = data_wrapper_source();
    let current = fs::read_to_string(data_mod).unwrap_or_default();
    let is_cli_owned = current.is_empty()
        || normalized_source(&current) == normalized_source(generate_data_loader_stub())
        || current.contains(GENERATED_DATA_WRAPPER_MARKER);

    if !is_cli_owned {
        if announce {
            output::print_skip("src/data/mod.rs");
            output::print_dim(
                "preserved custom data loader; generated sample data is available in src/generated/data_seed.rs",
            );
        }
        return Ok(());
    }

    ensure_generated_export()?;

    if current != content {
        fs::write(data_mod, content).map_err(|e| CliError::IoError {
            context: "failed to write src/data/mod.rs".to_string(),
            source: e,
        })?;
        if announce {
            output::print_update("src/data/mod.rs");
        }
    }
    Ok(())
}

fn ensure_generated_export() -> CliResult {
    let lib_rs = Path::new("src/lib.rs");
    let current = fs::read_to_string(lib_rs).map_err(|e| CliError::IoError {
        context: "failed to read src/lib.rs".to_string(),
        source: e,
    })?;
    if current.contains("pub mod generated;") {
        return Ok(());
    }

    let updated = if current.contains("pub mod solver;") {
        current.replacen("pub mod solver;", "pub mod generated;\npub mod solver;", 1)
    } else {
        let mut next = current.trim_end().to_string();
        if !next.is_empty() {
            next.push('\n');
        }
        next.push_str("pub mod generated;\n");
        next
    };

    fs::write(lib_rs, updated).map_err(|e| CliError::IoError {
        context: "failed to write src/lib.rs".to_string(),
        source: e,
    })?;
    Ok(())
}

fn normalized_source(src: &str) -> String {
    src.replace("\r\n", "\n").trim().to_string()
}

fn data_wrapper_source() -> &'static str {
    r#"/* Demo data wrapper for local development.

@generated by solverforge-cli: data-wrapper v1

`solverforge generate data` rewrites the compiler-owned sample builders in
`src/generated/data_seed.rs`. Replace this wrapper only if you want to stop
using the generated seeds entirely. */

pub use crate::generated::data_seed::DemoData;

use crate::domain::Plan;

/// Generates a demo plan for the given dataset.
pub fn generate(demo: DemoData) -> Plan {
    crate::generated::data_seed::generate(demo)
}
"#
}

fn build_data_seed_source(
    spec: &app_spec::AppSpec,
    domain: &crate::commands::generate_constraint::domain::DomainModel,
    mode: &str,
) -> CliResult<String> {
    let domain_dir = Path::new("src/domain");
    let solution_file = find_file_for_type(domain_dir, &domain.solution_type)?;
    let solution_src = fs::read_to_string(&solution_file).map_err(|e| CliError::IoError {
        context: format!("failed to read {}", solution_file.display()),
        source: e,
    })?;
    let collections = parse_solution_collection_fields(&solution_src);
    let type_names = collections
        .iter()
        .map(|(_, ty)| ty.clone())
        .collect::<Vec<_>>();
    let struct_fields = parse_collection_structs(spec)?;
    let count_map = build_count_map(spec);

    let imports = if type_names.is_empty() {
        "use crate::domain::Plan;".to_string()
    } else {
        format!("use crate::domain::{{Plan, {}}};", type_names.join(", "))
    };

    let counts_small = count_bindings(spec, "small");
    let counts_standard = count_bindings(spec, "standard");
    let collection_builders = collections
        .iter()
        .map(|(plural, ty)| {
            render_collection_builder(plural, ty, spec, &struct_fields, &count_map, mode)
        })
        .collect::<CliResult<Vec<_>>>()?
        .join("\n\n");
    let constructor_args = collections
        .iter()
        .map(|(plural, _)| plural.clone())
        .collect::<Vec<_>>()
        .join(", ");
    let plan_call = if constructor_args.is_empty() {
        "    Plan::new()".to_string()
    } else {
        format!("    Plan::new({})", constructor_args)
    };

    Ok(format!(
        r#"// @generated by solverforge-cli: data-seed v1

use std::str::FromStr;

{imports}

#[derive(Debug, Clone, Copy)]
pub enum DemoData {{
    Small,
    Standard,
}}

impl FromStr for DemoData {{
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {{
        match s.to_uppercase().as_str() {{
            "SMALL" => Ok(DemoData::Small),
            "STANDARD" => Ok(DemoData::Standard),
            _ => Err(()),
        }}
    }}
}}

pub fn generate(demo: DemoData) -> Plan {{
    match demo {{
        DemoData::Small => generate_plan({counts_small}),
        DemoData::Standard => generate_plan({counts_standard}),
    }}
}}

fn generate_plan({count_signature}) -> Plan {{
{collection_builders}

{plan_call}
}}
"#,
        imports = imports,
        counts_small = counts_small,
        counts_standard = counts_standard,
        count_signature = count_signature(spec),
        collection_builders = indent_block(&collection_builders, 4),
        plan_call = plan_call,
    ))
}

fn parse_collection_structs(
    spec: &app_spec::AppSpec,
) -> CliResult<BTreeMap<String, Vec<(String, String)>>> {
    let domain_dir = Path::new("src/domain");
    let mut map = BTreeMap::new();
    for entry in spec.entities.iter().chain(spec.facts.iter()) {
        let ty = snake_to_pascal(&entry.name);
        let file = find_file_for_type(domain_dir, &ty)?;
        let src = fs::read_to_string(&file).map_err(|e| CliError::IoError {
            context: format!("failed to read {}", file.display()),
            source: e,
        })?;
        map.insert(ty.clone(), parse_struct_fields(&src, &ty));
    }
    Ok(map)
}

fn parse_solution_collection_fields(src: &str) -> Vec<(String, String)> {
    src.lines()
        .filter_map(|line| {
            let trimmed = line.trim();
            if !trimmed.starts_with("pub ") || trimmed.contains("score:") {
                return None;
            }
            let trimmed = trimmed.trim_start_matches("pub ").trim_end_matches(',');
            let colon = trimmed.find(':')?;
            let field = trimmed[..colon].trim().to_string();
            let type_part = trimmed[colon + 1..].trim();
            if type_part.starts_with("Vec<") && type_part.ends_with('>') {
                Some((field, type_part[4..type_part.len() - 1].to_string()))
            } else {
                None
            }
        })
        .collect()
}

fn parse_struct_fields(src: &str, type_name: &str) -> Vec<(String, String)> {
    let lines: Vec<&str> = src.lines().collect();
    let mut inside = false;
    let mut depth = 0i32;
    let mut fields = Vec::new();
    for line in lines {
        let trimmed = line.trim();
        if !inside {
            if trimmed.contains(&format!("struct {}", type_name)) {
                inside = true;
                depth += trimmed.chars().filter(|&c| c == '{').count() as i32;
                depth -= trimmed.chars().filter(|&c| c == '}').count() as i32;
            }
            continue;
        }
        depth += trimmed.chars().filter(|&c| c == '{').count() as i32;
        depth -= trimmed.chars().filter(|&c| c == '}').count() as i32;
        if trimmed.starts_with("pub ") {
            let trimmed = trimmed.trim_start_matches("pub ").trim_end_matches(',');
            if let Some(colon) = trimmed.find(':') {
                fields.push((
                    trimmed[..colon].trim().to_string(),
                    trimmed[colon + 1..].trim().to_string(),
                ));
            }
        }
        if depth <= 0 {
            break;
        }
    }
    fields
}

fn render_collection_builder(
    plural: &str,
    ty: &str,
    spec: &app_spec::AppSpec,
    structs: &BTreeMap<String, Vec<(String, String)>>,
    count_map: &BTreeMap<String, String>,
    mode: &str,
) -> CliResult<String> {
    let fields = structs
        .get(ty)
        .ok_or_else(|| CliError::general(format!("missing parsed fields for {}", ty)))?;
    let count_var = count_map
        .get(plural)
        .ok_or_else(|| CliError::general(format!("missing count var for {}", plural)))?;
    let entity_name = spec
        .entities
        .iter()
        .find(|entry| entry.plural == plural)
        .map(|entry| entry.name.clone());
    let initializers = fields
        .iter()
        .map(|(field, ty_name)| {
            render_field_initializer(
                field,
                ty_name,
                plural,
                entity_name.as_deref(),
                spec,
                count_map,
                mode,
            )
            .map(|value| format!("{field}: {value}"))
        })
        .collect::<CliResult<Vec<_>>>()?
        .join(", ");
    Ok(format!(
        "let {plural} = (0..{count_var})\n    .map(|idx| {ty} {{ {initializers} }})\n    .collect::<Vec<_>>();",
        plural = plural,
        count_var = count_var,
        ty = ty,
        initializers = initializers
    ))
}

fn render_field_initializer(
    field: &str,
    ty_name: &str,
    plural: &str,
    entity_name: Option<&str>,
    spec: &app_spec::AppSpec,
    count_map: &BTreeMap<String, String>,
    mode: &str,
) -> CliResult<String> {
    if field == "id" && ty_name == "String" {
        let stem = plural.trim_end_matches('s');
        return Ok(format!("format!(\"{}-{{idx}}\")", stem));
    }
    if field == "name" && ty_name == "String" {
        let stem = plural.trim_end_matches('s');
        return Ok(format!("format!(\"{}-{{idx}}\")", stem));
    }
    if field == "index" && ty_name == "usize" {
        return Ok("idx".to_string());
    }
    if let Some(entity_name) = entity_name {
        if let Some(variable) = spec.variables.iter().find(|variable| {
            variable.entity == entity_name && variable.field == field && variable.enabled
        }) {
            return match variable.kind.as_str() {
                "standard" => {
                    let source_plural = &variable.range;
                    let source_count = count_map
                        .get(source_plural)
                        .cloned()
                        .unwrap_or_else(|| "0".to_string());
                    if mode == "stub" {
                        Ok("None".to_string())
                    } else {
                        Ok(format!(
                            "if {source_count} == 0 {{ None }} else {{ Some(idx % {source_count}) }}",
                            source_count = source_count
                        ))
                    }
                }
                "list" => {
                    let source_plural = &variable.elements;
                    let source_count = count_map
                        .get(source_plural)
                        .cloned()
                        .unwrap_or_else(|| "0".to_string());
                    if mode == "stub" {
                        Ok("vec![]".to_string())
                    } else {
                        Ok(format!(
                            "if {source_count} == 0 || {entity_count} == 0 {{ vec![] }} else {{ (0..{source_count}).filter(|item_idx| item_idx % {entity_count} == idx % {entity_count}).collect::<Vec<usize>>() }}",
                            source_count = source_count,
                            entity_count = count_map.get(plural).cloned().unwrap_or_else(|| "1".to_string())
                        ))
                    }
                }
                _ => Ok(default_value_for_type(ty_name)),
            };
        }
    }
    Ok(default_value_for_type(ty_name))
}

fn default_value_for_type(ty_name: &str) -> String {
    match ty_name {
        "String" => "String::new()".to_string(),
        "usize" => "0".to_string(),
        "u32" => "0u32".to_string(),
        "u64" => "0u64".to_string(),
        "i32" => "0i32".to_string(),
        "i64" => "0i64".to_string(),
        "f32" => "0.0f32".to_string(),
        "f64" => "0.0f64".to_string(),
        "bool" => "false".to_string(),
        _ if ty_name.starts_with("Option<") => "None".to_string(),
        _ if ty_name.starts_with("Vec<") => "vec![]".to_string(),
        _ => "Default::default()".to_string(),
    }
}

fn count_signature(spec: &app_spec::AppSpec) -> String {
    spec.facts
        .iter()
        .chain(spec.entities.iter())
        .map(|entry| format!("{}: usize", count_var_name(&entry.plural)))
        .collect::<Vec<_>>()
        .join(", ")
}

fn count_bindings(spec: &app_spec::AppSpec, mode: &str) -> String {
    spec.facts
        .iter()
        .chain(spec.entities.iter())
        .enumerate()
        .map(|(idx, entry)| {
            let base = if entry.kind == "problem_fact" {
                2usize
            } else {
                3usize
            };
            let offset = if mode == "small" { idx } else { idx * 2 + 1 };
            format!("{}", base + offset)
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn build_count_map(spec: &app_spec::AppSpec) -> BTreeMap<String, String> {
    spec.facts
        .iter()
        .chain(spec.entities.iter())
        .map(|entry| (entry.plural.clone(), count_var_name(&entry.plural)))
        .collect()
}

fn count_var_name(plural: &str) -> String {
    format!("count_{}", plural)
}

fn indent_block(src: &str, spaces: usize) -> String {
    let pad = " ".repeat(spaces);
    src.lines()
        .map(|line| {
            if line.is_empty() {
                String::new()
            } else {
                format!("{}{}", pad, line)
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}
