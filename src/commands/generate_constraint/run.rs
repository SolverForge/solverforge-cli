// ─── Public entry point ───────────────────────────────────────────────────────

use owo_colors::OwoColorize;
use std::fs;
use std::path::Path;

use super::domain::parse_domain;
use super::mod_rewriter::{extract_types, rewrite_mod};
use super::skeleton::generate_skeleton;
use super::utils::{snake_to_title, validate_name};
use super::wizard::resolve_pattern_and_hardness;

/// Runs `solverforge generate constraint <name> [pattern flags] [--hard|--soft]`.
pub fn run(
    name: &str,
    soft: bool,
    unary: bool,
    pair: bool,
    join: bool,
    balance: bool,
    reward: bool,
) -> Result<(), String> {
    validate_name(name)?;

    let constraints_dir = Path::new("src/constraints");
    let mod_path = constraints_dir.join("mod.rs");
    let new_file = constraints_dir.join(format!("{}.rs", name));

    if !constraints_dir.exists() {
        return Err("not a SolverForge project directory (src/constraints/ not found)".to_string());
    }
    if !mod_path.exists() {
        return Err("src/constraints/mod.rs not found".to_string());
    }
    if new_file.exists() {
        return Err(format!("constraint '{}' already exists", name));
    }

    let mod_src = fs::read_to_string(&mod_path)
        .map_err(|e| format!("failed to read src/constraints/mod.rs: {}", e))?;

    // Parse domain model; fall back gracefully
    let domain = parse_domain();

    // Resolve solution/score types: prefer domain parser, fall back to mod.rs extraction
    let (solution_type, score_type) = if let Some(ref d) = domain {
        (d.solution_type.clone(), d.score_type.clone())
    } else {
        extract_types(&mod_src)
    };

    let constraint_name = snake_to_title(name);

    // Determine pattern + hardness
    let (pattern, is_soft) =
        resolve_pattern_and_hardness(soft, unary, pair, join, balance, reward, &domain)?;

    // Generate and write the new constraint file
    let skeleton = generate_skeleton(
        name,
        pattern,
        is_soft,
        &solution_type,
        &score_type,
        &constraint_name,
        domain.as_ref(),
    );
    fs::write(&new_file, skeleton)
        .map_err(|e| format!("failed to write {}: {}", new_file.display(), e))?;

    // Rewrite mod.rs
    let new_mod = rewrite_mod(&mod_src, name);
    fs::write(&mod_path, new_mod)
        .map_err(|e| format!("failed to write src/constraints/mod.rs: {}", e))?;

    // Success output
    println!(
        "{} Created {}",
        "▸".bright_green(),
        format!("src/constraints/{}.rs", name).bright_cyan()
    );
    println!(
        "{} Updated {}",
        "▸".bright_green(),
        "src/constraints/mod.rs".bright_cyan()
    );
    println!();
    println!("  Next steps:");
    println!(
        "    1. Open {}",
        format!("src/constraints/{}.rs", name).bright_cyan()
    );
    println!(
        "    2. {}",
        "Replace the TODO placeholders with your domain logic".bright_black()
    );
    println!(
        "    {} {}  {}",
        "$".bright_black(),
        "solverforge server".bright_cyan(),
        "# test your constraint".bright_black()
    );
    println!();

    Ok(())
}
