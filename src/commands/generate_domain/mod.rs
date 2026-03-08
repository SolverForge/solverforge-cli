mod generators;
mod utils;
mod wiring;

#[cfg(test)]
mod tests;

pub(crate) use utils::{find_file_for_type, snake_to_pascal};

use std::fs;
use std::path::Path;

use crate::commands::generate_constraint::parse_domain;
use generators::{generate_entity, generate_fact, generate_solution};
use utils::{ensure_domain_dir, print_created, print_updated, validate_score_type};
use wiring::{
    inject_planning_variable, replace_score_type, update_domain_mod, wire_collection_into_solution,
};

// ─── Entry points ─────────────────────────────────────────────────────────────

pub fn run_entity(name: &str, planning_variable: Option<&str>) -> Result<(), String> {
    use crate::commands::generate_constraint::validate_name;
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
    fs::write(&file_path, src)
        .map_err(|e| format!("failed to write {}: {}", file_path.display(), e))?;

    update_domain_mod(name, &pascal)?;
    wire_collection_into_solution(name, &pascal, "planning_entity_collection")?;

    print_created(file_path.to_str().unwrap());
    print_updated("src/domain/mod.rs");
    Ok(())
}

pub fn run_fact(name: &str) -> Result<(), String> {
    use crate::commands::generate_constraint::validate_name;
    validate_name(name)?;

    let domain_dir = Path::new("src/domain");
    ensure_domain_dir(domain_dir)?;

    let pascal = snake_to_pascal(name);
    let file_path = domain_dir.join(format!("{}.rs", name));
    if file_path.exists() {
        return Err(format!("'{}' already exists", file_path.display()));
    }

    let src = generate_fact(&pascal);
    fs::write(&file_path, src)
        .map_err(|e| format!("failed to write {}: {}", file_path.display(), e))?;

    update_domain_mod(name, &pascal)?;
    wire_collection_into_solution(name, &pascal, "problem_fact_collection")?;

    print_created(file_path.to_str().unwrap());
    print_updated("src/domain/mod.rs");
    Ok(())
}

pub fn run_solution(name: &str, score: &str) -> Result<(), String> {
    use crate::commands::generate_constraint::validate_name;
    validate_name(name)?;
    validate_score_type(score)?;

    let domain_dir = Path::new("src/domain");
    ensure_domain_dir(domain_dir)?;

    // Check if any solution already exists
    if let Some(domain) = parse_domain() {
        // Check if this is the default scaffold
        if is_default_scaffold()? {
            // Remove default scaffold files and continue
            remove_default_scaffold()?;
        } else {
            return Err(format!(
                "a planning solution '{}' already exists — use `solverforge destroy solution` then `solverforge generate solution` to replace it",
                domain.solution_type
            ));
        }
    }

    let pascal = snake_to_pascal(name);
    let file_path = domain_dir.join(format!("{}.rs", name));
    if file_path.exists() {
        return Err(format!("'{}' already exists", file_path.display()));
    }

    let src = generate_solution(&pascal, score);
    fs::write(&file_path, src)
        .map_err(|e| format!("failed to write {}: {}", file_path.display(), e))?;

    update_domain_mod(name, &pascal)?;

    print_created(file_path.to_str().unwrap());
    print_updated("src/domain/mod.rs");
    Ok(())
}

pub fn run_variable(field: &str, entity: &str) -> Result<(), String> {
    use crate::commands::generate_constraint::validate_name;
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

// ─── Default scaffold detection and removal ──────────────────────────────────

fn is_default_scaffold() -> Result<bool, String> {
    let plan_path = Path::new("src/domain/plan.rs");
    if !plan_path.exists() {
        return Ok(false);
    }

    let content =
        fs::read_to_string(plan_path).map_err(|e| format!("Failed to read plan.rs: {}", e))?;

    // Check for the scaffold marker
    Ok(content.contains("Rename this to something domain-specific"))
}

fn remove_default_scaffold() -> Result<(), String> {
    // Remove default domain files
    let domain_files = ["plan.rs", "task.rs", "resource.rs"];
    for file in &domain_files {
        let path = Path::new("src/domain").join(file);
        if path.exists() {
            fs::remove_file(&path).map_err(|e| format!("Failed to remove {}: {}", file, e))?;
        }
    }

    // Clear src/domain/mod.rs
    let domain_mod = Path::new("src/domain/mod.rs");
    if domain_mod.exists() {
        fs::write(domain_mod, "// Domain module\n")
            .map_err(|e| format!("Failed to clear domain/mod.rs: {}", e))?;
    }

    // Remove all_assigned.rs from constraints
    let all_assigned = Path::new("src/constraints/all_assigned.rs");
    if all_assigned.exists() {
        fs::remove_file(all_assigned)
            .map_err(|e| format!("Failed to remove all_assigned.rs: {}", e))?;
    }

    // Update src/constraints/mod.rs to remove all_assigned
    let constraints_mod = Path::new("src/constraints/mod.rs");
    if constraints_mod.exists() {
        let content = fs::read_to_string(constraints_mod)
            .map_err(|e| format!("Failed to read constraints/mod.rs: {}", e))?;

        let lines: Vec<&str> = content
            .lines()
            .filter(|line| !line.contains("all_assigned"))
            .collect();

        let new_content = lines.join("\n");
        fs::write(constraints_mod, new_content)
            .map_err(|e| format!("Failed to update constraints/mod.rs: {}", e))?;
    }

    // Stub src/data/mod.rs
    let data_mod = Path::new("src/data/mod.rs");
    if data_mod.exists() {
        let stub_content = "// Data loading module\n\npub fn load() -> Result<(), Box<dyn std::error::Error>> {\n    todo!(\"Implement data loading\")\n}\n";
        fs::write(data_mod, stub_content)
            .map_err(|e| format!("Failed to stub data/mod.rs: {}", e))?;
    }

    println!("✓ Removed default scaffold");
    Ok(())
}
