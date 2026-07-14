use super::{
    domain::{DomainModel, EntityInfo, FactInfo, ScalarVarInfo},
    mod_rewriter::{remove_constraint_from_source, rewrite_mod, validate_constraint_mod_source},
    run::run as run_generate_constraint,
    skeleton::{generate_skeleton, Pattern},
    utils::validate_name,
};
use crate::scalar_variable_hooks::ScalarVariableHooks;
use crate::test_support;
use std::{
    fs,
    path::{Path, PathBuf},
};

struct CwdGuard {
    original_dir: PathBuf,
}

impl CwdGuard {
    fn enter(path: &Path) -> Self {
        let original_dir = std::env::current_dir().expect("failed to read current dir");
        std::env::set_current_dir(path).expect("failed to enter temp dir");
        Self { original_dir }
    }
}

impl Drop for CwdGuard {
    fn drop(&mut self) {
        std::env::set_current_dir(&self.original_dir).expect("failed to restore current dir");
    }
}

#[test]
fn test_validate_name() {
    assert!(validate_name("max_hours").is_ok());
    assert!(validate_name("required_skill").is_ok());
    assert!(validate_name("a").is_ok());
    assert!(validate_name("MaxHours").is_err());
    assert!(validate_name("1bad").is_err());
    assert!(validate_name("bad-name").is_err());
    assert!(validate_name("").is_err());
}

#[test]
fn test_rewrite_mod_appends_module_and_call_to_managed_blocks() {
    let src = r#"pub use self::assemble::create_constraints;

// @solverforge:begin constraint-modules
mod all_assigned;
// @solverforge:end constraint-modules

mod assemble {
    use super::*;

    pub fn create_constraints() -> impl ConstraintSet<Plan, HardSoftScore> {
        // @solverforge:begin constraint-calls
        (
            all_assigned::constraint(),
        )
        // @solverforge:end constraint-calls
    }
}
"#;
    let result = rewrite_mod(src, "max_hours").expect("rewrite should succeed");
    assert!(result.contains("mod max_hours;"));
    assert!(result.contains("max_hours::constraint(),"));
}

#[test]
fn test_remove_constraint_from_source_preserves_empty_tuple_shape() {
    let src = r#"pub use self::assemble::create_constraints;

// @solverforge:begin constraint-modules
mod all_assigned;
// @solverforge:end constraint-modules

mod assemble {
    use super::*;

    pub fn create_constraints() -> impl ConstraintSet<Plan, HardSoftScore> {
        // @solverforge:begin constraint-calls
        (
            all_assigned::constraint(),
        )
        // @solverforge:end constraint-calls
    }
}
"#;
    let result = remove_constraint_from_source(src, "all_assigned").expect("remove should succeed");
    assert!(!result.contains("mod all_assigned;"));
    assert!(result.contains("        ()"));
}

#[test]
fn test_rewrite_mod_flattens_nested_constraint_tuples() {
    let src = r#"pub use self::assemble::create_constraints;

// @solverforge:begin constraint-modules
mod all_assigned;
mod extra;
// @solverforge:end constraint-modules

mod assemble {
    use super::*;

    pub fn create_constraints() -> impl ConstraintSet<Plan, HardSoftScore> {
        // @solverforge:begin constraint-calls
        (
            (
                all_assigned::constraint(),
                extra::constraint(),
            ),
        )
        // @solverforge:end constraint-calls
    }
}
"#;

    let result = rewrite_mod(src, "capacity").expect("rewrite should succeed");
    assert!(result.contains("mod capacity;"));
    assert!(result.contains("capacity::constraint(),"));
    assert!(result.contains("all_assigned::constraint(),"));
    assert!(result.contains("extra::constraint(),"));
}

#[test]
fn test_remove_constraint_from_source_handles_nested_tuple_calls() {
    let src = r#"pub use self::assemble::create_constraints;

// @solverforge:begin constraint-modules
mod one;
mod two;
mod three;
// @solverforge:end constraint-modules

mod assemble {
    use super::*;

    pub fn create_constraints() -> impl ConstraintSet<Plan, HardSoftScore> {
        // @solverforge:begin constraint-calls
        (
            (
                one::constraint(),
                two::constraint(),
            ),
            three::constraint(),
        )
        // @solverforge:end constraint-calls
    }
}
"#;

    let result = remove_constraint_from_source(src, "two").expect("remove should succeed");
    assert!(!result.contains("mod two;"));
    assert!(!result.contains("two::constraint(),"));
    assert!(result.contains("one::constraint(),"));
    assert!(result.contains("three::constraint(),"));
}

