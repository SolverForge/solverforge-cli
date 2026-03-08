use std::fs;
use std::path::Path;

use crate::commands::generate_constraint::parse_domain;
use crate::commands::generate_domain::{find_file_for_type, snake_to_pascal};

// ─── Solution ──────────────────────────────────────────────────────────────────

pub fn run_solution() -> Result<(), String> {
    let domain =
        parse_domain().ok_or_else(|| "No planning solution found in src/domain/".to_string())?;

    let domain_dir = Path::new("src/domain");
    let solution_file = find_file_for_type(domain_dir, &domain.solution_type)?;

    // Extract the filename without extension
    let file_name = solution_file
        .file_stem()
        .and_then(|s| s.to_str())
        .ok_or_else(|| "Invalid solution file name".to_string())?;

    // Delete the solution file
    fs::remove_file(&solution_file)
        .map_err(|e| format!("Failed to delete {}: {}", solution_file.display(), e))?;

    // Remove from domain/mod.rs
    remove_from_domain_mod(file_name)?;

    println!("✓ Removed solution: {}", domain.solution_type);
    Ok(())
}

// ─── Entity ────────────────────────────────────────────────────────────────────

pub fn run_entity(name: &str) -> Result<(), String> {
    let domain = parse_domain().ok_or_else(|| "No domain model found".to_string())?;

    let snake = name.to_lowercase().replace('-', "_");
    let pascal = snake_to_pascal(&snake);

    // Check if entity exists by looking for it in the entities list
    let entity = domain
        .entities
        .iter()
        .find(|e| e.item_type == pascal)
        .ok_or_else(|| format!("Entity '{}' not found", name))?;

    // Try to find the file
    let domain_dir = Path::new("src/domain");
    let file_path = find_file_for_type(domain_dir, &pascal).or_else(|_| {
        // Fallback to snake_case filename
        let path = domain_dir.join(format!("{}.rs", snake));
        if path.exists() {
            Ok(path)
        } else {
            Err(format!("Entity file for {} not found", pascal))
        }
    })?;

    // Extract the filename without extension
    let file_name = file_path
        .file_stem()
        .and_then(|s| s.to_str())
        .ok_or_else(|| "Invalid entity file name".to_string())?;

    // Delete the entity file
    fs::remove_file(&file_path)
        .map_err(|e| format!("Failed to delete {}: {}", file_path.display(), e))?;

    // Remove from domain/mod.rs
    remove_from_domain_mod(file_name)?;

    // Unwire from solution
    unwire_collection_from_solution(&entity.field_name, &entity.item_type, &domain.solution_type)?;

    println!("✓ Removed entity: {}", pascal);
    Ok(())
}

// ─── Fact ──────────────────────────────────────────────────────────────────────

pub fn run_fact(name: &str) -> Result<(), String> {
    let domain = parse_domain().ok_or_else(|| "No domain model found".to_string())?;

    let snake = name.to_lowercase().replace('-', "_");
    let pascal = snake_to_pascal(&snake);

    // Check if fact exists by looking for it in the facts list
    let fact = domain
        .facts
        .iter()
        .find(|f| f.item_type == pascal)
        .ok_or_else(|| format!("Fact '{}' not found", name))?;

    // Try to find the file
    let domain_dir = Path::new("src/domain");
    let file_path = find_file_for_type(domain_dir, &pascal).or_else(|_| {
        // Fallback to snake_case filename
        let path = domain_dir.join(format!("{}.rs", snake));
        if path.exists() {
            Ok(path)
        } else {
            Err(format!("Fact file for {} not found", pascal))
        }
    })?;

    // Extract the filename without extension
    let file_name = file_path
        .file_stem()
        .and_then(|s| s.to_str())
        .ok_or_else(|| "Invalid fact file name".to_string())?;

    // Delete the fact file
    fs::remove_file(&file_path)
        .map_err(|e| format!("Failed to delete {}: {}", file_path.display(), e))?;

    // Remove from domain/mod.rs
    remove_from_domain_mod(file_name)?;

    // Unwire from solution
    unwire_collection_from_solution(&fact.field_name, &fact.item_type, &domain.solution_type)?;

    println!("✓ Removed fact: {}", pascal);
    Ok(())
}

// ─── Constraint ────────────────────────────────────────────────────────────────

