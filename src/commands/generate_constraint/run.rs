use std::fs;
use std::path::Path;

use super::domain::parse_domain;
use super::mod_rewriter::rewrite_mod;
use super::skeleton::generate_skeleton;
use super::utils::validate_name;
use super::wizard::{resolve_pattern_and_hardness, PatternFlags};
use crate::app_spec;
use crate::commands::generate_domain::is_soft_score;
use crate::error::{CliError, CliResult};
use crate::output;

// Print lines present in `after` but not in `before`, prefixed with '+', when --verbose.
fn print_diff_verbose(before: &str, after: &str) {
    if !output::is_verbose() {
        return;
    }
    let before_lines: Vec<&str> = before.lines().collect();
    for line in after.lines() {
        if !before_lines.contains(&line) {
            println!("+ {}", line);
        }
    }
}

/// Runs `solverforge generate constraint <name> [pattern flags] [--hard|--soft]`.
#[allow(clippy::too_many_arguments)]
pub fn run(
    name: &str,
    soft: bool,
    unary: bool,
    pair: bool,
    join: bool,
    balance: bool,
    reward: bool,
    runs: bool,
    presence: bool,
    collect_vec: bool,
    group_complement: bool,
    projected_group: bool,
    force: bool,
    pretend: bool,
) -> CliResult {
    validate_name(name)?;

    let constraints_dir = Path::new("src/constraints");
    let mod_path = constraints_dir.join("mod.rs");
    let new_file = constraints_dir.join(format!("{}.rs", name));

    if !constraints_dir.exists() {
        return Err(CliError::NotInProject {
            missing: "src/constraints/",
        });
    }
    if !mod_path.exists() {
        return Err(CliError::NotInProject {
            missing: "src/constraints/mod.rs",
        });
    }
    if new_file.exists() && !force {
        return Err(CliError::ResourceExists {
            kind: "constraint",
            name: name.to_string(),
        });
    }

    let mod_src = fs::read_to_string(&mod_path).map_err(|e| CliError::IoError {
        context: "failed to read src/constraints/mod.rs".to_string(),
        source: e,
    })?;

    let domain = parse_domain().map_err(CliError::general)?;
    let solution_type = domain.solution_type.clone();
    let score_type = domain.score_type.clone();

    // Determine pattern + hardness
    let (pattern, is_soft) = resolve_pattern_and_hardness(
        PatternFlags {
            soft,
            unary,
            pair,
            join,
            balance,
            reward,
            runs,
            presence,
            collect_vec,
            group_complement,
            projected_group,
        },
        &domain,
    )?;
    if !is_soft && is_soft_score(&score_type) {
        return Err(CliError::general(
            "SoftScore supports only soft generated constraints; pass --soft or choose a score type with hard levels",
        ));
    }

    if matches!(
        pattern,
        super::skeleton::Pattern::Unary
            | super::skeleton::Pattern::Pair
            | super::skeleton::Pattern::Balance
            | super::skeleton::Pattern::Reward
            | super::skeleton::Pattern::Join
            | super::skeleton::Pattern::Runs
            | super::skeleton::Pattern::IndexedPresence
            | super::skeleton::Pattern::CollectVec
            | super::skeleton::Pattern::GroupComplement
            | super::skeleton::Pattern::ProjectedGroup
    ) && domain.entities.is_empty()
    {
        return Err(CliError::with_hint(
            "constraint generation needs at least one planning entity collection",
            "run `solverforge generate entity ...` first",
        ));
    }

    if matches!(
        pattern,
        super::skeleton::Pattern::Pair
            | super::skeleton::Pattern::Join
            | super::skeleton::Pattern::Balance
            | super::skeleton::Pattern::Runs
            | super::skeleton::Pattern::IndexedPresence
            | super::skeleton::Pattern::CollectVec
            | super::skeleton::Pattern::GroupComplement
            | super::skeleton::Pattern::ProjectedGroup
    ) && domain
        .entities
        .first()
        .is_some_and(|entity| entity.scalar_vars.is_empty())
    {
        return Err(CliError::with_hint(
            "this constraint pattern needs a scalar planning variable on the first planning entity collection",
            "run `solverforge generate variable <field> --entity <Entity> --kind scalar --range <facts>` first",
        ));
    }

    if matches!(
        pattern,
        super::skeleton::Pattern::Join
            | super::skeleton::Pattern::GroupComplement
            | super::skeleton::Pattern::ProjectedGroup
    ) && domain.facts.is_empty()
    {
        return Err(CliError::with_hint(
            "this constraint pattern needs at least one problem fact collection",
            "run `solverforge generate fact ...` first",
        ));
    }

    // Generate the new constraint file
    let skeleton = generate_skeleton(
        name,
        pattern,
        is_soft,
        &solution_type,
        &score_type,
        name,
        Some(&domain),
    );

    let new_mod = rewrite_mod(&mod_src, name).map_err(CliError::general)?;

    if pretend {
        println!("Would create src/constraints/{}.rs", name);
        println!("Would update src/constraints/mod.rs");
        return Ok(());
    }

    fs::write(&new_file, &skeleton).map_err(|e| CliError::IoError {
        context: format!("failed to write {}", new_file.display()),
        source: e,
    })?;

    fs::write(&mod_path, &new_mod).map_err(|e| CliError::IoError {
        context: "failed to write src/constraints/mod.rs".to_string(),
        source: e,
    })?;

    crate::commands::sf_config::add_constraint(name)?;
    app_spec::sync_from_project()?;

    output::print_create(&format!("src/constraints/{}.rs", name));
    print_diff_verbose("", &skeleton);
    output::print_update("src/constraints/mod.rs");
    print_diff_verbose(&mod_src, &new_mod);
    println!();
    if !output::is_quiet() {
        println!("  Next steps:");
        println!("    1. Open src/constraints/{}.rs", name);
        println!("    2. Replace the TODO placeholders with your domain logic");
        println!("    solverforge server  # test your constraint");
        println!();
    }

    Ok(())
}
