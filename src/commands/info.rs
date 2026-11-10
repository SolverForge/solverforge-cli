use std::fs;
use std::path::Path;

use crate::commands::generate_constraint::{domain::list_constraints, parse_domain};
use crate::error::{CliError, CliResult};
use crate::output;

pub fn run() -> CliResult {
    let domain_dir = Path::new("src/domain");
    if !domain_dir.exists() {
        return Err(CliError::NotInProject {
            missing: "src/domain/",
        });
    }

    let project_name = read_project_name().unwrap_or_else(|| "<unknown>".to_string());
    output::print_heading(&format!("Project: {}", project_name));
    println!();

    let domain = parse_domain().map_err(CliError::general)?;
    println!("  Solution:    {}", domain.solution_type);
    println!("  Score type:  {}", domain.score_type);
    println!();

    if !domain.entities.is_empty() {
        println!("  Entities:");
        for entity in &domain.entities {
            let mut solvable_fields = Vec::new();
            solvable_fields.extend(
                entity
                    .scalar_vars
                    .iter()
                    .map(|field| format!("{} [scalar]", field.field)),
            );
            solvable_fields.extend(
                entity
                    .list_vars
                    .iter()
                    .map(|field| format!("{} [list]", field.field)),
            );

            if solvable_fields.is_empty() {
                println!("    - {}", entity.item_type);
            } else {
                println!(
                    "    - {} (solvable fields: {})",
                    entity.item_type,
                    solvable_fields.join(", ")
                );
            }
        }
        println!();
    }

    if !domain.facts.is_empty() {
        println!("  Facts:");
        for fact in &domain.facts {
            println!("    - {}", fact.item_type);
        }
        println!();
    }

    let constraints = list_constraints(Path::new(".")).map_err(CliError::general)?;
    if !constraints.is_empty() {
        println!("  Constraints:");
        for c in &constraints {
            println!("    - {}", c);
        }
        println!();
    }

    if Path::new("solver.toml").exists() {
        println!("  Config:      solver.toml");
    }

    Ok(())
}

fn read_project_name() -> Option<String> {
    let cargo = fs::read_to_string("Cargo.toml").ok()?;
    for line in cargo.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("name") && trimmed.contains('=') {
            let val = trimmed.split('=').nth(1)?.trim();
            return Some(val.trim_matches('"').to_string());
        }
    }
    None
}
