use super::{
    domain::{DomainModel, EntityInfo, FactInfo, ScalarVarInfo},
    mod_rewriter::{remove_constraint_from_source, rewrite_mod, validate_constraint_mod_source},
    run::run as run_generate_constraint,
    skeleton::{generate_skeleton, Pattern},
    utils::{snake_to_title, validate_name},
};
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
fn test_snake_to_title() {
    assert_eq!(snake_to_title("max_hours"), "Max Hours");
    assert_eq!(snake_to_title("required_skill"), "Required Skill");
    assert_eq!(snake_to_title("all_assigned"), "All Assigned");
    assert_eq!(snake_to_title("capacity"), "Capacity");
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
        "capacity", false, true, false, false, false, false, false, false,
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
        "capacity", false, true, false, false, false, false, false, false,
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
        entities: vec![EntityInfo {
            field_name: "shifts".to_string(),
            item_type: "Shift".to_string(),
            scalar_vars: vec![ScalarVarInfo {
                field: "employee_idx".to_string(),
                value_range: "employees".to_string(),
                allows_unassigned: true,
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
        "No Overlap",
        Some(&domain),
    );
    assert!(result.contains("for_each(|s: &EmployeeSchedule| s.shifts.as_slice())"));
    assert!(result.contains("<HardSoftDecimalScore as Score>::one_hard()"));
    assert!(result.contains("HARD:"));
    assert!(!result.contains("todo!"));
    assert!(result
        .contains("panic!(\"replace placeholder condition before enabling this constraint\")"));
}

#[test]
fn test_generate_skeleton_pair_hard() {
    let domain = DomainModel {
        solution_type: "EmployeeSchedule".to_string(),
        score_type: "HardSoftDecimalScore".to_string(),
        entities: vec![EntityInfo {
            field_name: "shifts".to_string(),
            item_type: "Shift".to_string(),
            scalar_vars: vec![ScalarVarInfo {
                field: "employee_idx".to_string(),
                value_range: "employees".to_string(),
                allows_unassigned: true,
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
        "No Overlap",
        Some(&domain),
    );
    assert!(result.contains("for_each(|s: &EmployeeSchedule| s.shifts.as_slice())"));
    assert!(result.contains("joiner::equal(|e: &Shift| e.employee_idx)"));
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
        entities: vec![EntityInfo {
            field_name: "shifts".to_string(),
            item_type: "Shift".to_string(),
            scalar_vars: vec![ScalarVarInfo {
                field: "employee_idx".to_string(),
                value_range: "employees".to_string(),
                allows_unassigned: true,
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
        "Required Skill",
        Some(&domain),
    );
    assert!(result.contains("equal_bi"));
    assert!(result.contains("employees.as_slice()"));
    assert!(result.contains("Employee"));
    assert!(result.contains("|e: &Shift| e.employee_idx"));
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
        entities: vec![EntityInfo {
            field_name: "shifts".to_string(),
            item_type: "Shift".to_string(),
            scalar_vars: vec![ScalarVarInfo {
                field: "employee_idx".to_string(),
                value_range: "employees".to_string(),
                allows_unassigned: true,
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
        "Balance",
        Some(&domain),
    );
    assert!(result.contains(".balance(|e: &Shift| e.employee_idx)"));
    assert!(result.contains("SOFT:"));
}

#[test]
fn test_generate_skeleton_reward_soft_is_compile_safe() {
    let domain = DomainModel {
        solution_type: "EmployeeSchedule".to_string(),
        score_type: "HardSoftDecimalScore".to_string(),
        entities: vec![EntityInfo {
            field_name: "shifts".to_string(),
            item_type: "Shift".to_string(),
            scalar_vars: vec![ScalarVarInfo {
                field: "employee_idx".to_string(),
                value_range: "employees".to_string(),
                allows_unassigned: true,
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
        "Preferred Assignment",
        Some(&domain),
    );
    assert!(result.contains(".reward(<HardSoftDecimalScore as Score>::one_soft())"));
    assert!(result.contains(
        "panic!(\"replace placeholder reward condition before enabling this constraint\")"
    ));
    assert!(!result.contains("todo!"));
}
