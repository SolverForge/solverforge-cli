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
    solver_toml = "../../solver.toml",
    scalar_groups = "scalar_groups",
    conflict_repairs = "conflict_repairs"
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

#[test]
fn check_rejects_stale_scalar_group_variable_targets() {
    let _cwd_guard = test_support::lock_cwd();
    let tmp = tempfile::tempdir().expect("failed to create temp dir");
    let _dir_guard = CwdGuard::enter(tmp.path());
    write_project_layout();
    fs::write(
        "solverforge.app.toml",
        r#"
[[scalar_groups]]
name = "stale_assignment"
kind = "assignment"
solver_config = false
enabled = true

[[scalar_groups.targets]]
entity = "task"
entity_plural = "tasks"
field = "missing_idx"
"#,
    )
    .expect("failed to write app spec");

    let err = run().expect_err("stale scalar group target should fail check");
    let err = err.to_string();

    assert!(
        err.contains("1 error(s), 0 warning(s)"),
        "unexpected summary: {err}"
    );
}

#[test]
fn check_skips_disabled_scalar_group_variable_targets() {
    let _cwd_guard = test_support::lock_cwd();
    let tmp = tempfile::tempdir().expect("failed to create temp dir");
    let _dir_guard = CwdGuard::enter(tmp.path());
    write_project_layout();
    fs::write(
        "solverforge.app.toml",
        r#"
[[scalar_groups]]
name = "disabled_assignment"
kind = "assignment"
solver_config = false
enabled = false

[[scalar_groups.targets]]
entity = "task"
entity_plural = "tasks"
field = "missing_idx"
"#,
    )
    .expect("failed to write app spec");

    run().expect("disabled scalar group targets should be inert");
}

#[test]
fn check_rejects_assignment_rule_without_sequence_key() {
    let _cwd_guard = test_support::lock_cwd();
    let tmp = tempfile::tempdir().expect("failed to create temp dir");
    let _dir_guard = CwdGuard::enter(tmp.path());
    write_project_layout();
    fs::write(
        "solverforge.app.toml",
        r#"
[[scalar_groups]]
name = "ordered_assignment"
kind = "assignment"
solver_config = false
enabled = true

[scalar_groups.assignment_hooks]
assignment_rule = "compatible_pair"

[[scalar_groups.targets]]
entity = "task"
entity_plural = "tasks"
field = "resource_idx"
"#,
    )
    .expect("failed to write app spec");

    let err = run().expect_err("invalid scalar group hooks should fail check");
    let err = err.to_string();

    assert!(
        err.contains("1 error(s), 0 warning(s)"),
        "unexpected summary: {err}"
    );
}

#[test]
fn check_rejects_enabled_scalar_group_without_solution_path() {
    let _cwd_guard = test_support::lock_cwd();
    let tmp = tempfile::tempdir().expect("failed to create temp dir");
    let _dir_guard = CwdGuard::enter(tmp.path());
    write_project_layout();
    let plan = fs::read_to_string("src/domain/plan.rs").expect("failed to read plan");
    fs::write(
        "src/domain/plan.rs",
        plan.replace("    scalar_groups = \"scalar_groups\",\n", ""),
    )
    .expect("failed to remove scalar group solution path");
    fs::write(
        "solverforge.app.toml",
        r#"
[[scalar_groups]]
name = "required_assignment"
kind = "assignment"
solver_config = false
enabled = true

[[scalar_groups.targets]]
entity = "task"
entity_plural = "tasks"
field = "resource_idx"
"#,
    )
    .expect("failed to write app spec");

    let err = run().expect_err("missing scalar group solution path should fail check");
    let err = err.to_string();

    assert!(
        err.contains("1 error(s), 0 warning(s)"),
        "unexpected summary: {err}"
    );
}

#[test]
fn check_rejects_stale_grouped_scalar_solver_config() {
    let _cwd_guard = test_support::lock_cwd();
    let tmp = tempfile::tempdir().expect("failed to create temp dir");
    let _dir_guard = CwdGuard::enter(tmp.path());
    write_project_layout();
    fs::write(
        "solver.toml",
        r#"
[[phases]]
type = "local_search"

[phases.move_selector]
type = "grouped_scalar_move_selector"
group_name = "missing_group"
"#,
    )
    .expect("failed to write solver config");

    let err = run().expect_err("stale grouped scalar config should fail check");
    let err = err.to_string();

    assert!(
        err.contains("1 error(s), 0 warning(s)"),
        "unexpected summary: {err}"
    );
}

