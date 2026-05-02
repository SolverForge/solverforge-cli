use super::run;
use crate::test_support;
use std::fs;
use std::path::{Path, PathBuf};

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
fn check_rejects_malformed_current_managed_files() {
    let _cwd_guard = test_support::lock_cwd();
    let tmp = tempfile::tempdir().expect("failed to create temp dir");
    let _dir_guard = CwdGuard::enter(tmp.path());
    write_project_layout();

    fs::write(
        "src/domain/mod.rs",
        r#"mod task;
mod plan;

pub use task::Task;
pub use plan::Plan;
"#,
    )
    .expect("failed to corrupt domain mod");
    fs::write(
        "src/domain/task.rs",
        r#"use serde::{Deserialize, Serialize};
use solverforge::prelude::*;

#[planning_entity]
#[derive(Serialize, Deserialize)]
pub struct Task {
    #[planning_id]
    pub id: String,
}

impl Task {
    pub fn new(id: impl Into<String>) -> Self {
        Self { id: id.into() }
    }
}
"#,
    )
    .expect("failed to corrupt task");
    fs::write(
        "src/domain/plan.rs",
        r#"use serde::{Deserialize, Serialize};
use solverforge::prelude::*;

// @solverforge:begin solution-imports
use super::Task;
// @solverforge:end solution-imports

#[planning_solution(
    constraints = "crate::constraints::create_constraints",
    solver_toml = "../../solver.toml"
)]
#[derive(Serialize, Deserialize)]
pub struct Plan {
    #[planning_entity_collection]
    pub tasks: Vec<Task>,
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
    .expect("failed to corrupt plan");
    fs::write(
        "src/constraints/mod.rs",
        r#"pub use self::assemble::create_constraints;

mod assemble {
    use super::*;

    pub fn create_constraints() -> impl ConstraintSet<Plan, HardSoftScore> {
        ()
    }
}
"#,
    )
    .expect("failed to corrupt constraints mod");

    let err = run().expect_err("malformed managed files should fail check");
    let err = err.to_string();

    assert!(
        err.contains("2 error(s), 0 warning(s)"),
        "unexpected summary: {err}"
    );
}

#[test]
fn check_rejects_invalid_override_templates() {
    let _cwd_guard = test_support::lock_cwd();
    let tmp = tempfile::tempdir().expect("failed to create temp dir");
    let _dir_guard = CwdGuard::enter(tmp.path());
    write_project_layout();
    fs::create_dir_all(".solverforge/templates").expect("failed to create override dir");
    fs::write(
        ".solverforge/templates/entity.rs.tmpl",
        "pub struct {{NAME}} {\n    pub id: String,\n}\n",
    )
    .expect("failed to write entity override");
    fs::write(
        ".solverforge/templates/solution.rs.tmpl",
        "pub struct {{NAME}} {\n    pub score: Option<{{FIELDS}}>,\n}\n",
    )
    .expect("failed to write solution override");

    let err = run().expect_err("invalid override templates should fail check");
    let err = err.to_string();

    assert!(
        err.contains("2 error(s), 0 warning(s)"),
        "unexpected summary: {err}"
    );
}

#[test]
fn check_rejects_missing_primary_re_exports_and_constraint_tuple_drift() {
    let _cwd_guard = test_support::lock_cwd();
    let tmp = tempfile::tempdir().expect("failed to create temp dir");
    let _dir_guard = CwdGuard::enter(tmp.path());
    write_project_layout();

    fs::write(
        "src/domain/mod.rs",
        r#"solverforge::planning_model! {
    root = "src/domain";

    // @solverforge:begin domain-exports
mod task;
mod plan;

pub use plan::Plan;
// @solverforge:end domain-exports
}
"#,
    )
    .expect("failed to rewrite domain mod");
    fs::write(
        "src/constraints/mod.rs",
        r#"use crate::domain::Plan;
use solverforge::prelude::*;

pub use self::assemble::create_constraints;

// @solverforge:begin constraint-modules
mod coverage_gap;
// @solverforge:end constraint-modules

mod assemble {
    use super::*;

    pub fn create_constraints() -> impl ConstraintSet<Plan, HardSoftScore> {
        // @solverforge:begin constraint-calls
        ()
        // @solverforge:end constraint-calls
    }
}
"#,
    )
    .expect("failed to rewrite constraints mod");
    fs::write(
        "src/constraints/coverage_gap.rs",
        "pub fn constraint() {}\n",
    )
    .expect("failed to write constraint file");

    let err = run().expect_err("semantic managed drift should fail check");
    let err = err.to_string();

    assert!(
        err.contains("2 error(s), 0 warning(s)"),
        "unexpected summary: {err}"
    );
}

#[test]
fn check_rejects_variables_that_reference_missing_fact_collections() {
    let _cwd_guard = test_support::lock_cwd();
    let tmp = tempfile::tempdir().expect("failed to create temp dir");
    let _dir_guard = CwdGuard::enter(tmp.path());
    write_project_layout();

    fs::write(
        "src/domain/task.rs",
        r#"use serde::{Deserialize, Serialize};
use solverforge::prelude::*;

#[planning_entity]
#[derive(Serialize, Deserialize)]
pub struct Task {
    #[planning_id]
    pub id: String,
    // @solverforge:begin entity-variables
    #[planning_variable(value_range_provider = "resources", allows_unassigned = true)]
    pub resource_idx: Option<usize>,
    #[planning_list_variable(element_collection = "item_facts")]
    pub visits: Vec<usize>,
    // @solverforge:end entity-variables
}

impl Task {
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            // @solverforge:begin entity-variable-init
            resource_idx: None,
            visits: Vec::new(),
            // @solverforge:end entity-variable-init
        }
    }
}
"#,
    )
    .expect("failed to rewrite task");

    let err = run().expect_err("missing fact references should fail check");
    let err = err.to_string();

    assert!(
        err.contains("1 error(s), 0 warning(s)"),
        "unexpected summary: {err}"
    );
}

fn write_project_layout() {
    fs::create_dir_all("src/domain").expect("failed to create domain dir");
    fs::create_dir_all("src/constraints").expect("failed to create constraints dir");
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
#[derive(Serialize, Deserialize)]
pub struct Task {
    #[planning_id]
    pub id: String,
    // @solverforge:begin entity-variables
    #[planning_variable(allows_unassigned = true)]
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
    .expect("failed to write task");
    fs::write(
        "src/domain/plan.rs",
        r#"use serde::{Deserialize, Serialize};
use solverforge::prelude::*;

// @solverforge:begin solution-imports
use super::Task;
// @solverforge:end solution-imports

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
    fs::write(
        "src/constraints/mod.rs",
        r#"use crate::domain::Plan;
use solverforge::prelude::*;

pub use self::assemble::create_constraints;

// @solverforge:begin constraint-modules
// @solverforge:end constraint-modules

mod assemble {
    use super::*;

    pub fn create_constraints() -> impl ConstraintSet<Plan, HardSoftScore> {
        // @solverforge:begin constraint-calls
        ()
        // @solverforge:end constraint-calls
    }
}
"#,
    )
    .expect("failed to write constraints mod");
    fs::write("solver.toml", "# test config\n").expect("failed to write solver.toml");
}
