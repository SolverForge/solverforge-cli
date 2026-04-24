use std::fs;
use std::path::Path;

use crate::commands::generate_constraint::{parse_domain, validate_constraint_mod_source};
use crate::commands::generate_domain::{find_file_for_type, validate_domain_mod_source};
use crate::error::{CliError, CliResult};
use crate::managed_block;
use crate::output;

const ENTITY_TEMPLATE_PATH: &str = ".solverforge/templates/entity.rs.tmpl";
const SOLUTION_TEMPLATE_PATH: &str = ".solverforge/templates/solution.rs.tmpl";

pub fn run() -> CliResult {
    let domain_dir = Path::new("src/domain");
    if !domain_dir.exists() {
        return Err(CliError::NotInProject {
            missing: "src/domain/",
        });
    }

    let mut warnings = Vec::new();
    let mut errors = Vec::new();

    output::print_status("check", "src/domain/mod.rs");
    let domain_mod_path = domain_dir.join("mod.rs");
    let domain_mod_src = read_path(&domain_mod_path, &mut errors);
    let mut domain_mod_valid = false;
    if let Some(src) = domain_mod_src.as_deref() {
        if validate_managed_source(
            &domain_mod_path,
            src,
            &mut errors,
            validate_domain_mod_source,
        ) {
            domain_mod_valid = true;
            if let Ok(module_names) = validate_domain_mod_source(src) {
                for mod_name in module_names {
                    let file = domain_dir.join(format!("{mod_name}.rs"));
                    if !file.exists() {
                        errors.push(format!(
                            "Domain module '{}' declared in src/domain/mod.rs but file not found",
                            mod_name
                        ));
                    }
                }
            }
        }
    }

    let constraints_dir = Path::new("src/constraints");
    if constraints_dir.exists() {
        output::print_status("check", "src/constraints/mod.rs");
        let mod_path = constraints_dir.join("mod.rs");
        let constraints_mod_src = read_path(&mod_path, &mut errors);
        if let Some(src) = constraints_mod_src.as_deref() {
            if validate_managed_source(&mod_path, src, &mut errors, validate_constraint_mod_source)
            {
                if let Ok(module_names) = validate_constraint_mod_source(src) {
                    for mod_name in module_names {
                        let file = constraints_dir.join(format!("{mod_name}.rs"));
                        if !file.exists() {
                            errors.push(format!(
                                "Constraint module '{}' declared in src/constraints/mod.rs but file not found",
                                mod_name
                            ));
                        }
                    }
                }
            }
        }
    } else {
        warnings.push("src/constraints/ directory not found".to_string());
    }

    for (path, required_blocks) in [
        (ENTITY_TEMPLATE_PATH, managed_block::ENTITY_REQUIRED_BLOCKS),
        (
            SOLUTION_TEMPLATE_PATH,
            managed_block::SOLUTION_REQUIRED_BLOCKS,
        ),
    ] {
        let path = Path::new(path);
        if !path.exists() {
            continue;
        }
        output::print_status("check", &path.display().to_string());
        let src = read_path(path, &mut errors);
        if let Some(src) = src.as_deref() {
            validate_managed_source(path, src, &mut errors, |src| {
                managed_block::require_blocks(src, required_blocks)?;
                Ok(Vec::new())
            });
        }
    }

    if domain_mod_valid {
        output::print_status("check", "domain model");
        match parse_domain() {
            Ok(domain) => {
                let fact_fields = domain
                    .facts
                    .iter()
                    .map(|fact| fact.field_name.as_str())
                    .collect::<std::collections::BTreeSet<_>>();

                for entity in &domain.entities {
                    if entity.scalar_vars.is_empty() && entity.list_vars.is_empty() {
                        warnings.push(format!(
                            "Entity '{}' has no solvable fields — solver cannot optimize it",
                            entity.item_type
                        ));
                    }

                    for variable in &entity.scalar_vars {
                        if !variable.value_range.is_empty()
                            && !fact_fields.contains(variable.value_range.as_str())
                        {
                            errors.push(format!(
                            "Entity '{}.{}' references missing fact collection '{}' via value_range",
                            entity.item_type, variable.field, variable.value_range
                        ));
                        }
                    }

                    for variable in &entity.list_vars {
                        if variable.element_collection.is_empty() {
                            errors.push(format!(
                                "Entity '{}.{}' is missing element_collection",
                                entity.item_type, variable.field
                            ));
                        } else if !fact_fields.contains(variable.element_collection.as_str()) {
                            errors.push(format!(
                            "Entity '{}.{}' references missing fact collection '{}' via element_collection",
                            entity.item_type, variable.field, variable.element_collection
                        ));
                        }
                    }
                }

                if domain.solution_type.is_empty() {
                    errors.push("No planning solution found".to_string());
                } else {
                    match find_file_for_type(domain_dir, &domain.solution_type) {
                        Ok(path) => {
                            output::print_status("check", &path.display().to_string());
                            if let Some(src) = read_path(&path, &mut errors) {
                                validate_managed_source(&path, &src, &mut errors, |src| {
                                    managed_block::require_blocks(
                                        src,
                                        managed_block::SOLUTION_REQUIRED_BLOCKS,
                                    )?;
                                    Ok(Vec::new())
                                });
                            }
                        }
                        Err(err) => errors.push(format!(
                            "planning solution '{}' source not found: {}",
                            domain.solution_type, err
                        )),
                    }
                }

                for entity in &domain.entities {
                    match find_file_for_type(domain_dir, &entity.item_type) {
                        Ok(path) => {
                            output::print_status("check", &path.display().to_string());
                            if let Some(src) = read_path(&path, &mut errors) {
                                validate_managed_source(&path, &src, &mut errors, |src| {
                                    managed_block::require_blocks(
                                        src,
                                        managed_block::ENTITY_REQUIRED_BLOCKS,
                                    )?;
                                    Ok(Vec::new())
                                });
                            }
                        }
                        Err(err) => errors.push(format!(
                            "entity '{}' source not found: {}",
                            entity.item_type, err
                        )),
                    }
                }
            }
            Err(err) => errors.push(format!("Cannot parse domain model: {err}")),
        }
    }

    output::print_status("check", "solver.toml");
    if !Path::new("solver.toml").exists() {
        warnings.push("solver.toml not found — solver will use defaults".to_string());
    }

    println!();

    if errors.is_empty() && warnings.is_empty() {
        output::print_success("  All checks passed.");
        return Ok(());
    }

    for warning in &warnings {
        println!("  warning: {}", warning);
    }
    for error in &errors {
        println!("  error: {}", error);
    }
    println!();

    if !errors.is_empty() {
        return Err(CliError::general(format!(
            "{} error(s), {} warning(s)",
            errors.len(),
            warnings.len()
        )));
    }

    println!("  {} warning(s), 0 errors", warnings.len());
    Ok(())
}

fn read_path(path: &Path, errors: &mut Vec<String>) -> Option<String> {
    if !path.exists() {
        errors.push(format!("{} not found", path.display()));
        return None;
    }

    fs::read_to_string(path).map_or_else(
        |err| {
            errors.push(format!("failed to read {}: {}", path.display(), err));
            None
        },
        Some,
    )
}

fn validate_managed_source<F>(
    path: &Path,
    src: &str,
    errors: &mut Vec<String>,
    validator: F,
) -> bool
where
    F: FnOnce(&str) -> Result<Vec<String>, String>,
{
    match validator(src) {
        Ok(_) => true,
        Err(err) => {
            errors.push(format!("{}: {}", path.display(), err));
            false
        }
    }
}

#[cfg(test)]
#[path = "check_tests.rs"]
mod tests;
