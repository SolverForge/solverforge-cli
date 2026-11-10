use std::fs;
use std::path::Path;

use crate::commands::generate_constraint::{
    domain::list_constraints, remove_constraint_from_source, ConstraintSources,
    CONSTRAINTS_ASSEMBLE_PATH, CONSTRAINTS_MOD_PATH,
};
use crate::commands::generate_constraint::{domain::DomainModel, parse_domain};
use crate::commands::generate_domain::{find_file_for_type, snake_to_pascal};
use crate::commands::generate_domain::{
    remove_domain_mod_entry_source, sync_demo_data_module, unwire_collection_from_solution_source,
};
use crate::error::{CliError, CliResult};
use crate::model_contract;
use crate::model_id::ConstraintId;
use crate::output;
use crate::{app_spec, commands::generate_domain};

fn confirm_destroy(kind: &str, name: &str, skip_confirm: bool) -> CliResult<bool> {
    if skip_confirm {
        return Ok(true);
    }

    let prompt = format!("Remove {} '{}'?", kind, name);
    let confirmed = dialoguer::Confirm::new()
        .with_prompt(prompt)
        .default(false)
        .interact()
        .map_err(|e| CliError::general(format!("prompt failed: {}", e)))?;

    Ok(confirmed)
}

pub fn run_solution(skip_confirm: bool) -> CliResult {
    let domain = parse_domain().map_err(CliError::general)?;

    if !confirm_destroy("solution", &domain.solution_type, skip_confirm)? {
        output::print_skip(&format!("solution {}", domain.solution_type));
        return Ok(());
    }

    let domain_dir = Path::new("src/domain");
    let solution_file = find_file_for_type(domain_dir, &domain.solution_type)?;
    let mod_path = domain_dir.join("mod.rs");

    let file_name = solution_file
        .file_stem()
        .and_then(|s| s.to_str())
        .ok_or_else(|| CliError::general("invalid solution file name"))?
        .to_string();

    if !mod_path.exists() {
        return Err(CliError::NotInProject {
            missing: "src/domain/mod.rs",
        });
    }

    let mod_src = fs::read_to_string(&mod_path).map_err(|e| CliError::IoError {
        context: "failed to read src/domain/mod.rs".to_string(),
        source: e,
    })?;
    let new_mod_src = remove_domain_mod_entry_source(&mod_src, &file_name, Some(&file_name))
        .map_err(CliError::general)?;

    fs::write(&mod_path, new_mod_src).map_err(|e| CliError::IoError {
        context: "failed to update src/domain/mod.rs".to_string(),
        source: e,
    })?;
    fs::remove_file(&solution_file).map_err(|e| CliError::IoError {
        context: format!("failed to delete {}", solution_file.display()),
        source: e,
    })?;
    app_spec::sync_from_project()?;
    sync_demo_data_module()?;

    output::print_remove(&format!("src/domain/{}.rs", file_name));
    output::print_update("src/domain/mod.rs");
    Ok(())
}

