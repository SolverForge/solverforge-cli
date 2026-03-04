mod generators;
mod utils;
mod wiring;

#[cfg(test)]
mod tests;

pub(crate) use utils::{find_file_for_type, snake_to_pascal};

use std::fs;
use std::path::Path;

use crate::commands::add_constraint::parse_domain;
use generators::{generate_entity, generate_fact, generate_solution};
use utils::{ensure_domain_dir, print_created, print_updated, validate_score_type};
use wiring::{inject_planning_variable, replace_score_type, update_domain_mod, wire_collection_into_solution};

// ─── Entry points ─────────────────────────────────────────────────────────────

pub fn run_entity(name: &str, planning_variable: Option<&str>) -> Result<(), String> {
    use crate::commands::add_constraint::validate_name;
    validate_name(name)?;
    if let Some(var) = planning_variable {
        validate_name(var)?;
    }

    let domain_dir = Path::new("src/domain");
    ensure_domain_dir(domain_dir)?;

    let pascal = snake_to_pascal(name);
    let file_path = domain_dir.join(format!("{}.rs", name));
    if file_path.exists() {
        return Err(format!("'{}' already exists", file_path.display()));
    }

    let src = generate_entity(&pascal, planning_variable);
    fs::write(&file_path, src).map_err(|e| format!("failed to write {}: {}", file_path.display(), e))?;

    update_domain_mod(name, &pascal)?;
    wire_collection_into_solution(name, &pascal, "planning_entity_collection")?;

    print_created(file_path.to_str().unwrap());
    print_updated("src/domain/mod.rs");
    Ok(())
}

pub fn run_fact(name: &str) -> Result<(), String> {
    use crate::commands::add_constraint::validate_name;
    validate_name(name)?;

    let domain_dir = Path::new("src/domain");
    ensure_domain_dir(domain_dir)?;

    let pascal = snake_to_pascal(name);
    let file_path = domain_dir.join(format!("{}.rs", name));
    if file_path.exists() {
        return Err(format!("'{}' already exists", file_path.display()));
    }

    let src = generate_fact(&pascal);
    fs::write(&file_path, src).map_err(|e| format!("failed to write {}: {}", file_path.display(), e))?;

    update_domain_mod(name, &pascal)?;
    wire_collection_into_solution(name, &pascal, "problem_fact_collection")?;

    print_created(file_path.to_str().unwrap());
    print_updated("src/domain/mod.rs");
    Ok(())
}

pub fn run_solution(name: &str, score: &str) -> Result<(), String> {
    use crate::commands::add_constraint::validate_name;
    validate_name(name)?;
    validate_score_type(score)?;

    let domain_dir = Path::new("src/domain");
    ensure_domain_dir(domain_dir)?;

    // Check if any solution already exists
    if let Some(domain) = parse_domain() {
        return Err(format!(
            "a planning solution '{}' already exists — use `solverforge add score` to change the score type",
            domain.solution_type
        ));
    }

    let pascal = snake_to_pascal(name);
    let file_path = domain_dir.join(format!("{}.rs", name));
    if file_path.exists() {
        return Err(format!("'{}' already exists", file_path.display()));
    }

    let src = generate_solution(&pascal, score);
    fs::write(&file_path, src).map_err(|e| format!("failed to write {}: {}", file_path.display(), e))?;

    update_domain_mod(name, &pascal)?;

    print_created(file_path.to_str().unwrap());
    print_updated("src/domain/mod.rs");
    Ok(())
}

pub fn run_variable(field: &str, entity: &str) -> Result<(), String> {
    use crate::commands::add_constraint::validate_name;
    validate_name(field)?;

    let domain_dir = Path::new("src/domain");
    if !domain_dir.exists() {
        return Err("not a SolverForge project directory (src/domain/ not found)".to_string());
    }

    let entity_file = find_file_for_type(domain_dir, entity)?;

    let src = fs::read_to_string(&entity_file)
        .map_err(|e| format!("failed to read {}: {}", entity_file.display(), e))?;

    let new_src = inject_planning_variable(&src, entity, field)?;
    fs::write(&entity_file, new_src)
        .map_err(|e| format!("failed to write {}: {}", entity_file.display(), e))?;

    print_updated(entity_file.to_str().unwrap());
    Ok(())
}

pub fn run_score(score_type: &str) -> Result<(), String> {
    validate_score_type(score_type)?;

    let domain_dir = Path::new("src/domain");
    if !domain_dir.exists() {
        return Err("not a SolverForge project directory (src/domain/ not found)".to_string());
    }

    let domain = parse_domain().ok_or("no planning solution found in src/domain/")?;
    let solution_file = find_file_for_type(domain_dir, &domain.solution_type)?;

    let src = fs::read_to_string(&solution_file)
        .map_err(|e| format!("failed to read {}: {}", solution_file.display(), e))?;

    let new_src = replace_score_type(&src, &domain.score_type, score_type)?;
    fs::write(&solution_file, new_src)
        .map_err(|e| format!("failed to write {}: {}", solution_file.display(), e))?;

    print_updated(solution_file.to_str().unwrap());
    Ok(())
}