pub fn run_constraint(name: &str) -> Result<(), String> {
    let snake = name.to_lowercase().replace('-', "_");
    let file_path = format!("src/constraints/{}.rs", snake);

    if !Path::new(&file_path).exists() {
        return Err(format!("Constraint file {} does not exist", file_path));
    }

    // Delete the constraint file
    fs::remove_file(&file_path).map_err(|e| format!("Failed to delete {}: {}", file_path, e))?;

    // Remove from constraints/mod.rs
    remove_constraint_from_mod(&snake)?;

    println!("✓ Removed constraint: {}", name);
    Ok(())
}

// ─── Helpers ───────────────────────────────────────────────────────────────────

fn remove_from_domain_mod(mod_name: &str) -> Result<(), String> {
    let mod_path = Path::new("src/domain/mod.rs");
    if !mod_path.exists() {
        return Ok(()); // Nothing to remove from
    }

    let content = fs::read_to_string(mod_path)
        .map_err(|e| format!("Failed to read src/domain/mod.rs: {}", e))?;

    // Remove mod declaration and pub use statement
    let lines: Vec<&str> = content.lines().collect();
    let mut new_lines = Vec::new();

    for line in lines {
        // Skip lines that declare or use this module
        if line.trim() == format!("mod {};", mod_name)
            || line.trim().starts_with(&format!("pub use {}::", mod_name))
        {
            continue;
        }
        new_lines.push(line);
    }

    let new_content = new_lines.join("\n");
    fs::write(mod_path, new_content)
        .map_err(|e| format!("Failed to update src/domain/mod.rs: {}", e))?;

    Ok(())
}

fn unwire_collection_from_solution(
    field_name: &str,
    type_name: &str,
    solution_type: &str,
) -> Result<(), String> {
    let domain_dir = Path::new("src/domain");
    let solution_file = find_file_for_type(domain_dir, solution_type)?;

    let content = fs::read_to_string(&solution_file)
        .map_err(|e| format!("Failed to read {}: {}", solution_file.display(), e))?;

    let mut lines: Vec<String> = content.lines().map(|s| s.to_string()).collect();
    let mut i = 0;

    while i < lines.len() {
        let line = &lines[i];

        // Remove field declaration (looking for the field_name as used in entities/facts)
        if line.contains(&format!("{}: Vec<{}>", field_name, type_name)) {
            // Also remove any annotations above it
            let mut start = i;
            while start > 0 && lines[start - 1].trim().starts_with('#') {
                start -= 1;
            }
            lines.drain(start..=i);
            i = start;
            continue;
        }

        // Remove from constructor
        if line.contains(&format!("{}: Vec::new()", field_name)) {
            lines.remove(i);
            continue;
        }

        // Remove use statement
        if line.trim() == format!("use super::{};", type_name) {
            lines.remove(i);
            continue;
        }

        i += 1;
    }

    let new_content = lines.join("\n");
    fs::write(&solution_file, new_content)
        .map_err(|e| format!("Failed to update {}: {}", solution_file.display(), e))?;

    Ok(())
}

fn remove_constraint_from_mod(name: &str) -> Result<(), String> {
    let mod_path = Path::new("src/constraints/mod.rs");
    if !mod_path.exists() {
        return Ok(()); // Nothing to remove from
    }

    let content = fs::read_to_string(mod_path)
        .map_err(|e| format!("Failed to read src/constraints/mod.rs: {}", e))?;

    let lines: Vec<&str> = content.lines().collect();
    let mut new_lines = Vec::new();
    let mut in_tuple = false;
    let mut removed_item = false;

    for line in lines {
        // Skip module declaration
        if line.trim() == format!("mod {};", name) {
            continue;
        }

        // Track if we're inside the tuple
        if line.contains("pub fn all() ->") || line.contains("impl Constraint") {
            in_tuple = true;
            new_lines.push(line);
        } else if in_tuple && line.contains(')') {
            // Clean up trailing comma if we removed the last item
            if removed_item && !new_lines.is_empty() {
                let last_idx = new_lines.len() - 1;
                if let Some(last) = new_lines.get_mut(last_idx) {
                    if last.trim().ends_with(',') {
                        *last = last.trim().trim_end_matches(',');
                    }
                }
            }
            in_tuple = false;
            new_lines.push(line);
        } else if in_tuple && line.contains(&format!("{}::", name)) {
            // Skip lines that reference this module in the tuple
            removed_item = true;
            continue;
        } else {
            new_lines.push(line);
        }
    }

    let result = new_lines.join("\n");

    fs::write(mod_path, result)
        .map_err(|e| format!("Failed to update src/constraints/mod.rs: {}", e))?;

    Ok(())
}
