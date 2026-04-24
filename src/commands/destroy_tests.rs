use super::{run_constraint, run_entity, run_fact, run_solution};
use crate::{commands::generate_constraint::remove_constraint_from_source, test_support};
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
fn removes_constraint_from_managed_tuple_entry() {
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
            all_assigned::constraint(),
            extra::constraint(),
        )
        // @solverforge:end constraint-calls
    }
}
"#;

    let updated =
        remove_constraint_from_source(src, "all_assigned").expect("source should be rewritten");
    assert!(!updated.contains("mod all_assigned;"));
    assert!(!updated.contains("all_assigned::constraint(),"));
    assert!(updated.contains("extra::constraint(),"));
}

#[test]
fn removes_last_constraint_to_empty_tuple() {
    let src = r#"pub use self::assemble::create_constraints;

// @solverforge:begin constraint-modules
mod extra;
// @solverforge:end constraint-modules

mod assemble {
    use super::*;

    pub fn create_constraints() -> impl ConstraintSet<Plan, HardSoftScore> {
        // @solverforge:begin constraint-calls
        (
            extra::constraint(),
        )
        // @solverforge:end constraint-calls
    }
}
"#;

    let updated = remove_constraint_from_source(src, "extra").expect("source should be rewritten");
    assert!(!updated.contains("mod extra;"));
    assert!(updated.contains("        ()"));
}

#[test]
fn destroy_solution_preflights_domain_mod_before_delete() {
    let _cwd_guard = test_support::lock_cwd();
    let tmp = tempfile::tempdir().expect("failed to create temp dir");
    let _dir_guard = CwdGuard::enter(tmp.path());
    write_minimal_domain_project(false);
    let plan_before = fs::read_to_string("src/domain/plan.rs").expect("failed to read plan");
    let mod_before = fs::read_to_string("src/domain/mod.rs").expect("failed to read domain mod");

    let err =
        run_solution(true).expect_err("invalid domain managed markers should fail before delete");

    assert!(
        err.to_string()
            .contains("src/domain/mod.rs must declare solverforge::planning_model! { ... }"),
        "unexpected error: {err}"
    );
    assert!(
        Path::new("src/domain/plan.rs").exists(),
        "failed preflight must not delete the solution file"
    );
    assert_eq!(
        fs::read_to_string("src/domain/plan.rs").expect("failed to read plan"),
        plan_before
    );
    assert_eq!(
        fs::read_to_string("src/domain/mod.rs").expect("failed to read domain mod"),
        mod_before
    );
}

#[test]
fn destroy_entity_preflights_domain_mod_before_delete() {
    let _cwd_guard = test_support::lock_cwd();
    let tmp = tempfile::tempdir().expect("failed to create temp dir");
    let _dir_guard = CwdGuard::enter(tmp.path());
    write_minimal_domain_project(false);
    let task_before = fs::read_to_string("src/domain/task.rs").expect("failed to read task");
    let plan_before = fs::read_to_string("src/domain/plan.rs").expect("failed to read plan");
    let mod_before = fs::read_to_string("src/domain/mod.rs").expect("failed to read domain mod");

    let err = run_entity("task", true)
        .expect_err("invalid domain managed markers should fail before delete");

    assert!(
        err.to_string()
            .contains("src/domain/mod.rs must declare solverforge::planning_model! { ... }"),
        "unexpected error: {err}"
    );
    assert!(
        Path::new("src/domain/task.rs").exists(),
        "failed preflight must not delete the entity file"
    );
    assert_eq!(
        fs::read_to_string("src/domain/task.rs").expect("failed to read task"),
        task_before
    );
    assert_eq!(
        fs::read_to_string("src/domain/plan.rs").expect("failed to read plan"),
        plan_before
    );
    assert_eq!(
        fs::read_to_string("src/domain/mod.rs").expect("failed to read domain mod"),
        mod_before
    );
}