#[test]
fn check_rejects_missing_candidate_scalar_group_construction_config() {
    let _cwd_guard = test_support::lock_cwd();
    let tmp = tempfile::tempdir().expect("failed to create temp dir");
    let _dir_guard = CwdGuard::enter(tmp.path());
    write_project_layout();
    fs::write(
        "solverforge.app.toml",
        r#"
[[scalar_groups]]
name = "candidate_group"
kind = "candidates"
candidate_provider = "candidate_provider"
solver_config = true
enabled = true

[[scalar_groups.targets]]
entity = "task"
entity_plural = "tasks"
field = "resource_idx"
"#,
    )
    .expect("failed to write app spec");
    fs::write(
        "solver.toml",
        r#"
# @solverforge:begin solver-config
# @solverforge:owner scalar-group candidate_group search
[[phases]]
type = "local_search"

[phases.move_selector]
type = "grouped_scalar_move_selector"
group_name = "candidate_group"
require_hard_improvement = true
# @solverforge:end solver-config
"#,
    )
    .expect("failed to write solver config");

    let err = run().expect_err("missing candidate construction config should fail check");
    let err = err.to_string();

    assert!(
        err.contains("1 error(s), 0 warning(s)"),
        "unexpected summary: {err}"
    );
}

#[test]
fn check_rejects_stale_grouped_scalar_neighborhood_solver_config() {
    let _cwd_guard = test_support::lock_cwd();
    let tmp = tempfile::tempdir().expect("failed to create temp dir");
    let _dir_guard = CwdGuard::enter(tmp.path());
    write_project_layout();
    fs::write(
        "solver.toml",
        r#"
[[phases]]
type = "local_search"
local_search_type = "variable_neighborhood_descent"

[[phases.neighborhoods]]
type = "grouped_scalar_move_selector"
group_name = "missing_group"
"#,
    )
    .expect("failed to write solver config");

    let err = run().expect_err("stale neighborhood scalar group should fail check");
    let err = err.to_string();

    assert!(
        err.contains("1 error(s), 0 warning(s)"),
        "unexpected summary: {err}"
    );
}

#[test]
fn check_rejects_stale_conflict_repair_neighborhood_solver_config() {
    let _cwd_guard = test_support::lock_cwd();
    let tmp = tempfile::tempdir().expect("failed to create temp dir");
    let _dir_guard = CwdGuard::enter(tmp.path());
    write_project_layout();
    fs::write(
        "solver.toml",
        r#"
[[phases]]
type = "local_search"
local_search_type = "variable_neighborhood_descent"

[[phases.neighborhoods]]
type = "compound_conflict_repair_move_selector"
constraints = ["missing_constraint"]
"#,
    )
    .expect("failed to write solver config");

    let err = run().expect_err("stale neighborhood conflict repair should fail check");
    let err = err.to_string();

    assert!(
        err.contains("1 error(s), 0 warning(s)"),
        "unexpected summary: {err}"
    );
}

#[test]
fn check_rejects_stale_nested_selector_solver_config() {
    let _cwd_guard = test_support::lock_cwd();
    let tmp = tempfile::tempdir().expect("failed to create temp dir");
    let _dir_guard = CwdGuard::enter(tmp.path());
    write_project_layout();
    fs::write(
        "solver.toml",
        r#"
[[phases]]
type = "local_search"

[phases.move_selector]
type = "union_move_selector"

[[phases.move_selector.selectors]]
type = "limited_neighborhood"
selected_count_limit = 8

[phases.move_selector.selectors.selector]
type = "grouped_scalar_move_selector"
group_name = "missing_group"
"#,
    )
    .expect("failed to write solver config");

    let err = run().expect_err("stale nested selector ref should fail check");
    let err = err.to_string();

    assert!(
        err.contains("1 error(s), 0 warning(s)"),
        "unexpected summary: {err}"
    );
}

#[test]
fn check_rejects_mismatched_solver_config_managed_blocks() {
    let _cwd_guard = test_support::lock_cwd();
    let tmp = tempfile::tempdir().expect("failed to create temp dir");
    let _dir_guard = CwdGuard::enter(tmp.path());
    write_project_layout();
    fs::write(
        "solver.toml",
        r#"
# @solverforge:begin solver-config
# @solverforge:owner scalar-group paired construction
[[phases]]
type = "construction_heuristic"
group_name = "paired"
"#,
    )
    .expect("failed to write solver config");

    let err = run().expect_err("mismatched managed solver config block should fail check");
    let err = err.to_string();

    assert!(
        err.contains("error(s), 0 warning(s)"),
        "unexpected summary: {err}"
    );
}