pub fn run_entity(name: &str, skip_confirm: bool) -> CliResult {
    let domain = parse_domain().map_err(CliError::general)?;

    let snake = name.to_lowercase().replace('-', "_");
    let pascal = snake_to_pascal(&snake);

    let entity = domain
        .entities
        .iter()
        .find(|e| e.item_type == pascal)
        .ok_or_else(|| CliError::ResourceNotFound {
            kind: "entity",
            name: name.to_string(),
        })?;

    model_contract::ensure_entity_has_no_scalar_group_targets(&pascal)?;
    ensure_type_is_unreferenced_in_source(
        "entity",
        &domain,
        &entity.item_type,
        &entity.field_name,
        &snake,
    )?;

    if !confirm_destroy("entity", &pascal, skip_confirm)? {
        output::print_skip(&format!("entity {}", pascal));
        return Ok(());
    }

    let domain_dir = Path::new("src/domain");
    let mod_path = domain_dir.join("mod.rs");
    let file_path = find_file_for_type(domain_dir, &pascal).or_else(|_| {
        let path = domain_dir.join(format!("{}.rs", snake));
        if path.exists() {
            Ok(path)
        } else {
            Err(CliError::ResourceNotFound {
                kind: "entity file",
                name: pascal.clone(),
            })
        }
    })?;

    let file_name = file_path
        .file_stem()
        .and_then(|s| s.to_str())
        .ok_or_else(|| CliError::general("invalid entity file name"))?
        .to_string();

    if !mod_path.exists() {
        return Err(CliError::NotInProject {
            missing: "src/domain/mod.rs",
        });
    }

    let mod_src = fs::read_to_string(&mod_path).map_err(|e| CliError::IoError {
        context: "failed to read src/domain/mod.rs".to_string(),
        source: e,
    })?;
    let solution_file = find_file_for_type(domain_dir, &domain.solution_type)?;
    let solution_file_name = solution_file
        .file_stem()
        .and_then(|s| s.to_str())
        .ok_or_else(|| CliError::general("invalid solution file name"))?
        .to_string();
    let solution_src = fs::read_to_string(&solution_file).map_err(|e| CliError::IoError {
        context: format!("failed to read {}", solution_file.display()),
        source: e,
    })?;
    let new_mod_src =
        remove_domain_mod_entry_source(&mod_src, &file_name, Some(&solution_file_name))
            .map_err(CliError::general)?;
    let new_solution_src = unwire_collection_from_solution_source(
        &solution_src,
        &entity.field_name,
        &entity.item_type,
    )
    .map_err(CliError::general)?;

    fs::write(&solution_file, new_solution_src).map_err(|e| CliError::IoError {
        context: format!("failed to update {}", solution_file.display()),
        source: e,
    })?;
    fs::write(&mod_path, new_mod_src).map_err(|e| CliError::IoError {
        context: "failed to update src/domain/mod.rs".to_string(),
        source: e,
    })?;
    fs::remove_file(&file_path).map_err(|e| CliError::IoError {
        context: format!("failed to delete {}", file_path.display()),
        source: e,
    })?;
    crate::commands::sf_config::remove_entity(&snake)?;
    app_spec::sync_from_project()?;
    // Keep the compiler-owned demo data in sync after the domain changed.
    sync_demo_data_module()?;

    output::print_remove(&format!("src/domain/{}.rs", file_name));
    output::print_update("src/domain/mod.rs");
    Ok(())
}

pub fn run_fact(name: &str, skip_confirm: bool) -> CliResult {
    let domain = parse_domain().map_err(CliError::general)?;

    let snake = name.to_lowercase().replace('-', "_");
    let pascal = snake_to_pascal(&snake);

    let fact = domain
        .facts
        .iter()
        .find(|f| f.item_type == pascal)
        .ok_or_else(|| CliError::ResourceNotFound {
            kind: "fact",
            name: name.to_string(),
        })?;

    ensure_fact_collection_is_unreferenced(&domain, fact.field_name.as_str(), &fact.item_type)?;
    ensure_type_is_unreferenced_in_source(
        "fact",
        &domain,
        &fact.item_type,
        &fact.field_name,
        &snake,
    )?;

    if !confirm_destroy("fact", &pascal, skip_confirm)? {
        output::print_skip(&format!("fact {}", pascal));
        return Ok(());
    }

    let domain_dir = Path::new("src/domain");
    let mod_path = domain_dir.join("mod.rs");
    let file_path = find_file_for_type(domain_dir, &pascal).or_else(|_| {
        let path = domain_dir.join(format!("{}.rs", snake));
        if path.exists() {
            Ok(path)
        } else {
            Err(CliError::ResourceNotFound {
                kind: "fact file",
                name: pascal.clone(),
            })
        }
    })?;

    let file_name = file_path
        .file_stem()
        .and_then(|s| s.to_str())
        .ok_or_else(|| CliError::general("invalid fact file name"))?
        .to_string();

    if !mod_path.exists() {
        return Err(CliError::NotInProject {
            missing: "src/domain/mod.rs",
        });
    }

    let mod_src = fs::read_to_string(&mod_path).map_err(|e| CliError::IoError {
        context: "failed to read src/domain/mod.rs".to_string(),
        source: e,
    })?;
    let solution_file = find_file_for_type(domain_dir, &domain.solution_type)?;
    let solution_file_name = solution_file
        .file_stem()
        .and_then(|s| s.to_str())
        .ok_or_else(|| CliError::general("invalid solution file name"))?
        .to_string();
    let solution_src = fs::read_to_string(&solution_file).map_err(|e| CliError::IoError {
        context: format!("failed to read {}", solution_file.display()),
        source: e,
    })?;
    let new_mod_src =
        remove_domain_mod_entry_source(&mod_src, &file_name, Some(&solution_file_name))
            .map_err(CliError::general)?;
    let new_solution_src =
        unwire_collection_from_solution_source(&solution_src, &fact.field_name, &fact.item_type)
            .map_err(CliError::general)?;

    fs::write(&solution_file, new_solution_src).map_err(|e| CliError::IoError {
        context: format!("failed to update {}", solution_file.display()),
        source: e,
    })?;
    fs::write(&mod_path, new_mod_src).map_err(|e| CliError::IoError {
        context: "failed to update src/domain/mod.rs".to_string(),
        source: e,
    })?;
    fs::remove_file(&file_path).map_err(|e| CliError::IoError {
        context: format!("failed to delete {}", file_path.display()),
        source: e,
    })?;
    crate::commands::sf_config::remove_fact(&snake)?;
    app_spec::sync_from_project()?;
    // Keep the compiler-owned demo data in sync after the domain changed.
    sync_demo_data_module()?;

    output::print_remove(&format!("src/domain/{}.rs", file_name));
    output::print_update("src/domain/mod.rs");
    Ok(())
}