#[test]
fn test_rewrite_mod_requires_every_declared_constraint_to_be_wired() {
    let src = r#"pub use self::assemble::create_constraints;

// @solverforge:begin constraint-modules
mod all_assigned;
mod max_hours;
// @solverforge:end constraint-modules

mod assemble {
    use super::*;

    pub fn create_constraints() -> impl ConstraintSet<Plan, HardSoftScore> {
        // @solverforge:begin constraint-calls
        (
            all_assigned::constraint(),
        )
        // @solverforge:end constraint-calls
    }
}
"#;

    let err = rewrite_mod(src, "capacity").expect_err("missing constraint wiring should fail");

    assert_eq!(
        err,
        "managed constraint block declares module 'max_hours' but 'constraint-calls' does not invoke it"
    );
}

#[test]
fn test_validate_constraint_mod_source_rejects_undeclared_call() {
    let src = r#"pub use self::assemble::create_constraints;

// @solverforge:begin constraint-modules
mod all_assigned;
// @solverforge:end constraint-modules

mod assemble {
    use super::*;

    pub fn create_constraints() -> impl ConstraintSet<Plan, HardSoftScore> {
        // @solverforge:begin constraint-calls
        (
            all_assigned::constraint(),
            extra::constraint(),
        )
        // @solverforge:end constraint-calls
    }
}
"#;

    let err =
        validate_constraint_mod_source(src).expect_err("undeclared constraint call should fail");

    assert_eq!(
        err,
        "managed constraint block invokes undeclared module 'extra' in 'constraint-calls'"
    );
}

#[test]
fn test_run_preflights_managed_constraint_mod_before_creating_file() {
    let _cwd_guard = test_support::lock_cwd();
    let tmp = tempfile::tempdir().expect("failed to create temp dir");
    let _dir_guard = CwdGuard::enter(tmp.path());
    write_minimal_domain_project();
    fs::create_dir_all("src/constraints").expect("failed to create constraints dir");
    let invalid_mod = r#"pub use self::assemble::create_constraints;

mod assemble {
    use super::*;

    pub fn create_constraints() -> impl ConstraintSet<Plan, HardSoftScore> {
        ()
    }
}
"#;
    fs::write("src/constraints/mod.rs", invalid_mod).expect("failed to write constraints mod");

    let err = run_generate_constraint(
        "capacity", false, true, false, false, false, false, false, false, false, false, false,
        false, false,
    )
    .expect_err("invalid managed block markers should fail before file creation");

    assert!(
        err.to_string()
            .contains("missing or duplicated managed block markers for 'constraint-modules'"),
        "unexpected error: {err}"
    );
    assert!(
        !Path::new("src/constraints/capacity.rs").exists(),
        "failed preflight must not create an orphan constraint file"
    );
    assert_eq!(
        fs::read_to_string("src/constraints/mod.rs").expect("failed to read constraints mod"),
        invalid_mod
    );
}

#[test]
fn test_run_rejects_hard_constraint_for_soft_score() {
    let _cwd_guard = test_support::lock_cwd();
    let tmp = tempfile::tempdir().expect("failed to create temp dir");
    let _dir_guard = CwdGuard::enter(tmp.path());
    write_minimal_domain_project();
    rewrite_plan_score("SoftScore");
    write_minimal_constraints_mod("SoftScore");

    let err = run_generate_constraint(
        "capacity", false, true, false, false, false, false, false, false, false, false, false,
        false, false,
    )
    .expect_err("hard generated constraints should be rejected for SoftScore");

    assert!(
        err.to_string()
            .contains("SoftScore supports only soft generated constraints"),
        "unexpected error: {err}"
    );
    assert!(
        !Path::new("src/constraints/capacity.rs").exists(),
        "rejected hard SoftScore constraint must not create a file"
    );
}

