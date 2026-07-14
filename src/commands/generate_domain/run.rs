use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use crate::app_spec;
use crate::commands::generate_constraint::validate_name;
use crate::commands::generate_constraint::{domain::DomainModel, parse_domain};
use crate::error::{CliError, CliResult};
use crate::list_variable_metadata::ListVariableMetadata;
use crate::output;
use crate::scalar_variable_hooks::ScalarVariableHooks;

use super::generators::{generate_entity, generate_fact, generate_solution};
use super::utils::{ensure_domain_dir, find_file_for_type, snake_to_pascal, validate_score_type};
use super::wiring::{
    inject_list_variable, inject_scalar_variable, remove_variable_field, replace_score_type,
    rewrite_domain_mod_source, wire_solution_collection_source, ScalarVariableWiring,
};

const NEUTRAL_SOLUTION_SENTINEL: &str = "// @solverforge:neutral-solution";
const NEUTRAL_SOLUTION_TYPE: &str = "Plan";
const NEUTRAL_SOLUTION_MODULE: &str = "plan";
const NEUTRAL_SCORE_TYPE: &str = "HardSoftScore";
const DOMAIN_EXPORTS_BLOCK: &str = "domain-exports";
const CONSTRAINT_MODULES_BLOCK: &str = "constraint-modules";
const CONSTRAINT_CALLS_BLOCK: &str = "constraint-calls";

struct CollectionRewritePlan {
    solution_file: PathBuf,
    rewritten_domain_mod: String,
    rewritten_solution: String,
}

struct NeutralReplacementPlan {
    neutral_plan_path: PathBuf,
    rewritten_constraints_mod: Option<String>,
    contract_rewrites: Vec<(PathBuf, String)>,
}

pub(crate) struct VariableRequest {
    pub field: String,
    pub entity: String,
    pub kind: String,
    pub range: Option<String>,
    pub countable_range: Option<String>,
    pub elements: Option<String>,
    pub allows_unassigned: bool,
    pub scalar_hooks: ScalarVariableHooks,
    pub list_metadata: ListVariableMetadata,
}

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

    let src =
        generate_entity(&pascal, planning_variable, &parsed_fields).map_err(CliError::general)?;
    let rewrites = preflight_collection_rewrites(name, &pascal, "planning_entity_collection")?;

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
    fs::write("src/domain/mod.rs", &rewrites.rewritten_domain_mod).map_err(|e| {
        CliError::IoError {
            context: "failed to update src/domain/mod.rs".to_string(),
            source: e,
        }
    })?;
    fs::write(&rewrites.solution_file, &rewrites.rewritten_solution).map_err(|e| {
        CliError::IoError {
            context: format!("failed to update {}", rewrites.solution_file.display()),
            source: e,
        }
    })?;
    let plural = super::utils::pluralize(name);
    crate::commands::sf_config::add_entity(name, &pascal, &plural)?;

    output::print_create(file_path.to_str().unwrap());
    print_diff_verbose("", &src);
    output::print_update("src/domain/mod.rs");
    output::print_update(rewrites.solution_file.to_str().unwrap());
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
    let rewrites = preflight_collection_rewrites(name, &pascal, "problem_fact_collection")?;

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
    fs::write("src/domain/mod.rs", &rewrites.rewritten_domain_mod).map_err(|e| {
        CliError::IoError {
            context: "failed to update src/domain/mod.rs".to_string(),
            source: e,
        }
    })?;
    fs::write(&rewrites.solution_file, &rewrites.rewritten_solution).map_err(|e| {
        CliError::IoError {
            context: format!("failed to update {}", rewrites.solution_file.display()),
            source: e,
        }
    })?;
    let plural = super::utils::pluralize(name);
    crate::commands::sf_config::add_fact(name, &pascal, &plural)?;

    output::print_create(file_path.to_str().unwrap());
    print_diff_verbose("", &src);
    output::print_update("src/domain/mod.rs");
    output::print_update(rewrites.solution_file.to_str().unwrap());
    sync_project_metadata()?;
    Ok(())
}

