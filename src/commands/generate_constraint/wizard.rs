use dialoguer::{theme::ColorfulTheme, Select};
use owo_colors::OwoColorize;

use super::domain::DomainModel;
use super::skeleton::Pattern;

#[derive(Debug, Clone, Copy)]
pub(crate) struct PatternFlags {
    pub soft: bool,
    pub unary: bool,
    pub pair: bool,
    pub join: bool,
    pub balance: bool,
    pub reward: bool,
    pub runs: bool,
    pub presence: bool,
    pub collect_vec: bool,
    pub group_complement: bool,
    pub projected_group: bool,
}

pub(crate) fn resolve_pattern_and_hardness(
    flags: PatternFlags,
    domain: &DomainModel,
) -> Result<(Pattern, bool), String> {
    let explicit_pattern: Option<Pattern> = match (
        flags.unary,
        flags.pair,
        flags.join,
        flags.balance,
        flags.reward,
        flags.runs,
        flags.presence,
        flags.collect_vec,
        flags.group_complement,
        flags.projected_group,
    ) {
        (true, _, _, _, _, _, _, _, _, _) => Some(Pattern::Unary),
        (_, true, _, _, _, _, _, _, _, _) => Some(Pattern::Pair),
        (_, _, true, _, _, _, _, _, _, _) => Some(Pattern::Join),
        (_, _, _, true, _, _, _, _, _, _) => Some(Pattern::Balance),
        (_, _, _, _, true, _, _, _, _, _) => Some(Pattern::Reward),
        (_, _, _, _, _, true, _, _, _, _) => Some(Pattern::Runs),
        (_, _, _, _, _, _, true, _, _, _) => Some(Pattern::IndexedPresence),
        (_, _, _, _, _, _, _, true, _, _) => Some(Pattern::CollectVec),
        (_, _, _, _, _, _, _, _, true, _) => Some(Pattern::GroupComplement),
        (_, _, _, _, _, _, _, _, _, true) => Some(Pattern::ProjectedGroup),
        _ => None,
    };

    // balance and reward imply soft
    let pattern_implies_soft = matches!(
        explicit_pattern,
        Some(Pattern::Balance) | Some(Pattern::Reward)
    );
    let is_soft_explicit = flags.soft || pattern_implies_soft;

    match explicit_pattern {
        Some(p) => Ok((p, is_soft_explicit)),
        None => {
            // Interactive wizard
            run_wizard(flags.soft, domain)
        }
    }
}

fn run_wizard(soft_flag: bool, domain: &DomainModel) -> Result<(Pattern, bool), String> {
    println!("{} Scanning domain model...", "▸".bright_green());
    println!();
    println!("  Found: {}", domain.solution_type.bright_white().bold());
    for e in &domain.entities {
        let mut solvable_fields = Vec::new();
        solvable_fields.extend(
            e.scalar_vars
                .iter()
                .map(|field| format!("{} [scalar]", field.field)),
        );
        solvable_fields.extend(
            e.list_vars
                .iter()
                .map(|field| format!("{} [list]", field.field)),
        );
        let var_info = if solvable_fields.is_empty() {
            String::new()
        } else {
            format!("  — solvable fields: {}", solvable_fields.join(", "))
        };
        println!(
            "    Entities:  {} ({}){}",
            e.field_name.bright_cyan(),
            e.item_type,
            var_info
        );
    }
    for f in &domain.facts {
        println!(
            "    Facts:     {} ({})",
            f.field_name.bright_cyan(),
            f.item_type
        );
    }
    println!();

    // Pattern selection
    let has_join = !domain.facts.is_empty();

    let mut pattern_options: Vec<(&str, Pattern)> = vec![
        (
            "Penalize matching entities        (e.g. unassigned, invalid state)",
            Pattern::Unary,
        ),
        (
            "Penalize conflicting pairs        (e.g. overlapping shifts, double-booking)",
            Pattern::Pair,
        ),
    ];
    if has_join {
        pattern_options.push((
            "Penalize entity-fact mismatch     (e.g. missing skill, wrong location)",
            Pattern::Join,
        ));
    }
    pattern_options.push((
        "Balance assignments                (e.g. fair workload distribution)",
        Pattern::Balance,
    ));
    pattern_options.push((
        "Reward matching entities           (e.g. desired day, preferred shift)",
        Pattern::Reward,
    ));

    let labels: Vec<&str> = pattern_options.iter().map(|(l, _)| *l).collect();

    let pattern_idx = Select::with_theme(&ColorfulTheme::default())
        .with_prompt("Constraint type")
        .items(&labels)
        .default(0)
        .interact()
        .map_err(|e| format!("prompt error: {}", e))?;

    let pattern = pattern_options[pattern_idx].1;

    // Hard/soft selection (skip if pattern implies soft)
    let is_soft = if matches!(pattern, Pattern::Balance | Pattern::Reward) || soft_flag {
        true
    } else {
        let hardness_idx = Select::with_theme(&ColorfulTheme::default())
            .with_prompt("Hard or soft")
            .items([
                "Hard  (must be satisfied — correctness)",
                "Soft  (should be optimized — quality)",
            ])
            .default(0)
            .interact()
            .map_err(|e| format!("prompt error: {}", e))?;
        hardness_idx == 1
    };

    Ok((pattern, is_soft))
}