#[test]
fn destroy_fact_preflights_domain_mod_before_delete() {
    let _cwd_guard = test_support::lock_cwd();
    let tmp = tempfile::tempdir().expect("failed to create temp dir");
    let _dir_guard = CwdGuard::enter(tmp.path());
    write_minimal_domain_project(false);
    let resource_before =
        fs::read_to_string("src/domain/resource.rs").expect("failed to read resource");
    let plan_before = fs::read_to_string("src/domain/plan.rs").expect("failed to read plan");
    let mod_before = fs::read_to_string("src/domain/mod.rs").expect("failed to read domain mod");

    let err = run_fact("resource", true)
        .expect_err("invalid domain managed markers should fail before delete");

    assert!(
        err.to_string()
            .contains("src/domain/mod.rs must declare solverforge::planning_model! { ... }"),
        "unexpected error: {err}"
    );
    assert!(
        Path::new("src/domain/resource.rs").exists(),
        "failed preflight must not delete the fact file"
    );
    assert_eq!(
        fs::read_to_string("src/domain/resource.rs").expect("failed to read resource"),
        resource_before
    );
    assert_eq!(
        fs::read_to_string("src/domain/plan.rs").expect("failed to read plan"),
        plan_before
    );
    assert_eq!(
        fs::read_to_string("src/domain/mod.rs").expect("failed to read domain mod"),
        mod_before
    );
}

#[test]
fn destroy_fact_rejects_still_referenced_fact_collection() {
    let _cwd_guard = test_support::lock_cwd();
    let tmp = tempfile::tempdir().expect("failed to create temp dir");
    let _dir_guard = CwdGuard::enter(tmp.path());
    write_minimal_domain_project(true);
    fs::write(
        "src/domain/task.rs",
        r#"use serde::{Deserialize, Serialize};
use solverforge::prelude::*;

#[planning_entity]
#[derive(Clone, Serialize, Deserialize)]
pub struct Task {
    #[planning_id]
    pub id: String,
    // @solverforge:begin entity-variables
    #[planning_variable(value_range = "resources", allows_unassigned = true)]
    pub resource_idx: Option<usize>,
    // @solverforge:end entity-variables
}

impl Task {
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            // @solverforge:begin entity-variable-init
            resource_idx: None,
            // @solverforge:end entity-variable-init
        }
    }
}
"#,
    )
    .expect("failed to rewrite task");
    let resource_before =
        fs::read_to_string("src/domain/resource.rs").expect("failed to read resource");
    let plan_before = fs::read_to_string("src/domain/plan.rs").expect("failed to read plan");
    let mod_before = fs::read_to_string("src/domain/mod.rs").expect("failed to read domain mod");

    let err = run_fact("resource", true)
        .expect_err("destroy fact should fail while variables still reference the collection");

    assert!(
        err.to_string().contains(
            "cannot destroy fact 'Resource' because managed planning variables still reference collection 'resources': Task.resource_idx (value_range = \"resources\")"
        ),
        "unexpected error: {err}"
    );
    assert!(
        Path::new("src/domain/resource.rs").exists(),
        "failed dependency check must not delete the fact file"
    );
    assert_eq!(
        fs::read_to_string("src/domain/resource.rs").expect("failed to read resource"),
        resource_before
    );
    assert_eq!(
        fs::read_to_string("src/domain/plan.rs").expect("failed to read plan"),
        plan_before
    );
    assert_eq!(
        fs::read_to_string("src/domain/mod.rs").expect("failed to read domain mod"),
        mod_before
    );
}

#[test]
fn destroy_entity_preflights_solution_rewrite_before_domain_update_or_delete() {
    let _cwd_guard = test_support::lock_cwd();
    let tmp = tempfile::tempdir().expect("failed to create temp dir");
    let _dir_guard = CwdGuard::enter(tmp.path());
    write_minimal_domain_project(true);
    let plan_without_collection_markers = fs::read_to_string("src/domain/plan.rs")
        .expect("failed to read plan")
        .replace("    // @solverforge:begin solution-collections\n", "")
        .replace("    // @solverforge:end solution-collections\n", "");
    fs::write("src/domain/plan.rs", &plan_without_collection_markers)
        .expect("failed to rewrite plan");
    let task_before = fs::read_to_string("src/domain/task.rs").expect("failed to read task");
    let mod_before = fs::read_to_string("src/domain/mod.rs").expect("failed to read domain mod");

    let err = run_entity("task", true)
        .expect_err("invalid solution managed markers should fail before mutation");

    assert!(
        err.to_string()
            .contains("missing or duplicated managed block markers for 'solution-collections'"),
        "unexpected error: {err}"
    );
    assert!(
        Path::new("src/domain/task.rs").exists(),
        "failed solution preflight must not delete the entity file"
    );
    assert_eq!(
        fs::read_to_string("src/domain/task.rs").expect("failed to read task"),
        task_before
    );
    assert_eq!(
        fs::read_to_string("src/domain/mod.rs").expect("failed to read domain mod"),
        mod_before
    );
    assert_eq!(
        fs::read_to_string("src/domain/plan.rs").expect("failed to read plan"),
        plan_without_collection_markers
    );
}