fn preflight_collection_rewrites(
    name: &str,
    pascal: &str,
    annotation: &str,
) -> CliResult<CollectionRewritePlan> {
    let domain_dir = Path::new("src/domain");
    let domain = parse_domain().map_err(CliError::general)?;
    let mod_path = domain_dir.join("mod.rs");
    if !mod_path.exists() {
        return Err(CliError::NotInProject {
            missing: "src/domain/mod.rs",
        });
    }

    let mod_src = fs::read_to_string(&mod_path).map_err(|e| CliError::IoError {
        context: "failed to read src/domain/mod.rs".to_string(),
        source: e,
    })?;
    let solution_file =
        find_file_for_type(domain_dir, &domain.solution_type).map_err(CliError::general)?;
    let solution_module_name = solution_file
        .file_stem()
        .and_then(|stem| stem.to_str())
        .ok_or_else(|| CliError::general("invalid solution file name"))?
        .to_string();
    let solution_src = fs::read_to_string(&solution_file).map_err(|e| CliError::IoError {
        context: format!("failed to read {}", solution_file.display()),
        source: e,
    })?;
    let plural = super::utils::pluralize(name);
    let rewritten_domain_mod =
        rewrite_domain_mod_source(&mod_src, name, pascal, Some(&solution_module_name))
            .map_err(CliError::general)?;
    let rewritten_solution =
        wire_solution_collection_source(&solution_src, pascal, &plural, annotation)
            .map_err(CliError::general)?;

    Ok(CollectionRewritePlan {
        solution_file,
        rewritten_domain_mod,
        rewritten_solution,
    })
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
    let file_path = domain_dir.join(format!("{}.rs", name));
    let mod_path = domain_dir.join("mod.rs");
    if file_path.exists() {
        return Err(CliError::ResourceExists {
            kind: "solution file",
            name: crate::output::display_path(&file_path),
        });
    }
    if !mod_path.exists() {
        return Err(CliError::NotInProject {
            missing: "src/domain/mod.rs",
        });
    }

    let src = generate_solution(&pascal, score).map_err(CliError::general)?;
    let domain = match parse_domain() {
        Ok(domain) => Some(domain),
        Err(err) if err.contains("requires exactly one #[planning_solution]") => None,
        Err(err) => return Err(CliError::general(err)),
    };
    let replace_neutral = if let Some(domain) = domain.as_ref() {
        if is_neutral_scaffold(domain)? {
            precompute_neutral_replacement(&pascal, score)?;
            true
        } else {
            return Err(CliError::with_hint(
                format!(
                    "a planning solution '{}' already exists",
                    domain.solution_type
                ),
                "use `solverforge destroy solution` then `solverforge generate solution` to replace it",
            ));
        }
    } else {
        false
    };
    let base_domain_mod = if replace_neutral {
        empty_domain_mod_source().to_string()
    } else {
        fs::read_to_string(&mod_path).map_err(|e| CliError::IoError {
            context: "failed to read src/domain/mod.rs".to_string(),
            source: e,
        })?
    };
    let rewritten_domain_mod =
        rewrite_domain_mod_source(&base_domain_mod, name, &pascal, Some(name))
            .map_err(CliError::general)?;

    // A fresh neutral scaffold may be replaced once. Shaped projects require an explicit destroy.
    if replace_neutral {
        remove_neutral_scaffold(&pascal, score)?;
    }

    fs::write(&file_path, &src).map_err(|e| CliError::IoError {
        context: format!(
            "failed to write {}",
            crate::output::display_path(&file_path)
        ),
        source: e,
    })?;
    fs::write(&mod_path, rewritten_domain_mod).map_err(|e| CliError::IoError {
        context: "failed to update src/domain/mod.rs".to_string(),
        source: e,
    })?;

    output::print_create(&crate::output::display_path(&file_path));
    output::print_update("src/domain/mod.rs");
    sync_project_metadata()?;
    Ok(())
}