#[test]
fn check_rejects_solver_config_reference_to_disabled_scalar_group() {
    let _cwd_guard = test_support::lock_cwd();
    let tmp = tempfile::tempdir().expect("failed to create temp dir");
    let _dir_guard = CwdGuard::enter(tmp.path());
    write_project_layout();
    fs::write(
        "solverforge.app.toml",
        r#"
[[scalar_groups]]
name = "disabled_assignment"
kind = "assignment"
solver_config = true
enabled = false

[[scalar_groups.targets]]
entity = "task"
entity_plural = "tasks"
field = "resource_idx"
"#,
    )
    .expect("failed to write app spec");
    fs::write(
        "solver.toml",
        r#"
[[phases]]
type = "local_search"

[phases.move_selector]
type = "grouped_scalar_move_selector"
group_name = "disabled_assignment"
"#,
    )
    .expect("failed to write solver config");

    let err = run().expect_err("disabled scalar group solver config should fail check");
    let err = err.to_string();

    assert!(
        err.contains("1 error(s), 0 warning(s)"),
        "unexpected summary: {err}"
    );
}

#[test]
fn check_rejects_stale_conflict_repair_constraint() {
    let _cwd_guard = test_support::lock_cwd();
    let tmp = tempfile::tempdir().expect("failed to create temp dir");
    let _dir_guard = CwdGuard::enter(tmp.path());
    write_project_layout();
    fs::write(
        "solverforge.app.toml",
        r#"
[[conflict_repairs]]
constraint = "Missing Constraint"
provider = "repair_missing"
selector = "compound"
solver_config = false
enabled = true
"#,
    )
    .expect("failed to write app spec");

    let err = run().expect_err("stale conflict repair should fail check");
    let err = err.to_string();

    assert!(
        err.contains("1 error(s), 0 warning(s)"),
        "unexpected summary: {err}"
    );
}

#[test]
fn check_rejects_stale_conflict_repair_solver_config() {
    let _cwd_guard = test_support::lock_cwd();
    let tmp = tempfile::tempdir().expect("failed to create temp dir");
    let _dir_guard = CwdGuard::enter(tmp.path());
    write_project_layout();
    fs::write(
        "solver.toml",
        r#"
[[phases]]
type = "local_search"

[phases.move_selector]
type = "compound_conflict_repair_move_selector"
constraints = ["Missing Constraint"]
"#,
    )
    .expect("failed to write solver config");

    let err = run().expect_err("stale conflict repair config should fail check");
    let err = err.to_string();

    assert!(
        err.contains("1 error(s), 0 warning(s)"),
        "unexpected summary: {err}"
    );
}

#[test]
fn check_rejects_constraint_named_value_that_differs_from_module_id() {
    let _cwd_guard = test_support::lock_cwd();
    let tmp = tempfile::tempdir().expect("failed to create temp dir");
    let _dir_guard = CwdGuard::enter(tmp.path());
    write_project_layout();
    fs::write(
        "src/constraints/unary_placeholder.rs",
        r#"pub fn constraint() {
    something.named("Unary Placeholder");
}
"#,
    )
    .expect("failed to write constraint");
    fs::write(
        "src/constraints/mod.rs",
        r#"use crate::domain::Plan;
use solverforge::prelude::*;

pub use self::assemble::create_constraints;

// @solverforge:begin constraint-modules
mod unary_placeholder;
// @solverforge:end constraint-modules

mod assemble {
    use super::*;

    pub fn create_constraints() -> impl ConstraintSet<Plan, HardSoftScore> {
        // @solverforge:begin constraint-calls
        unary_placeholder::constraint()
        // @solverforge:end constraint-calls
    }
}
"#,
    )
    .expect("failed to write constraints mod");

    let err = run().expect_err("constraint ID drift should fail check");
    let err = err.to_string();

    assert!(
        err.contains("1 error(s), 0 warning(s)"),
        "unexpected summary: {err}"
    );
}

#[test]
fn check_ignores_constraint_helper_files_not_declared_in_managed_mod() {
    let _cwd_guard = test_support::lock_cwd();
    let tmp = tempfile::tempdir().expect("failed to create temp dir");
    let _dir_guard = CwdGuard::enter(tmp.path());
    write_project_layout();
    fs::write(
        "src/constraints/helpers.rs",
        r#"pub fn shared_weight(raw: i32) -> i32 {
    raw.max(0)
}
"#,
    )
    .expect("failed to write helper module");

    run().expect("unmanaged helper files should not be treated as active constraints");
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
    solver_toml = "../../solver.toml",
    scalar_groups = "scalar_groups",
    conflict_repairs = "conflict_repairs"
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
