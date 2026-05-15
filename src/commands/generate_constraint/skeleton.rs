use super::domain::DomainModel;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Pattern {
    Unary,
    Pair,
    Join,
    Balance,
    Reward,
    Runs,
    IndexedPresence,
    CollectVec,
    GroupComplement,
    ProjectedGroup,
}

pub(crate) fn generate_skeleton(
    _name: &str,
    pattern: Pattern,
    is_soft: bool,
    solution_type: &str,
    score_type: &str,
    constraint_name: &str,
    domain: Option<&DomainModel>,
) -> String {
    let hardness_comment = if is_soft {
        "SOFT: TODO — describe what this constraint optimizes."
    } else {
        "HARD: TODO — describe what this constraint enforces."
    };

    // Pick the first entity and fact from domain (if available)
    let entity = domain.and_then(|d| d.entities.first());
    let fact = domain.and_then(|d| d.facts.first());

    let entity_field = entity.map(|e| e.field_name.as_str()).unwrap_or("entities");
    let entity_type = entity
        .map(|e| e.item_type.as_str())
        .unwrap_or(solution_type);
    let planning_var = entity
        .and_then(|e| e.scalar_vars.first())
        .map(|s| s.field.as_str())
        .unwrap_or("value");

    let fact_field = fact.map(|f| f.field_name.as_str()).unwrap_or("facts");
    let fact_type = fact.map(|f| f.item_type.as_str()).unwrap_or("Fact");

    // Build import line(s)
    let imports = match pattern {
        Pattern::Join | Pattern::ProjectedGroup => {
            if fact.is_some() && solution_type != entity_type {
                format!(
                    "use crate::domain::{{{solution_type}, {entity_type}, {fact_type}}};\nuse solverforge::prelude::*;\nuse solverforge::stream::joiner::equal_bi;\nuse solverforge::IncrementalConstraint;",
                )
            } else if fact.is_some() {
                format!(
                    "use crate::domain::{{{solution_type}, {fact_type}}};\nuse solverforge::prelude::*;\nuse solverforge::stream::joiner::equal_bi;\nuse solverforge::IncrementalConstraint;",
                )
            } else {
                format!(
                    "use crate::domain::{{{solution_type}, {entity_type}}};\nuse solverforge::prelude::*;\nuse solverforge::stream::joiner::equal_bi;\nuse solverforge::IncrementalConstraint;",
                )
            }
        }
        Pattern::Balance => format!(
            "use crate::domain::{{{solution_type}, {entity_type}}};\nuse solverforge::prelude::*;\nuse solverforge::stream::collector::LoadBalance;\nuse solverforge::IncrementalConstraint;",
        ),
        Pattern::GroupComplement => {
            if fact.is_some() && solution_type != entity_type {
                format!(
                    "use crate::domain::{{{solution_type}, {entity_type}, {fact_type}}};\nuse solverforge::prelude::*;\nuse solverforge::IncrementalConstraint;",
                )
            } else if fact.is_some() {
                format!(
                    "use crate::domain::{{{solution_type}, {fact_type}}};\nuse solverforge::prelude::*;\nuse solverforge::IncrementalConstraint;",
                )
            } else {
                format!(
                    "use crate::domain::{{{solution_type}, {entity_type}}};\nuse solverforge::prelude::*;\nuse solverforge::IncrementalConstraint;",
                )
            }
        }
        _ => format!(
            "use crate::domain::{{{solution_type}, {entity_type}}};\nuse solverforge::prelude::*;\nuse solverforge::IncrementalConstraint;",
        ),
    };

    let penalty_expr = if is_soft {
        format!("<{score_type} as Score>::one_soft()")
    } else {
        format!("<{score_type} as Score>::one_hard()")
    };
    let unary_penalty = dynamic_penalty_arg("unary_weight", is_soft);
    let pair_penalty = dynamic_penalty_arg("pair_weight", is_soft);
    let join_penalty = dynamic_penalty_arg("join_weight", is_soft);
    let balance_penalty = dynamic_penalty_arg("balance_weight", is_soft);
    let runs_penalty = dynamic_penalty_arg("runs_weight", is_soft);
    let indexed_presence_penalty = dynamic_penalty_arg("indexed_presence_weight", is_soft);
    let collected_values_penalty = dynamic_penalty_arg("collected_values_weight", is_soft);
    let group_complement_penalty = dynamic_penalty_arg("group_complement_weight", is_soft);
    let projected_group_penalty = dynamic_penalty_arg("projected_group_weight", is_soft);

    let (body, helpers) = match pattern {
        Pattern::Unary => {
            let action = if is_soft {
                "        .reward(unary_weight)".to_string()
            } else {
                format!("        .penalize({unary_penalty})")
            };
            (
                format!(
                    r#"    ConstraintFactory::<{solution_type}, {score_type}>::new()
        .for_each(entity_items)
{action}
        .named("{constraint_name}")"#
                ),
                format!(
                    r#"

fn entity_items(solution: &{solution_type}) -> &[{entity_type}] {{
    solution.{entity_field}.as_slice()
}}

fn unary_condition(_entity: &{entity_type}) -> bool {{
    panic!("replace placeholder condition before enabling this constraint")
}}

fn unary_weight(entity: &{entity_type}) -> {score_type} {{
    if unary_condition(entity) {{
        {penalty_expr}
    }} else {{
        <{score_type} as Score>::zero()
    }}
}}"#
                ),
            )
        }

        Pattern::Pair => (
            format!(
                r#"    ConstraintFactory::<{solution_type}, {score_type}>::new()
        .for_each(entity_items)
        .join(joiner::equal({planning_var}_join_key))
        .penalize({pair_penalty})
        .named("{constraint_name}")"#
            ),
            format!(
                r#"

fn entity_items(solution: &{solution_type}) -> &[{entity_type}] {{
    solution.{entity_field}.as_slice()
}}

fn {planning_var}_join_key(entity: &{entity_type}) -> Option<usize> {{
    entity.{planning_var}
}}

fn pair_condition(_left: &{entity_type}, _right: &{entity_type}) -> bool {{
    panic!("replace placeholder pair condition before enabling this constraint")
}}

fn pair_weight(left: &{entity_type}, right: &{entity_type}) -> {score_type} {{
    if pair_condition(left, right) {{
        {penalty_expr}
    }} else {{
        <{score_type} as Score>::zero()
    }}
}}"#
            ),
        ),

        Pattern::Join => (
            format!(
                r#"    ConstraintFactory::<{solution_type}, {score_type}>::new()
        .for_each(entity_items)
        .join((
            fact_items,
            equal_bi(
                entity_join_key,
                fact_join_key,
            ),
        ))
        .penalize({join_penalty})
        .named("{constraint_name}")"#
            ),
            format!(
                r#"

fn entity_items(solution: &{solution_type}) -> &[{entity_type}] {{
    solution.{entity_field}.as_slice()
}}

fn fact_items(solution: &{solution_type}) -> &[{fact_type}] {{
    solution.{fact_field}.as_slice()
}}

fn entity_join_key(entity: &{entity_type}) -> Option<usize> {{
    entity.{planning_var}
}}

fn fact_join_key(_fact: &{fact_type}) -> Option<usize> {{
    panic!("replace placeholder join key extractor before enabling this constraint")
}}

fn join_condition(_entity: &{entity_type}, _fact: &{fact_type}) -> bool {{
    panic!("replace placeholder join condition before enabling this constraint")
}}

fn join_weight(entity: &{entity_type}, fact: &{fact_type}) -> {score_type} {{
    if join_condition(entity, fact) {{
        {penalty_expr}
    }} else {{
        <{score_type} as Score>::zero()
    }}
}}"#
            ),
        ),

        Pattern::Balance => (
            format!(
                r#"    ConstraintFactory::<{solution_type}, {score_type}>::new()
        .for_each(entity_items)
        .group_by(
            balance_scope,
            load_balance(balance_group_key, balance_metric),
        )
        .penalize({balance_penalty})
        .named("{constraint_name}")"#
            ),
            format!(
                r#"

fn entity_items(solution: &{solution_type}) -> &[{entity_type}] {{
    solution.{entity_field}.as_slice()
}}

fn balance_scope(_entity: &{entity_type}) -> usize {{
    0
}}

fn balance_group_key(entity: &{entity_type}) -> Option<usize> {{
    entity.{planning_var}
}}

fn balance_metric(_entity: &{entity_type}) -> i64 {{
    1
}}

fn balance_condition(_scope: &usize, _load: &LoadBalance<Option<usize>>) -> bool {{
    panic!("replace placeholder balance condition before enabling this constraint")
}}

fn balance_weight(scope: &usize, load: &LoadBalance<Option<usize>>) -> {score_type} {{
    if balance_condition(scope, load) {{
        {penalty_expr}
    }} else {{
        <{score_type} as Score>::zero()
    }}
}}"#
            ),
        ),

        Pattern::Reward => (
            format!(
                r#"    ConstraintFactory::<{solution_type}, {score_type}>::new()
        .for_each(entity_items)
        .reward(reward_weight)
        .named("{constraint_name}")"#
            ),
            format!(
                r#"

fn entity_items(solution: &{solution_type}) -> &[{entity_type}] {{
    solution.{entity_field}.as_slice()
}}

fn reward_condition(_entity: &{entity_type}) -> bool {{
    panic!("replace placeholder reward condition before enabling this constraint")
}}

fn reward_weight(entity: &{entity_type}) -> {score_type} {{
    if reward_condition(entity) {{
        {penalty_expr}
    }} else {{
        <{score_type} as Score>::zero()
    }}
}}"#
            ),
        ),

        Pattern::Runs => (
            format!(
                r#"    ConstraintFactory::<{solution_type}, {score_type}>::new()
        .for_each(entity_items)
        .filter(has_{planning_var})
        .group_by(
            {planning_var}_group_key,
            consecutive_runs({planning_var}_run_index),
        )
        .penalize({runs_penalty})
        .named("{constraint_name}")"#
            ),
            format!(
                r#"

fn entity_items(solution: &{solution_type}) -> &[{entity_type}] {{
    solution.{entity_field}.as_slice()
}}

fn has_{planning_var}(entity: &{entity_type}) -> bool {{
    entity.{planning_var}.is_some()
}}

fn {planning_var}_group_key(entity: &{entity_type}) -> usize {{
    entity.{planning_var}.unwrap_or(usize::MAX)
}}

fn {planning_var}_run_index(_entity: &{entity_type}) -> i64 {{
    panic!("replace placeholder run index extractor before enabling this constraint")
}}

fn runs_condition(_value_idx: &usize, _runs: &Runs) -> bool {{
    panic!("replace placeholder runs condition before enabling this constraint")
}}

fn runs_weight(value_idx: &usize, runs: &Runs) -> {score_type} {{
    if runs_condition(value_idx, runs) {{
        {penalty_expr}
    }} else {{
        <{score_type} as Score>::zero()
    }}
}}"#
            ),
        ),

        Pattern::IndexedPresence => (
            format!(
                r#"    ConstraintFactory::<{solution_type}, {score_type}>::new()
        .for_each(entity_items)
        .filter(has_{planning_var})
        .group_by(
            {planning_var}_group_key,
            indexed_presence({planning_var}_presence_index),
        )
        .penalize({indexed_presence_penalty})
        .named("{constraint_name}")"#
            ),
            format!(
                r#"

fn entity_items(solution: &{solution_type}) -> &[{entity_type}] {{
    solution.{entity_field}.as_slice()
}}

fn has_{planning_var}(entity: &{entity_type}) -> bool {{
    entity.{planning_var}.is_some()
}}

fn {planning_var}_group_key(entity: &{entity_type}) -> usize {{
    entity.{planning_var}.unwrap_or(usize::MAX)
}}

fn {planning_var}_presence_index(_entity: &{entity_type}) -> i64 {{
    panic!("replace placeholder presence index extractor before enabling this constraint")
}}

fn indexed_presence_condition(_value_idx: &usize, _presence: &IndexedPresence) -> bool {{
    panic!("replace placeholder presence condition before enabling this constraint")
}}

fn indexed_presence_weight(value_idx: &usize, presence: &IndexedPresence) -> {score_type} {{
    if indexed_presence_condition(value_idx, presence) {{
        {penalty_expr}
    }} else {{
        <{score_type} as Score>::zero()
    }}
}}"#
            ),
        ),

        Pattern::CollectVec => (
            format!(
                r#"    ConstraintFactory::<{solution_type}, {score_type}>::new()
        .for_each(entity_items)
        .filter(has_{planning_var})
        .group_by(
            {planning_var}_group_key,
            collect_vec(collected_value),
        )
        .penalize({collected_values_penalty})
        .named("{constraint_name}")"#
            ),
            format!(
                r#"

fn entity_items(solution: &{solution_type}) -> &[{entity_type}] {{
    solution.{entity_field}.as_slice()
}}

fn has_{planning_var}(entity: &{entity_type}) -> bool {{
    entity.{planning_var}.is_some()
}}

fn {planning_var}_group_key(entity: &{entity_type}) -> usize {{
    entity.{planning_var}.unwrap_or(usize::MAX)
}}

fn collected_value(_entity: &{entity_type}) -> usize {{
    panic!("replace placeholder collected value before enabling this constraint")
}}

fn collected_values_condition(_value_idx: &usize, _items: &CollectedVec<usize>) -> bool {{
    panic!("replace placeholder collected-vector condition before enabling this constraint")
}}

fn collected_values_weight(value_idx: &usize, items: &CollectedVec<usize>) -> {score_type} {{
    if collected_values_condition(value_idx, items) {{
        {penalty_expr}
    }} else {{
        <{score_type} as Score>::zero()
    }}
}}"#
            ),
        ),

        Pattern::GroupComplement => (
            format!(
                r#"    ConstraintFactory::<{solution_type}, {score_type}>::new()
        .for_each(entity_items)
        .filter(has_{planning_var})
        .group_by(
            {planning_var}_group_key,
            count(),
        )
        .complement(
            fact_items,
            complement_group_key,
            complement_default_count,
        )
        .penalize({group_complement_penalty})
        .named("{constraint_name}")"#
            ),
            format!(
                r#"

fn entity_items(solution: &{solution_type}) -> &[{entity_type}] {{
    solution.{entity_field}.as_slice()
}}

fn fact_items(solution: &{solution_type}) -> &[{fact_type}] {{
    solution.{fact_field}.as_slice()
}}

fn has_{planning_var}(entity: &{entity_type}) -> bool {{
    entity.{planning_var}.is_some()
}}

fn {planning_var}_group_key(entity: &{entity_type}) -> usize {{
    entity.{planning_var}.unwrap_or(usize::MAX)
}}

fn complement_group_key(_fact: &{fact_type}) -> usize {{
    panic!("replace placeholder complement key extractor before enabling this constraint")
}}

fn complement_default_count(_fact: &{fact_type}) -> usize {{
    0
}}

fn group_complement_condition(_key: &usize, _count: &usize) -> bool {{
    panic!("replace placeholder complement condition before enabling this constraint")
}}

fn group_complement_weight(key: &usize, count: &usize) -> {score_type} {{
    if group_complement_condition(key, count) {{
        {penalty_expr}
    }} else {{
        <{score_type} as Score>::zero()
    }}
}}"#
            ),
        ),

        Pattern::ProjectedGroup => (
            format!(
                r#"    ConstraintFactory::<{solution_type}, {score_type}>::new()
        .for_each(entity_items)
        .join((
            fact_items,
            equal_bi(
                entity_join_key,
                fact_join_key,
            ),
        ))
        .project(projected_group_entry)
        .group_by(
            projected_group_key,
            count(),
        )
        .penalize({projected_group_penalty})
        .named("{constraint_name}")"#
            ),
            format!(
                r#"

struct ProjectedGroupEntry {{
    group_key: usize,
}}

fn entity_items(solution: &{solution_type}) -> &[{entity_type}] {{
    solution.{entity_field}.as_slice()
}}

fn fact_items(solution: &{solution_type}) -> &[{fact_type}] {{
    solution.{fact_field}.as_slice()
}}

fn entity_join_key(entity: &{entity_type}) -> Option<usize> {{
    entity.{planning_var}
}}

fn fact_join_key(_fact: &{fact_type}) -> Option<usize> {{
    panic!("replace placeholder join key extractor before enabling this constraint")
}}

fn projected_group_entry(_entity: &{entity_type}, _fact: &{fact_type}) -> ProjectedGroupEntry {{
    panic!("replace placeholder projected-group entry before enabling this constraint")
}}

fn projected_group_key(entry: &ProjectedGroupEntry) -> usize {{
    entry.group_key
}}

fn projected_group_condition(_key: &usize, _count: &usize) -> bool {{
    panic!("replace placeholder projected-group condition before enabling this constraint")
}}

fn projected_group_weight(key: &usize, count: &usize) -> {score_type} {{
    if projected_group_condition(key, count) {{
        {penalty_expr}
    }} else {{
        <{score_type} as Score>::zero()
    }}
}}"#
            ),
        ),
    };

    format!(
        "{imports}\n\n/// {hardness_comment}\npub fn constraint() -> impl IncrementalConstraint<{solution_type}, {score_type}> {{\n{body}\n}}{helpers}\n"
    )
}

fn dynamic_penalty_arg(weight_fn: &str, is_soft: bool) -> String {
    if is_soft {
        weight_fn.to_string()
    } else {
        format!("hard_weight({weight_fn})")
    }
}