#[test]
fn test_run_accepts_soft_constraint_for_soft_score() {
    let _cwd_guard = test_support::lock_cwd();
    let tmp = tempfile::tempdir().expect("failed to create temp dir");
    let _dir_guard = CwdGuard::enter(tmp.path());
    write_minimal_domain_project();
    rewrite_plan_score("SoftScore");
    write_minimal_constraints_mod("SoftScore");

    run_generate_constraint(
        "preference",
        true,
        true,
        false,
        false,
        false,
        false,
        false,
        false,
        false,
        false,
        false,
        false,
        true,
    )
    .expect("soft generated constraints should be accepted for SoftScore");
}

fn write_minimal_domain_project() {
    fs::create_dir_all("src/domain").expect("failed to create domain dir");
    fs::write(
        "src/domain/mod.rs",
        r#"solverforge::planning_model! {
    root = "src/domain";

    // @solverforge:begin domain-exports
mod task;
mod plan;

pub use task::Task;
pub use plan::Plan;
// @solverforge:end domain-exports
}
"#,
    )
    .expect("failed to write domain mod");
    fs::write(
        "src/domain/task.rs",
        r#"use serde::{Deserialize, Serialize};
use solverforge::prelude::*;

#[planning_entity]
#[derive(Clone, Serialize, Deserialize)]
pub struct Task {
    #[planning_id]
    pub id: String,
}
"#,
    )
    .expect("failed to write task");
    fs::write(
        "src/domain/plan.rs",
        r#"use serde::{Deserialize, Serialize};
use solverforge::prelude::*;

use super::Task;

#[planning_solution(
    constraints = "crate::constraints::create_constraints",
    solver_toml = "../../solver.toml"
)]
#[derive(Serialize, Deserialize)]
pub struct Plan {
    // @solverforge:begin solution-collections
    #[planning_entity_collection]
    pub tasks: Vec<Task>,
    // @solverforge:end solution-collections
    #[planning_score]
    pub score: Option<HardSoftScore>,
}

impl Plan {
    pub fn new(
        // @solverforge:begin solution-constructor-params
        tasks: Vec<Task>,
        // @solverforge:end solution-constructor-params
    ) -> Self {
        Self {
            // @solverforge:begin solution-constructor-init
            tasks,
            // @solverforge:end solution-constructor-init
            score: None,
        }
    }
}
"#,
    )
    .expect("failed to write plan");
}

fn rewrite_plan_score(score_type: &str) {
    let path = Path::new("src/domain/plan.rs");
    let source = fs::read_to_string(path).expect("failed to read plan");
    fs::write(path, source.replace("HardSoftScore", score_type)).expect("failed to rewrite plan");
}

fn write_minimal_constraints_mod(score_type: &str) {
    fs::create_dir_all("src/constraints").expect("failed to create constraints dir");
    fs::write(
        "src/constraints/mod.rs",
        format!(
            r#"pub use self::assemble::create_constraints;

// @solverforge:begin constraint-modules
// @solverforge:end constraint-modules

mod assemble {{
    use solverforge::prelude::*;
    use crate::domain::Plan;

    pub fn create_constraints() -> impl ConstraintSet<Plan, {score_type}> {{
        // @solverforge:begin constraint-calls
        ()
        // @solverforge:end constraint-calls
    }}
}}
"#
        ),
    )
    .expect("failed to write constraints mod");
}

#[test]
fn test_generate_skeleton_unary_hard() {
    let domain = DomainModel {
        solution_type: "EmployeeSchedule".to_string(),
        score_type: "HardSoftDecimalScore".to_string(),
        scalar_groups_path: None,
        conflict_repairs_path: None,
        entities: vec![EntityInfo {
            field_name: "shifts".to_string(),
            item_type: "Shift".to_string(),
            scalar_vars: vec![ScalarVarInfo {
                field: "employee_idx".to_string(),
                value_range_provider: "employees".to_string(),
                countable_range: None,
                allows_unassigned: true,
                hooks: ScalarVariableHooks::default(),
            }],
            list_vars: vec![],
        }],
        facts: vec![],
    };
    let result = generate_skeleton(
        "no_overlap",
        Pattern::Unary,
        false,
        "EmployeeSchedule",
        "HardSoftDecimalScore",
        "no_overlap",
        Some(&domain),
    );
    assert!(result.contains(".for_each(entity_items)"));
    assert!(result.contains("<HardSoftDecimalScore as Score>::one_hard()"));
    assert!(result.contains("HARD:"));
    assert!(result.contains(".penalize(hard_weight(unary_weight))"));
    assert!(result.contains("fn unary_weight"));
    assert!(result.contains(".named(\"no_overlap\")"));
    assert!(!result.contains(".filter("));
    assert!(!result.contains("todo!"));
    assert!(result
        .contains("panic!(\"replace placeholder condition before enabling this constraint\")"));
}

