use std::fs;
use std::path::Path;

use crate::commands::generate_constraint::{
    parse_domain, validate_constraint_mod_source, ConstraintSources, CONSTRAINTS_ASSEMBLE_PATH,
    CONSTRAINTS_MOD_PATH,
};
use crate::commands::generate_domain::{find_file_for_type, validate_domain_mod_source};
use crate::error::{CliError, CliResult};
use crate::managed_block;
use crate::model_contract::{ContractSeverity, ModelContract};
use crate::output;
use crate::solver_config;

const ENTITY_TEMPLATE_PATH: &str = ".solverforge/templates/entity.rs.tmpl";
const SOLUTION_TEMPLATE_PATH: &str = ".solverforge/templates/solution.rs.tmpl";
/// `check` validates structure only; say so instead of implying the project builds.
const STRUCTURAL_CHECKS_PASSED: &str =
    "Structural checks passed: managed blocks, domain model, model resources, solver.toml.";
const NOT_COMPILED_NOTE: &str = "The project was not compiled. Run `cargo build` to compile it.";

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
        output::print_status("check", CONSTRAINTS_MOD_PATH);
        output::print_status("check", CONSTRAINTS_ASSEMBLE_PATH);
        let module = read_path(Path::new(CONSTRAINTS_MOD_PATH), &mut errors);
        let assemble = read_path(Path::new(CONSTRAINTS_ASSEMBLE_PATH), &mut errors);
        if let (Some(module), Some(assemble)) = (module, assemble) {
            match validate_constraint_mod_source(&ConstraintSources { module, assemble }) {
                Ok(module_names) => {
                    for mod_name in module_names {
                        let file = constraints_dir.join(format!("{mod_name}.rs"));
                        if !file.exists() {
                            errors.push(format!(
                                "Constraint module '{mod_name}' declared in {CONSTRAINTS_MOD_PATH} but file not found"
                            ));
                        }
                    }
                }
                Err(err) => errors.push(err),
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
        output::print_status("check", &output::display_path(path));
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
                        let provider = variable.value_range_provider.as_str();
                        // A provider names a solution fact collection or a
                        // `Vec<usize>` field on the same entity, which gives
                        // each entity its own legal domain.
                        if !provider.is_empty()
                            && !fact_fields.contains(provider)
                            && !entity
                                .entity_value_ranges
                                .iter()
                                .any(|field| field == provider)
                        {
                            errors.push(format!(
                            "Entity '{}.{}' references value_range_provider '{}', which is neither a fact collection nor a Vec<usize> field on the entity",
                            entity.item_type, variable.field, variable.value_range_provider
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
                            output::print_status("check", &output::display_path(&path));
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
                            output::print_status("check", &output::display_path(&path));
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

                output::print_status("check", "model resources");
                match ModelContract::load() {
                    Ok(contract) => {
                        for diagnostic in contract.validate_model_resources() {
                            match diagnostic.severity {
                                ContractSeverity::Error => errors.push(diagnostic.message),
                            }
                        }
                    }
                    Err(err) => errors.push(format!("Cannot validate model resources: {err}")),
                }
            }
            Err(err) => errors.push(format!("Cannot parse domain model: {err}")),
        }
    }

    output::print_status("check", "solver.toml");
    if !Path::new("solver.toml").exists() {
        warnings.push("solver.toml not found — solver will use defaults".to_string());
    } else if let Some(src) = read_path(Path::new("solver.toml"), &mut errors) {
        if let Err(err) = solver_config::validate_managed_blocks(&src) {
            errors.push(err.to_string());
        }
        if let Err(err) = solver_config::validate_current_settings(&src) {
            errors.push(err.to_string());
        }
    }

    println!();

    if errors.is_empty() && warnings.is_empty() {
        output::print_success(&format!("  {STRUCTURAL_CHECKS_PASSED}"));
        println!("  {NOT_COMPILED_NOTE}");
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
    println!("  {NOT_COMPILED_NOTE}");
    Ok(())
}

fn read_path(path: &Path, errors: &mut Vec<String>) -> Option<String> {
    if !path.exists() {
        errors.push(format!("{} not found", output::display_path(path)));
        return None;
    }

    fs::read_to_string(path).map_or_else(
        |err| {
            errors.push(format!(
                "failed to read {}: {}",
                output::display_path(path),
                err
            ));
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
            errors.push(format!("{}: {}", output::display_path(path), err));
            false
        }
    }
}

#[cfg(test)]
#[path = "check_tests.rs"]
mod tests;