#[test]
fn destroy_constraint_preflights_constraint_mod_before_delete() {
    let _cwd_guard = test_support::lock_cwd();
    let tmp = tempfile::tempdir().expect("failed to create temp dir");
    let _dir_guard = CwdGuard::enter(tmp.path());
    fs::create_dir_all("src/constraints").expect("failed to create constraints dir");
    let invalid_mod = r#"pub use self::assemble::create_constraints;

mod all_assigned;

mod assemble {
    use super::*;

    pub fn create_constraints() -> impl ConstraintSet<Plan, HardSoftScore> {
        all_assigned::constraint()
    }
}
"#;
    fs::write("src/constraints/mod.rs", invalid_mod).expect("failed to write constraints mod");
    fs::write(
        "src/constraints/all_assigned.rs",
        "pub fn constraint() {}\n",
    )
    .expect("failed to write constraint file");

    let err = run_constraint("all_assigned", true)
        .expect_err("invalid constraint managed markers should fail before delete");

    assert!(
        err.to_string()
            .contains("missing or duplicated managed block markers for 'constraint-modules'"),
        "unexpected error: {err}"
    );
    assert!(
        Path::new("src/constraints/all_assigned.rs").exists(),
        "failed preflight must not delete the constraint file"
    );
    assert_eq!(
        fs::read_to_string("src/constraints/mod.rs").expect("failed to read constraints mod"),
        invalid_mod
    );
}

fn write_minimal_domain_project(managed_domain_mod: bool) {
    fs::create_dir_all("src/domain").expect("failed to create domain dir");
    let domain_mod = if managed_domain_mod {
        r#"solverforge::planning_model! {
    root = "src/domain";

    // @solverforge:begin domain-exports
mod resource;
mod task;
mod plan;

pub use resource::Resource;
pub use task::Task;
pub use plan::Plan;
// @solverforge:end domain-exports
}
"#
    } else {
        r#"mod resource;
mod task;
mod plan;

pub use resource::Resource;
pub use task::Task;
pub use plan::Plan;
"#
    };
    fs::write("src/domain/mod.rs", domain_mod).expect("failed to write domain mod");
    fs::write(
        "src/domain/resource.rs",
        r#"use serde::{Deserialize, Serialize};
use solverforge::prelude::*;

#[problem_fact]
#[derive(Clone, Serialize, Deserialize)]
pub struct Resource {
    #[planning_id]
    pub id: String,
}
"#,
    )
    .expect("failed to write resource");
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

// @solverforge:begin solution-imports
use super::Resource;
use super::Task;
// @solverforge:end solution-imports

#[planning_solution(
    constraints = "crate::constraints::create_constraints",
    solver_toml = "../../solver.toml"
)]
#[derive(Serialize, Deserialize)]
pub struct Plan {
    // @solverforge:begin solution-collections
    #[problem_fact_collection]
    pub resources: Vec<Resource>,
    #[planning_entity_collection]
    pub tasks: Vec<Task>,
    // @solverforge:end solution-collections
    #[planning_score]
    pub score: Option<HardSoftScore>,
}

impl Plan {
    pub fn new(
        // @solverforge:begin solution-constructor-params
        resources: Vec<Resource>,
        tasks: Vec<Task>,
        // @solverforge:end solution-constructor-params
    ) -> Self {
        Self {
            // @solverforge:begin solution-constructor-init
            resources,
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