#[test]
fn test_generate_skeleton_pair_hard() {
    let domain = DomainModel {
        solution_type: "EmployeeSchedule".to_string(),
        score_type: "HardSoftDecimalScore".to_string(),
        scalar_groups_path: None,
        conflict_repairs_path: None,
        entities: vec![EntityInfo {
            field_name: "shifts".to_string(),
            item_type: "Shift".to_string(),
            scalar_vars: vec![ScalarVarInfo {
                field: "employee_idx".to_string(),
                value_range_provider: "employees".to_string(),
                countable_range: None,
                allows_unassigned: true,
                hooks: ScalarVariableHooks::default(),
            }],
            list_vars: vec![],
        }],
        facts: vec![],
    };
    let result = generate_skeleton(
        "no_overlap",
        Pattern::Pair,
        false,
        "EmployeeSchedule",
        "HardSoftDecimalScore",
        "no_overlap",
        Some(&domain),
    );
    assert!(result.contains(".for_each(entity_items)"));
    assert!(result.contains(".join(joiner::equal(employee_idx_join_key))"));
    assert!(result.contains(".penalize(hard_weight(pair_weight))"));
    assert!(result.contains(".named(\"no_overlap\")"));
    assert!(!result.contains(".filter("));
    assert!(result.contains(
        "panic!(\"replace placeholder pair condition before enabling this constraint\")"
    ));
    assert!(!result.contains("todo!"));
}

#[test]
fn test_generate_skeleton_join_hard() {
    let domain = DomainModel {
        solution_type: "EmployeeSchedule".to_string(),
        score_type: "HardSoftDecimalScore".to_string(),
        scalar_groups_path: None,
        conflict_repairs_path: None,
        entities: vec![EntityInfo {
            field_name: "shifts".to_string(),
            item_type: "Shift".to_string(),
            scalar_vars: vec![ScalarVarInfo {
                field: "employee_idx".to_string(),
                value_range_provider: "employees".to_string(),
                countable_range: None,
                allows_unassigned: true,
                hooks: ScalarVariableHooks::default(),
            }],
            list_vars: vec![],
        }],
        facts: vec![FactInfo {
            field_name: "employees".to_string(),
            item_type: "Employee".to_string(),
        }],
    };
    let result = generate_skeleton(
        "required_skill",
        Pattern::Join,
        false,
        "EmployeeSchedule",
        "HardSoftDecimalScore",
        "required_skill",
        Some(&domain),
    );
    assert!(result.contains("equal_bi"));
    assert!(result.contains("employees.as_slice()"));
    assert!(result.contains("Employee"));
    assert!(result.contains("entity_join_key"));
    assert!(result.contains("fact_join_key"));
    assert!(result.contains(".penalize(hard_weight(join_weight))"));
    assert!(result.contains(".named(\"required_skill\")"));
    assert!(!result.contains(".filter("));
    assert!(
        result.contains("replace placeholder join key extractor before enabling this constraint")
    );
    assert!(result.contains("replace placeholder join condition before enabling this constraint"));
    assert!(!result.contains("todo!"));
}

#[test]
fn test_generate_skeleton_balance_soft() {
    let domain = DomainModel {
        solution_type: "EmployeeSchedule".to_string(),
        score_type: "HardSoftDecimalScore".to_string(),
        scalar_groups_path: None,
        conflict_repairs_path: None,
        entities: vec![EntityInfo {
            field_name: "shifts".to_string(),
            item_type: "Shift".to_string(),
            scalar_vars: vec![ScalarVarInfo {
                field: "employee_idx".to_string(),
                value_range_provider: "employees".to_string(),
                countable_range: None,
                allows_unassigned: true,
                hooks: ScalarVariableHooks::default(),
            }],
            list_vars: vec![],
        }],
        facts: vec![],
    };
    let result = generate_skeleton(
        "balance",
        Pattern::Balance,
        true,
        "EmployeeSchedule",
        "HardSoftDecimalScore",
        "balance",
        Some(&domain),
    );
    assert!(result.contains("load_balance(balance_group_key, balance_metric)"));
    assert!(result.contains(".penalize(balance_weight)"));
    assert!(result.contains(".named(\"balance\")"));
    assert!(!result.contains(".filter("));
    assert!(result.contains("SOFT:"));
}