fn ensure_fact_collection_is_unreferenced(
    domain: &DomainModel,
    field_name: &str,
    fact_type: &str,
) -> CliResult {
    let mut references = Vec::new();

    for entity in &domain.entities {
        for variable in &entity.scalar_vars {
            if !variable.value_range_provider.is_empty()
                && variable.value_range_provider == field_name
            {
                references.push(format!(
                    "{}.{} (value_range_provider = \"{}\")",
                    entity.item_type, variable.field, field_name
                ));
            }
        }
        for variable in &entity.list_vars {
            if variable.element_collection == field_name {
                references.push(format!(
                    "{}.{} (element_collection = \"{}\")",
                    entity.item_type, variable.field, field_name
                ));
            }
        }
    }

    if references.is_empty() {
        return Ok(());
    }

    Err(CliError::general(format!(
        "cannot destroy fact '{}' because managed planning variables still reference collection '{}': {}",
        fact_type,
        field_name,
        references.join(", ")
    )))
}

/// Refuses to destroy a domain type while project source outside the files the
/// destroy rewrites still names it.
///
/// Destroying an entity or fact deletes its file, its domain export, and its
/// solution collection. Any other source that uses the type (`Task`) or its
/// solution collection accessor (`Plan::tasks()`), typically constraints,
/// would stop compiling, so the destroy is blocked and the dependent files are
/// named instead of being edited or deleted behind the user's back.
fn ensure_type_is_unreferenced_in_source(
    kind: &str,
    domain: &DomainModel,
    type_name: &str,
    field_name: &str,
    snake: &str,
) -> CliResult {
    let domain_dir = Path::new("src/domain");
    let mut rewritten = vec![
        domain_dir.join("mod.rs"),
        domain_dir.join(format!("{snake}.rs")),
        Path::new("src/data/data_seed.rs").to_path_buf(),
    ];
    for owned_type in [type_name, domain.solution_type.as_str()] {
        if let Ok(path) = find_file_for_type(domain_dir, owned_type) {
            rewritten.push(path);
        }
    }

    let mut files = Vec::new();
    collect_rust_files(Path::new("src"), &mut files)?;
    files.sort();

    let mut dependents = Vec::new();
    for file in files {
        if rewritten.iter().any(|path| path == &file) {
            continue;
        }
        let source = fs::read_to_string(&file).map_err(|e| CliError::IoError {
            context: format!("failed to read {}", file.display()),
            source: e,
        })?;
        if source_references_type(&source, type_name, &domain.solution_type, field_name) {
            dependents.push(file);
        }
    }

    if dependents.is_empty() {
        return Ok(());
    }

    let constraints_dir = Path::new("src/constraints");
    let declared_constraints = list_constraints(Path::new(".")).map_err(CliError::general)?;
    let mut constraint_ids = Vec::new();
    let mut other_files = Vec::new();
    for file in &dependents {
        let constraint_id = (file.parent() == Some(constraints_dir))
            .then(|| file.file_stem().and_then(|stem| stem.to_str()))
            .flatten()
            .filter(|stem| declared_constraints.iter().any(|id| id == stem));
        match constraint_id {
            Some(id) => constraint_ids.push(id.to_string()),
            None => other_files.push(slash_path(file)),
        }
    }

    let mut hints = Vec::new();
    if !constraint_ids.is_empty() {
        hints.push(format!(
            "destroy dependent constraints first with {}",
            constraint_ids
                .iter()
                .map(|id| format!("`solverforge destroy --yes constraint {id}`"))
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    if !other_files.is_empty() {
        hints.push(format!(
            "remove the references to '{type_name}' from {}",
            other_files.join(", ")
        ));
    }

    Err(CliError::with_hint(
        format!(
            "cannot destroy {kind} '{type_name}' because source files still reference it: {}",
            dependents
                .iter()
                .map(|file| slash_path(file))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        hints.join("; then "),
    ))
}

fn collect_rust_files(dir: &Path, files: &mut Vec<std::path::PathBuf>) -> CliResult {
    if !dir.is_dir() {
        return Ok(());
    }
    let entries = fs::read_dir(dir).map_err(|e| CliError::IoError {
        context: format!("failed to read {}", dir.display()),
        source: e,
    })?;
    for entry in entries {
        let path = entry
            .map_err(|e| CliError::IoError {
                context: format!("failed to read {}", dir.display()),
                source: e,
            })?
            .path();
        if path.is_dir() {
            collect_rust_files(&path, files)?;
        } else if path.extension().and_then(|ext| ext.to_str()) == Some("rs") {
            files.push(path);
        }
    }
    Ok(())
}

/// Reports whether Rust `source` names `type_name` or the solution collection
/// accessor `<solution_type>::<field_name>` outside comments and literals.
fn source_references_type(
    source: &str,
    type_name: &str,
    solution_type: &str,
    field_name: &str,
) -> bool {
    let tokens = code_tokens(source);
    tokens.iter().enumerate().any(|(i, token)| {
        token == type_name
            || (token == solution_type
                && tokens.get(i + 1).map(String::as_str) == Some("::")
                && tokens.get(i + 2).map(String::as_str) == Some(field_name))
    })
}

/// Splits Rust source into identifiers and `::` path separators, dropping
/// comments, string/char literals, and other punctuation.
fn code_tokens(source: &str) -> Vec<String> {
    let chars: Vec<char> = source.chars().collect();
    let mut tokens = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        let next = chars.get(i + 1).copied();
        if c == '/' && next == Some('/') {
            while i < chars.len() && chars[i] != '\n' {
                i += 1;
            }
        } else if c == '/' && next == Some('*') {
            let mut depth = 0usize;
            while i < chars.len() {
                if chars[i] == '/' && chars.get(i + 1) == Some(&'*') {
                    depth += 1;
                    i += 2;
                } else if chars[i] == '*' && chars.get(i + 1) == Some(&'/') {
                    depth -= 1;
                    i += 2;
                    if depth == 0 {
                        break;
                    }
                } else {
                    i += 1;
                }
            }
        } else if c == 'r' && matches!(next, Some('"') | Some('#')) && raw_string_start(&chars, i) {
            i = skip_raw_string(&chars, i);
        } else if c == '"' {
            i += 1;
            while i < chars.len() && chars[i] != '"' {
                i += if chars[i] == '\\' { 2 } else { 1 };
            }
            i += 1;
        } else if c == '\'' {
            i = skip_char_literal_or_lifetime(&chars, i);
        } else if c.is_alphabetic() || c == '_' {
            let start = i;
            while i < chars.len() && (chars[i].is_alphanumeric() || chars[i] == '_') {
                i += 1;
            }
            tokens.push(chars[start..i].iter().collect());
        } else if c == ':' && next == Some(':') {
            tokens.push("::".to_string());
            i += 2;
        } else {
            i += 1;
        }
    }
    tokens
}

fn raw_string_start(chars: &[char], start: usize) -> bool {
    if start > 0 && (chars[start - 1].is_alphanumeric() || chars[start - 1] == '_') {
        return false;
    }
    let mut i = start + 1;
    while chars.get(i) == Some(&'#') {
        i += 1;
    }
    chars.get(i) == Some(&'"')
}

fn skip_raw_string(chars: &[char], start: usize) -> usize {
    let mut i = start + 1;
    let mut hashes = 0;
    while chars.get(i) == Some(&'#') {
        hashes += 1;
        i += 1;
    }
    i += 1;
    while i < chars.len() {
        if chars[i] == '"' && (1..=hashes).all(|k| chars.get(i + k) == Some(&'#')) {
            return i + 1 + hashes;
        }
        i += 1;
    }
    chars.len()
}

/// Skips a char literal (`'a'`, `'\''`); a lifetime (`'a`) only skips its quote
/// so the label is still tokenized harmlessly.
fn skip_char_literal_or_lifetime(chars: &[char], start: usize) -> usize {
    match (chars.get(start + 1), chars.get(start + 2)) {
        (Some('\\'), _) => {
            let mut i = start + 3;
            while i < chars.len() && chars[i] != '\'' {
                i += 1;
            }
            i + 1
        }
        (Some(_), Some('\'')) => start + 3,
        _ => start + 1,
    }
}

pub fn run_variable(field: &str, entity: &str, skip_confirm: bool) -> CliResult {
    model_contract::ensure_variable_has_no_scalar_group_targets(entity, field)?;

    if !confirm_destroy("variable", field, skip_confirm)? {
        output::print_skip(&format!("variable {}", field));
        return Ok(());
    }

    generate_domain::destroy_variable(field, entity)
}

pub fn run_constraint(name: &str, skip_confirm: bool) -> CliResult {
    let constraint_id = ConstraintId::parse(name)?;
    let file_path = Path::new("src/constraints").join(format!("{}.rs", constraint_id.as_str()));

    if !file_path.exists() {
        return Err(CliError::ResourceNotFound {
            kind: "constraint",
            name: name.to_string(),
        });
    }

    if !confirm_destroy("constraint", name, skip_confirm)? {
        output::print_skip(&format!("constraint {}", name));
        return Ok(());
    }

    let sources = ConstraintSources::read(Path::new("."))?;
    let rewritten = remove_constraint_from_source(&sources, constraint_id.as_str())
        .map_err(CliError::general)?;
    model_contract::ensure_constraint_has_no_conflict_repair_refs(constraint_id.as_str())?;

    rewritten.write(Path::new("."))?;
    fs::remove_file(&file_path).map_err(|e| CliError::IoError {
        context: format!("failed to delete {}", file_path.display()),
        source: e,
    })?;
    crate::commands::sf_config::remove_constraint(constraint_id.as_str())?;
    app_spec::sync_from_project()?;
    sync_demo_data_module()?;

    output::print_remove(&file_path.display().to_string());
    output::print_update(CONSTRAINTS_MOD_PATH);
    output::print_update(CONSTRAINTS_ASSEMBLE_PATH);
    Ok(())
}

pub fn run_scalar_group(name: &str, skip_confirm: bool) -> CliResult {
    if !confirm_destroy("scalar group", name, skip_confirm)? {
        output::print_skip(&format!("scalar group {}", name));
        return Ok(());
    }

    crate::commands::model_resource::destroy_scalar_group(name)
}

pub fn run_conflict_repair(name: &str, skip_confirm: bool) -> CliResult {
    if !confirm_destroy("conflict repair", name, skip_confirm)? {
        output::print_skip(&format!("conflict repair {}", name));
        return Ok(());
    }

    crate::commands::model_resource::destroy_conflict_repair(name)
}

#[cfg(test)]
#[path = "destroy_tests.rs"]
mod tests;

/// Renders a project-relative path with `/` separators on every platform, so
/// messages name files the same way the docs and hints do.
fn slash_path(path: &Path) -> String {
    path.components()
        .map(|component| component.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}