pub(crate) fn run_variable(request: VariableRequest) -> CliResult {
    let VariableRequest {
        field,
        entity,
        kind,
        range,
        countable_range,
        elements,
        allows_unassigned,
        scalar_hooks,
        list_metadata,
    } = request;
    validate_name(&field)?;
    scalar_hooks.validate_paths().map_err(CliError::general)?;
    list_metadata.validate().map_err(CliError::general)?;
    if let Some(range) = countable_range.as_deref() {
        crate::countable_range::CountableRange::parse(range).map_err(CliError::general)?;
    }
    let domain_dir = Path::new("src/domain");
    if !domain_dir.exists() {
        return Err(CliError::NotInProject {
            missing: "src/domain/",
        });
    }

    let entity_file = find_file_for_type(domain_dir, &entity)?;

    let src = fs::read_to_string(&entity_file).map_err(|e| CliError::IoError {
        context: format!("failed to read {}", entity_file.display()),
        source: e,
    })?;

    let new_src = match kind.as_str() {
        "scalar" => {
            if !list_metadata.is_empty() {
                return Err(CliError::general("list metadata flags require --kind list"));
            }
            if range.is_none() && countable_range.is_none() {
                return Err(CliError::general(
                    "scalar variables require --range <fact_collection> or --countable-range <from..to>",
                ));
            }
            inject_scalar_variable(
                &src,
                &entity,
                &field,
                ScalarVariableWiring {
                    range: range.as_deref().unwrap_or_default(),
                    countable_range: countable_range.as_deref(),
                    allows_unassigned,
                    hooks: &scalar_hooks,
                },
            )?
        }
        "list" => {
            if !scalar_hooks.is_empty() {
                return Err(CliError::general("scalar hook flags require --kind scalar"));
            }
            if countable_range.is_some() {
                return Err(CliError::general(
                    "--countable-range requires --kind scalar",
                ));
            }
            let elements = elements.ok_or_else(|| {
                CliError::general("list variables require --elements <fact_collection>")
            })?;
            inject_list_variable(&src, &entity, &field, &elements, &list_metadata)?
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
    syn::parse_file(&new_src).map_err(|err| {
        CliError::general(format!(
            "destroying variable '{field}' would leave invalid Rust in {}: {err}",
            entity_file.display()
        ))
    })?;
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

    let domain = parse_domain().map_err(CliError::general)?;
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

pub fn run_data(mode: &str, size: Option<&str>) -> CliResult {
    if let Some(size) = size {
        app_spec::set_demo_size(size)?;
    }
    render_data_seed(mode, true)
}

fn is_neutral_scaffold(domain: &DomainModel) -> CliResult<bool> {
    if domain.solution_type != NEUTRAL_SOLUTION_TYPE
        || !domain.entities.is_empty()
        || !domain.facts.is_empty()
    {
        return Ok(false);
    }

    let spec_path = Path::new("solverforge.app.toml");
    if !spec_path.exists() {
        return Ok(false);
    }

    let spec = app_spec::load()?;
    if spec.app.starter != "neutral-shell"
        || spec.solution.name != NEUTRAL_SOLUTION_TYPE
        || !spec.facts.is_empty()
        || !spec.entities.is_empty()
        || !spec.variables.is_empty()
        || !spec.constraints.is_empty()
    {
        return Ok(false);
    }

    Ok(neutral_solution_file_is_marked()?
        && neutral_domain_exports()?
        && neutral_constraint_blocks()?)
}

fn neutral_solution_file_is_marked() -> CliResult<bool> {
    let plan_path = Path::new("src/domain").join(format!("{NEUTRAL_SOLUTION_MODULE}.rs"));
    if !plan_path.exists() {
        return Ok(false);
    }

    let content = fs::read_to_string(&plan_path).map_err(|e| CliError::IoError {
        context: "failed to read plan.rs".to_string(),
        source: e,
    })?;

    Ok(content
        .lines()
        .any(|line| line.trim() == NEUTRAL_SOLUTION_SENTINEL))
}

fn neutral_domain_exports() -> CliResult<bool> {
    let domain_mod = Path::new("src/domain/mod.rs");
    if !domain_mod.exists() {
        return Ok(false);
    }
    let src = fs::read_to_string(domain_mod).map_err(|e| CliError::IoError {
        context: "failed to read src/domain/mod.rs".to_string(),
        source: e,
    })?;
    let block =
        crate::managed_block::read_block(&src, DOMAIN_EXPORTS_BLOCK).map_err(CliError::general)?;
    let lines = block
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>();

    Ok(lines == ["mod plan;", "pub use plan::Plan;"])
}

fn neutral_constraint_blocks() -> CliResult<bool> {
    let constraints_mod = Path::new("src/constraints/mod.rs");
    if !constraints_mod.exists() {
        return Ok(false);
    }
    let src = fs::read_to_string(constraints_mod).map_err(|e| CliError::IoError {
        context: "failed to read src/constraints/mod.rs".to_string(),
        source: e,
    })?;
    let modules = crate::managed_block::read_block(&src, CONSTRAINT_MODULES_BLOCK)
        .map_err(CliError::general)?;
    let calls = crate::managed_block::read_block(&src, CONSTRAINT_CALLS_BLOCK)
        .map_err(CliError::general)?;

    Ok(modules.trim().is_empty() && calls.trim() == "()")
}

fn precompute_neutral_replacement(
    solution_type: &str,
    score_type: &str,
) -> CliResult<NeutralReplacementPlan> {
    let neutral_plan_path = Path::new("src/domain").join(format!("{NEUTRAL_SOLUTION_MODULE}.rs"));
    let constraints_mod = Path::new("src/constraints/mod.rs");
    let rewritten_constraints_mod = if constraints_mod.exists() {
        Some(empty_constraints_mod_source(solution_type, score_type))
    } else {
        None
    };

    Ok(NeutralReplacementPlan {
        neutral_plan_path,
        rewritten_constraints_mod,
        contract_rewrites: precompute_generated_contract_references(solution_type, score_type)?,
    })
}

fn precompute_generated_contract_references(
    solution_type: &str,
    score_type: &str,
) -> CliResult<Vec<(PathBuf, String)>> {
    if solution_type == NEUTRAL_SOLUTION_TYPE && score_type == NEUTRAL_SCORE_TYPE {
        return Ok(Vec::new());
    }

    let mut rewrites = Vec::new();
    for path in ["src/api/dto.rs", "src/solver/service.rs", "src/lib.rs"] {
        let path = Path::new(path);
        if !path.exists() {
            continue;
        }
        let src = fs::read_to_string(path).map_err(|e| CliError::IoError {
            context: format!("failed to read {}", path.display()),
            source: e,
        })?;
        let updated = replace_identifier(&src, NEUTRAL_SOLUTION_TYPE, solution_type);
        let updated = replace_identifier(&updated, NEUTRAL_SCORE_TYPE, score_type);
        if updated != src {
            rewrites.push((path.to_path_buf(), updated));
        }
    }

    Ok(rewrites)
}

pub(crate) fn remove_neutral_scaffold(solution_type: &str, score_type: &str) -> CliResult {
    let plan = precompute_neutral_replacement(solution_type, score_type)?;
    apply_neutral_replacement(plan)
}

fn apply_neutral_replacement(plan: NeutralReplacementPlan) -> CliResult {
    if plan.neutral_plan_path.exists() {
        fs::remove_file(&plan.neutral_plan_path).map_err(|e| CliError::IoError {
            context: format!("failed to remove {}", plan.neutral_plan_path.display()),
            source: e,
        })?;
    }

    let domain_mod = Path::new("src/domain/mod.rs");
    if domain_mod.exists() {
        fs::write(domain_mod, empty_domain_mod_source()).map_err(|e| CliError::IoError {
            context: "failed to clear domain/mod.rs".to_string(),
            source: e,
        })?;
    }

    if let Some(rewritten_constraints_mod) = plan.rewritten_constraints_mod {
        fs::write("src/constraints/mod.rs", rewritten_constraints_mod).map_err(|e| {
            CliError::IoError {
                context: "failed to update constraints/mod.rs".to_string(),
                source: e,
            }
        })?;
    }

    for (path, rewritten) in plan.contract_rewrites {
        fs::write(&path, rewritten).map_err(|e| CliError::IoError {
            context: format!("failed to write {}", path.display()),
            source: e,
        })?;
        output::print_update(&path.display().to_string());
    }

    output::print_remove("neutral scaffold");
    Ok(())
}

fn replace_identifier(src: &str, from: &str, to: &str) -> String {
    let mut out = String::with_capacity(src.len());
    let mut start = 0;

    for (idx, _) in src.match_indices(from) {
        let before = src[..idx].chars().next_back();
        let after = src[idx + from.len()..].chars().next();
        if before.is_some_and(is_rust_identifier_char) || after.is_some_and(is_rust_identifier_char)
        {
            continue;
        }
        out.push_str(&src[start..idx]);
        out.push_str(to);
        start = idx + from.len();
    }

    out.push_str(&src[start..]);
    out
}

fn is_rust_identifier_char(ch: char) -> bool {
    ch == '_' || ch.is_ascii_alphanumeric()
}

fn empty_domain_mod_source() -> &'static str {
    r#"solverforge::planning_model! {
    root = "src/domain";

    // @solverforge:begin domain-exports
// @solverforge:end domain-exports
}
"#
}

fn empty_constraints_mod_source(solution_type: &str, score_type: &str) -> String {
    format!(
        r#"/* Constraint definitions.

Add constraint modules with `solverforge generate constraint ...`.
The neutral shell starts with an empty constraint set. */

use crate::domain::{solution_type};
use solverforge::prelude::*;

pub use self::assemble::create_constraints;

// @solverforge:begin constraint-modules
// @solverforge:end constraint-modules

mod assemble {{
    use super::*;

    pub fn create_constraints() -> impl ConstraintSet<{solution_type}, {score_type}> {{
        // @solverforge:begin constraint-calls
        ()
        // @solverforge:end constraint-calls
    }}
}}
"#
    )
}

fn sync_project_metadata() -> CliResult {
    app_spec::sync_from_project().and_then(|_| sync_demo_data_module())
}

fn sync_demo_data_module() -> CliResult {
    render_data_seed("sample", false)
}

fn render_data_seed(mode: &str, announce: bool) -> CliResult {
    let spec = app_spec::load()?;
    let domain = parse_domain().map_err(CliError::general)?;

    let rendered = build_data_seed_source(&spec, &domain, mode)?;
    let seed_path = Path::new("src/data/data_seed.rs");
    fs::write(seed_path, rendered).map_err(|e| CliError::IoError {
        context: "failed to write src/data/data_seed.rs".to_string(),
        source: e,
    })?;
    if announce {
        output::print_update("src/data/data_seed.rs");
    }
    Ok(())
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

    let solution_type = &domain.solution_type;
    let imports = if type_names.is_empty() {
        format!("use crate::domain::{solution_type};")
    } else {
        format!(
            "use crate::domain::{{{solution_type}, {}}};",
            type_names.join(", ")
        )
    };

    let demo_sizes = demo_sizes(spec);
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
    let solution_call = if constructor_args.is_empty() {
        format!("    {solution_type}::new()")
    } else {
        format!("    {solution_type}::new({constructor_args})")
    };
    let demo_variants = demo_sizes
        .iter()
        .map(|size| snake_to_pascal(size))
        .collect::<Vec<_>>()
        .join(",\n    ");
    let demo_id_arms = demo_sizes
        .iter()
        .map(|size| {
            format!(
                "            DemoData::{variant} => \"{upper}\",",
                upper = size.to_ascii_uppercase(),
                variant = snake_to_pascal(size)
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    let available_demo_variants = demo_sizes
        .iter()
        .map(|size| format!("DemoData::{}", snake_to_pascal(size)))
        .collect::<Vec<_>>()
        .join(", ");
    let default_demo_variant = snake_to_pascal(&spec.demo.default_size);
    let from_str_arms = demo_sizes
        .iter()
        .map(|size| {
            format!(
                "            \"{upper}\" => Ok(DemoData::{variant}),",
                upper = size.to_ascii_uppercase(),
                variant = snake_to_pascal(size)
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    let generate_arms = demo_sizes
        .iter()
        .map(|size| {
            format!(
                "        DemoData::{variant} => generate_plan({counts}),",
                variant = snake_to_pascal(size),
                counts = count_bindings(spec, size)
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    Ok(format!(
        r#"// @generated by solverforge-cli: data v1

use std::str::FromStr;

{imports}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DemoData {{
    {demo_variants}
}}

const AVAILABLE_DEMO_DATA: &[DemoData] = &[{available_demo_variants}];
const DEFAULT_DEMO_DATA: DemoData = DemoData::{default_demo_variant};

pub fn default_demo_data() -> DemoData {{
    DEFAULT_DEMO_DATA
}}

pub fn available_demo_data() -> &'static [DemoData] {{
    AVAILABLE_DEMO_DATA
}}

impl DemoData {{
    pub fn id(self) -> &'static str {{
        match self {{
{demo_id_arms}
        }}
    }}

    pub fn default_demo_data() -> Self {{
        default_demo_data()
    }}

    pub fn available_demo_data() -> &'static [Self] {{
        available_demo_data()
    }}
}}

impl FromStr for DemoData {{
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {{
        match s.to_ascii_uppercase().as_str() {{
{from_str_arms}
            _ => Err(()),
        }}
    }}
}}

pub fn generate(demo: DemoData) -> {solution_type} {{
    match demo {{
{generate_arms}
    }}
}}

fn generate_plan({count_signature}) -> {solution_type} {{
{collection_builders}

{solution_call}
}}
"#,
        imports = imports,
        solution_type = solution_type,
        demo_variants = demo_variants,
        available_demo_variants = available_demo_variants,
        default_demo_variant = default_demo_variant,
        demo_id_arms = indent_block(&demo_id_arms, 12),
        from_str_arms = indent_block(&from_str_arms, 12),
        generate_arms = indent_block(&generate_arms, 8),
        count_signature = count_signature(spec),
        collection_builders = indent_block(&collection_builders, 4),
        solution_call = solution_call,
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
    let context = FieldRenderContext {
        fields,
        plural,
        entity_name: entity_name.as_deref(),
        spec,
        count_map,
        mode,
    };
    let initializers = fields
        .iter()
        .map(|(field, ty_name)| {
            render_field_initializer(field, ty_name, &context)
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

struct FieldRenderContext<'a> {
    fields: &'a [(String, String)],
    plural: &'a str,
    entity_name: Option<&'a str>,
    spec: &'a app_spec::AppSpec,
    count_map: &'a BTreeMap<String, String>,
    mode: &'a str,
}

fn render_field_initializer(
    field: &str,
    ty_name: &str,
    context: &FieldRenderContext<'_>,
) -> CliResult<String> {
    if field == "id" && ty_name == "String" {
        let stem = context.plural.trim_end_matches('s');
        return Ok(format!("format!(\"{}-{{idx}}\")", stem));
    }
    if let Some(entity_name) = context.entity_name {
        if let Some(variable) = context.spec.variables.iter().find(|variable| {
            variable.entity == entity_name && variable.field == field && variable.enabled
        }) {
            return match variable.kind.as_str() {
                "scalar" => {
                    let source_plural = &variable.range;
                    let source_count = context
                        .count_map
                        .get(source_plural)
                        .cloned()
                        .unwrap_or_else(|| "0".to_string());
                    if context.mode == "stub" {
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
                    let source_count = context
                        .count_map
                        .get(source_plural)
                        .cloned()
                        .unwrap_or_else(|| "0".to_string());
                    if context.mode == "stub" {
                        Ok("vec![]".to_string())
                    } else {
                        Ok(format!(
                            "if {source_count} == 0 || {entity_count} == 0 {{ vec![] }} else {{ (0..{source_count}).filter(|item_idx| item_idx % {entity_count} == idx % {entity_count}).collect::<Vec<usize>>() }}",
                            source_count = source_count,
                            entity_count = context.count_map.get(context.plural).cloned().unwrap_or_else(|| "1".to_string())
                        ))
                    }
                }
                _ => Ok(default_value_for_type(ty_name)),
            };
        }
    }
    Ok(default_value_for_field(
        field,
        ty_name,
        context.plural,
        context.fields,
    ))
}

fn default_value_for_field(
    field: &str,
    ty_name: &str,
    plural: &str,
    _fields: &[(String, String)],
) -> String {
    if ty_name == "String" {
        let field_lower = field.to_ascii_lowercase();
        let stem = plural.trim_end_matches('s');
        return match field_lower.as_str() {
            "name" => format!("format!(\"{}-{{idx}}\")", stem),
            _ if field_lower.ends_with("_name") => format!(
                "format!(\"{}-{{idx}}\")",
                field_lower.trim_end_matches("_name")
            ),
            _ => format!("format!(\"{}-{}-{{idx}}\")", stem, field_lower),
        };
    }
    default_value_for_type(ty_name)
}

fn default_value_for_type(ty_name: &str) -> String {
    match ty_name {
        "String" => "String::new()".to_string(),
        "usize" => "idx + 1".to_string(),
        "u32" => "(idx as u32) + 1".to_string(),
        "u64" => "(idx as u64) + 1".to_string(),
        "i32" => "((idx % 7) as i32) - 2".to_string(),
        "i64" => "((idx % 11) as i64) - 5".to_string(),
        "f32" => "((idx % 9) as f32) * 1.25".to_string(),
        "f64" => "((idx % 13) as f64) * 1.25".to_string(),
        "bool" => "idx % 2 == 0".to_string(),
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

fn count_bindings(spec: &app_spec::AppSpec, size: &str) -> String {
    spec.facts
        .iter()
        .chain(spec.entities.iter())
        .enumerate()
        .map(|(idx, entry)| format!("{}", collection_count(entry.kind.as_str(), idx, size)))
        .collect::<Vec<_>>()
        .join(", ")
}

fn collection_count(kind: &str, idx: usize, size: &str) -> usize {
    match size {
        "small" => {
            if kind == "problem_fact" {
                2 + idx
            } else {
                3 + idx
            }
        }
        "large" => {
            if kind == "problem_fact" {
                8 + (idx * 3)
            } else {
                12 + (idx * 4)
            }
        }
        _ => {
            if kind == "problem_fact" {
                4 + (idx * 2)
            } else {
                6 + (idx * 3)
            }
        }
    }
}

fn demo_sizes(spec: &app_spec::AppSpec) -> Vec<String> {
    let mut sizes = if spec.demo.available_sizes.is_empty() {
        vec![
            "small".to_string(),
            "standard".to_string(),
            "large".to_string(),
        ]
    } else {
        spec.demo.available_sizes.clone()
    };
    if !sizes.iter().any(|size| size == &spec.demo.default_size) {
        sizes.push(spec.demo.default_size.clone());
    }
    sizes
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