#[test]
fn test_generate_skeleton_reward_soft_is_compile_safe() {
    let domain = DomainModel {
        solution_type: "EmployeeSchedule".to_string(),
        score_type: "HardSoftDecimalScore".to_string(),
        scalar_groups_path: None,
        conflict_repairs_path: None,
        entities: vec![EntityInfo {
            field_name: "shifts".to_string(),
            item_type: "Shift".to_string(),
            scalar_vars: vec![ScalarVarInfo {
                field: "employee_idx".to_string(),
                value_range_provider: "employees".to_string(),
                countable_range: None,
                allows_unassigned: true,
                hooks: ScalarVariableHooks::default(),
            }],
            list_vars: vec![],
        }],
        facts: vec![],
    };
    let result = generate_skeleton(
        "preferred_assignment",
        Pattern::Reward,
        true,
        "EmployeeSchedule",
        "HardSoftDecimalScore",
        "preferred_assignment",
        Some(&domain),
    );
    assert!(result.contains(".reward(reward_weight)"));
    assert!(result.contains(".named(\"preferred_assignment\")"));
    assert!(result.contains("fn reward_weight"));
    assert!(!result.contains(".filter("));
    assert!(result.contains(
        "panic!(\"replace placeholder reward condition before enabling this constraint\")"
    ));
    assert!(!result.contains("todo!"));
}

#[test]
fn test_grouped_skeletons_use_weight_closures_instead_of_post_group_filters() {
    let domain = grouped_skeleton_domain();

    for (pattern, expected_weight, extra_expected) in [
        (
            Pattern::Runs,
            ".penalize(hard_weight(runs_weight))",
            "fn runs_weight",
        ),
        (
            Pattern::IndexedPresence,
            ".penalize(hard_weight(indexed_presence_weight))",
            "fn indexed_presence_weight",
        ),
        (
            Pattern::CollectVec,
            ".penalize(hard_weight(collected_values_weight))",
            "fn collected_values_weight",
        ),
        (
            Pattern::GroupComplement,
            ".penalize(hard_weight(group_complement_weight))",
            ".complement(",
        ),
        (
            Pattern::ProjectedGroup,
            ".penalize(hard_weight(projected_group_weight))",
            ".project(projected_group_entry)",
        ),
    ] {
        let result = generate_skeleton(
            "advanced",
            pattern,
            false,
            "EmployeeSchedule",
            "HardSoftScore",
            "advanced",
            Some(&domain),
        );

        assert!(
            result.contains(expected_weight),
            "{pattern:?} should score through a grouped weight helper: {result}"
        );
        assert!(
            result.contains(extra_expected),
            "{pattern:?} should contain expected grouped API shape: {result}"
        );
        assert!(
            !result.contains(".filter(|_value_idx")
                && !result.contains(".filter(|_key")
                && !result.contains(".filter(|_e:"),
            "{pattern:?} should not emit unsupported post-group filters: {result}"
        );
        assert!(result.contains("<HardSoftScore as Score>::zero()"));
        assert!(result.contains(".named(\"advanced\")"));
        syn::parse_file(&result).expect("generated skeleton should parse as Rust");
    }
}

fn grouped_skeleton_domain() -> DomainModel {
    DomainModel {
        solution_type: "EmployeeSchedule".to_string(),
        score_type: "HardSoftScore".to_string(),
        scalar_groups_path: None,
        conflict_repairs_path: None,
        entities: vec![EntityInfo {
            field_name: "shifts".to_string(),
            item_type: "Shift".to_string(),
            scalar_vars: vec![ScalarVarInfo {
                field: "employee_idx".to_string(),
                value_range_provider: "employees".to_string(),
                countable_range: None,
                allows_unassigned: true,
                hooks: ScalarVariableHooks::default(),
            }],
            list_vars: vec![],
        }],
        facts: vec![FactInfo {
            field_name: "employees".to_string(),
            item_type: "Employee".to_string(),
        }],
    }
}
