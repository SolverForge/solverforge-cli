use std::fs;
use std::path::Path;

use crate::commands::generate_constraint::remove_constraint_from_source;
use crate::commands::generate_constraint::{domain::DomainModel, parse_domain};
use crate::commands::generate_domain::{find_file_for_type, snake_to_pascal};
use crate::commands::generate_domain::{
    remove_domain_mod_entry_source, unwire_collection_from_solution_source,
};
use crate::error::{CliError, CliResult};
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
            if !variable.value_range.is_empty() && variable.value_range == field_name {
                references.push(format!(
                    "{}.{} (value_range = \"{}\")",
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

pub fn run_variable(field: &str, entity: &str, skip_confirm: bool) -> CliResult {
    if !confirm_destroy("variable", field, skip_confirm)? {
        output::print_skip(&format!("variable {}", field));
        return Ok(());
    }

    generate_domain::destroy_variable(field, entity)
}

pub fn run_constraint(name: &str, skip_confirm: bool) -> CliResult {
    let snake = name.to_lowercase().replace('-', "_");
    let file_path = Path::new("src/constraints").join(format!("{}.rs", snake));
    let mod_path = Path::new("src/constraints/mod.rs");

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

    if !mod_path.exists() {
        return Err(CliError::NotInProject {
            missing: "src/constraints/mod.rs",
        });
    }

    let mod_src = fs::read_to_string(mod_path).map_err(|e| CliError::IoError {
        context: "failed to read src/constraints/mod.rs".to_string(),
        source: e,
    })?;
    let new_mod_src = remove_constraint_from_source(&mod_src, &snake).map_err(CliError::general)?;

    fs::write(mod_path, new_mod_src).map_err(|e| CliError::IoError {
        context: "failed to update src/constraints/mod.rs".to_string(),
        source: e,
    })?;
    fs::remove_file(&file_path).map_err(|e| CliError::IoError {
        context: format!("failed to delete {}", file_path.display()),
        source: e,
    })?;
    crate::commands::sf_config::remove_constraint(&snake)?;
    app_spec::sync_from_project()?;

    output::print_remove(&file_path.display().to_string());
    output::print_update("src/constraints/mod.rs");
    Ok(())
}

#[cfg(test)]
#[path = "destroy_tests.rs"]
mod tests;
