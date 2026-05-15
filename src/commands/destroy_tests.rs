use super::{run_conflict_repair, run_constraint, run_entity, run_fact, run_solution};
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
    #[planning_variable(value_range_provider = "resources", allows_unassigned = true)]
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
            "cannot destroy fact 'Resource' because managed planning variables still reference collection 'resources': Task.resource_idx (value_range_provider = \"resources\")"
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
fn destroy_entity_rejects_scalar_group_targets_without_mutation() {
    let _cwd_guard = test_support::lock_cwd();
    let tmp = tempfile::tempdir().expect("failed to create temp dir");
    let _dir_guard = CwdGuard::enter(tmp.path());
    write_minimal_domain_project(true);
    write_task_with_scalar_variable();
    write_scalar_group_app_spec();
    let task_before = fs::read_to_string("src/domain/task.rs").expect("failed to read task");
    let plan_before = fs::read_to_string("src/domain/plan.rs").expect("failed to read plan");
    let spec_before = fs::read_to_string("solverforge.app.toml").expect("failed to read app spec");

    let err = run_entity("task", true)
        .expect_err("destroy entity should reject scalar-group dependencies");

    assert!(
        err.to_string()
            .contains("cannot destroy entity 'Task' because scalar groups still target it: required_assignment"),
        "unexpected error: {err}"
    );
    assert!(Path::new("src/domain/task.rs").exists());
    assert_eq!(
        fs::read_to_string("src/domain/task.rs").expect("failed to read task"),
        task_before
    );
    assert_eq!(
        fs::read_to_string("src/domain/plan.rs").expect("failed to read plan"),
        plan_before
    );
    assert_eq!(
        fs::read_to_string("solverforge.app.toml").expect("failed to read app spec"),
        spec_before
    );
}

#[test]
fn destroy_variable_rejects_scalar_group_targets_without_mutation() {
    let _cwd_guard = test_support::lock_cwd();
    let tmp = tempfile::tempdir().expect("failed to create temp dir");
    let _dir_guard = CwdGuard::enter(tmp.path());
    write_minimal_domain_project(true);
    write_task_with_scalar_variable();
    write_scalar_group_app_spec();
    let task_before = fs::read_to_string("src/domain/task.rs").expect("failed to read task");
    let spec_before = fs::read_to_string("solverforge.app.toml").expect("failed to read app spec");

    let err = super::run_variable("resource_idx", "Task", true)
        .expect_err("destroy variable should reject scalar-group dependencies");

    assert!(
        err.to_string().contains(
            "cannot destroy variable 'Task.resource_idx' because scalar groups still target it: required_assignment"
        ),
        "unexpected error: {err}"
    );
    assert_eq!(
        fs::read_to_string("src/domain/task.rs").expect("failed to read task"),
        task_before
    );
    assert_eq!(
        fs::read_to_string("solverforge.app.toml").expect("failed to read app spec"),
        spec_before
    );
}

#[test]
fn destroy_variable_ignores_disabled_scalar_group_targets() {
    let _cwd_guard = test_support::lock_cwd();
    let tmp = tempfile::tempdir().expect("failed to create temp dir");
    let _dir_guard = CwdGuard::enter(tmp.path());
    write_minimal_domain_project(true);
    write_task_with_scalar_variable();
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
field = "resource_idx"
"#,
    )
    .expect("failed to write app spec");
    fs::create_dir_all("src/data").expect("failed to create data dir");

    super::run_variable("resource_idx", "Task", true)
        .expect("disabled scalar group should not block variable destroy");

    let task = fs::read_to_string("src/domain/task.rs").expect("failed to read task");
    assert!(
        !task.contains("resource_idx"),
        "variable should be removed when only disabled scalar groups target it: {task}"
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

#[test]
fn destroy_constraint_rejects_conflict_repair_refs_without_mutation() {
    let _cwd_guard = test_support::lock_cwd();
    let tmp = tempfile::tempdir().expect("failed to create temp dir");
    let _dir_guard = CwdGuard::enter(tmp.path());
    write_minimal_domain_project(true);
    write_all_assigned_constraint();
    fs::write(
        "solverforge.app.toml",
        r#"
[[conflict_repairs]]
constraint = "all_assigned"
provider = "repair_all_assigned"
selector = "compound"
solver_config = false
enabled = true
"#,
    )
    .expect("failed to write app spec");
    fs::write("solver.toml", "# test config\n").expect("failed to write solver config");
    let constraint_before =
        fs::read_to_string("src/constraints/all_assigned.rs").expect("failed to read constraint");
    let mod_before =
        fs::read_to_string("src/constraints/mod.rs").expect("failed to read constraints mod");
    let spec_before = fs::read_to_string("solverforge.app.toml").expect("failed to read app spec");

    let err = run_constraint("all_assigned", true)
        .expect_err("destroy constraint should reject conflict-repair dependencies");

    assert!(
        err.to_string().contains(
            "cannot destroy constraint 'all_assigned' because conflict repairs still reference it: conflict repair repair_all_assigned"
        ),
        "unexpected error: {err}"
    );
    assert_eq!(
        fs::read_to_string("src/constraints/all_assigned.rs").expect("failed to read constraint"),
        constraint_before
    );
    assert_eq!(
        fs::read_to_string("src/constraints/mod.rs").expect("failed to read constraints mod"),
        mod_before
    );
    assert_eq!(
        fs::read_to_string("solverforge.app.toml").expect("failed to read app spec"),
        spec_before
    );
}

#[test]
fn destroy_conflict_repair_requires_exact_constraint_id() {
    let _cwd_guard = test_support::lock_cwd();
    let tmp = tempfile::tempdir().expect("failed to create temp dir");
    let _dir_guard = CwdGuard::enter(tmp.path());
    write_minimal_domain_project(true);
    write_all_assigned_constraint();
    fs::write(
        "solverforge.app.toml",
        r#"
[[conflict_repairs]]
constraint = "all_assigned"
provider = "repair_all_assigned"
selector = "compound"
solver_config = false
enabled = true
"#,
    )
    .expect("failed to write app spec");
    fs::write("solver.toml", "# test config\n").expect("failed to write solver config");
    let spec_before = fs::read_to_string("solverforge.app.toml").expect("failed to read app spec");

    let err = run_conflict_repair("repair_all_assigned", true)
        .expect_err("provider name must not identify conflict repair resources");

    assert!(
        err.to_string()
            .contains("conflict repair 'repair_all_assigned' not found"),
        "unexpected error: {err}"
    );
    assert_eq!(
        fs::read_to_string("solverforge.app.toml").expect("failed to read app spec"),
        spec_before
    );
}

#[test]
fn destroy_conflict_repair_blocks_unmanaged_shared_selector_without_mutation() {
    let _cwd_guard = test_support::lock_cwd();
    let tmp = tempfile::tempdir().expect("failed to create temp dir");
    let _dir_guard = CwdGuard::enter(tmp.path());
    write_minimal_domain_project(true);
    write_all_assigned_constraint();
    fs::write(
        "solverforge.app.toml",
        r#"
[[conflict_repairs]]
constraint = "all_assigned"
provider = "repair_all_assigned"
selector = "compound"
solver_config = true
enabled = true

[[conflict_repairs]]
constraint = "other_constraint"
provider = "repair_other_constraint"
selector = "compound"
solver_config = true
enabled = true
"#,
    )
    .expect("failed to write app spec");
    fs::write(
        "solver.toml",
        r#"
[[phases]]
type = "local_search"

[phases.move_selector]
type = "compound_conflict_repair_move_selector"
constraints = ["all_assigned", "other_constraint"]
require_hard_improvement = true
max_moves_per_step = 12
"#,
    )
    .expect("failed to write solver config");

    let plan_before = fs::read_to_string("src/domain/plan.rs").expect("failed to read plan");
    let spec_before = fs::read_to_string("solverforge.app.toml").expect("failed to read app spec");
    let solver_before = fs::read_to_string("solver.toml").expect("failed to read solver config");

    let err = run_conflict_repair("all_assigned", true)
        .expect_err("unmanaged shared selector should block conflict repair destroy");

    assert!(
        err.to_string()
            .contains("phases[0].move_selector.constraints"),
        "unexpected error: {err}"
    );
    assert_eq!(
        fs::read_to_string("src/domain/plan.rs").expect("failed to read plan"),
        plan_before
    );
    assert_eq!(
        fs::read_to_string("solverforge.app.toml").expect("failed to read app spec"),
        spec_before
    );
    assert_eq!(
        fs::read_to_string("solver.toml").expect("failed to read solver config"),
        solver_before
    );
}

#[test]
fn destroy_scalar_group_blocks_nested_solver_config_refs_without_mutation() {
    let _cwd_guard = test_support::lock_cwd();
    let tmp = tempfile::tempdir().expect("failed to create temp dir");
    let _dir_guard = CwdGuard::enter(tmp.path());
    write_minimal_domain_project(true);
    write_task_with_scalar_variable();
    write_scalar_group_app_spec();
    fs::write(
        "solver.toml",
        r#"
[[phases]]
type = "local_search"

[phases.move_selector]
type = "union_move_selector"

[[phases.move_selector.selectors]]
type = "grouped_scalar_move_selector"
group_name = "required_assignment"
"#,
    )
    .expect("failed to write solver config");
    let plan_before = fs::read_to_string("src/domain/plan.rs").expect("failed to read plan");
    let spec_before = fs::read_to_string("solverforge.app.toml").expect("failed to read app spec");
    let solver_before = fs::read_to_string("solver.toml").expect("failed to read solver config");

    let err = super::run_scalar_group("required_assignment", true)
        .expect_err("nested solver config ref should block scalar group destroy");

    assert!(
        err.to_string()
            .contains("phases[0].move_selector.selectors[0].group_name"),
        "unexpected error: {err}"
    );
    assert_eq!(
        fs::read_to_string("src/domain/plan.rs").expect("failed to read plan"),
        plan_before
    );
    assert_eq!(
        fs::read_to_string("solverforge.app.toml").expect("failed to read app spec"),
        spec_before
    );
    assert_eq!(
        fs::read_to_string("solver.toml").expect("failed to read solver config"),
        solver_before
    );
}

#[test]
fn destroy_conflict_repair_blocks_nested_solver_config_refs_without_mutation() {
    let _cwd_guard = test_support::lock_cwd();
    let tmp = tempfile::tempdir().expect("failed to create temp dir");
    let _dir_guard = CwdGuard::enter(tmp.path());
    write_minimal_domain_project(true);
    write_all_assigned_constraint();
    fs::write(
        "solverforge.app.toml",
        r#"
[[conflict_repairs]]
constraint = "all_assigned"
provider = "repair_all_assigned"
selector = "compound"
solver_config = true
enabled = true
"#,
    )
    .expect("failed to write app spec");
    fs::write(
        "solver.toml",
        r#"
[[phases]]
type = "local_search"

[phases.move_selector]
type = "limited_neighborhood"
selected_count_limit = 4

[phases.move_selector.selector]
type = "compound_conflict_repair_move_selector"
constraints = ["all_assigned"]
"#,
    )
    .expect("failed to write solver config");
    let plan_before = fs::read_to_string("src/domain/plan.rs").expect("failed to read plan");
    let spec_before = fs::read_to_string("solverforge.app.toml").expect("failed to read app spec");
    let solver_before = fs::read_to_string("solver.toml").expect("failed to read solver config");

    let err = super::run_conflict_repair("all_assigned", true)
        .expect_err("nested solver config ref should block conflict repair destroy");

    assert!(
        err.to_string()
            .contains("phases[0].move_selector.selector.constraints"),
        "unexpected error: {err}"
    );
    assert_eq!(
        fs::read_to_string("src/domain/plan.rs").expect("failed to read plan"),
        plan_before
    );
    assert_eq!(
        fs::read_to_string("solverforge.app.toml").expect("failed to read app spec"),
        spec_before
    );
    assert_eq!(
        fs::read_to_string("solver.toml").expect("failed to read solver config"),
        solver_before
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

fn write_all_assigned_constraint() {
    fs::create_dir_all("src/constraints").expect("failed to create constraints dir");
    fs::write(
        "src/constraints/mod.rs",
        r#"use crate::domain::Plan;
use solverforge::prelude::*;

pub use self::assemble::create_constraints;

// @solverforge:begin constraint-modules
mod all_assigned;
// @solverforge:end constraint-modules

mod assemble {
    use super::*;

    pub fn create_constraints() -> impl ConstraintSet<Plan, HardSoftScore> {
        // @solverforge:begin constraint-calls
        all_assigned::constraint()
        // @solverforge:end constraint-calls
    }
}
"#,
    )
    .expect("failed to write constraints mod");
    fs::write(
        "src/constraints/all_assigned.rs",
        r#"pub fn constraint() {
    something.named("all_assigned");
}
"#,
    )
    .expect("failed to write constraint file");
}

fn write_task_with_scalar_variable() {
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
    #[planning_variable(value_range_provider = "resources", allows_unassigned = true)]
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
    .expect("failed to write task with scalar variable");
}

fn write_scalar_group_app_spec() {
    fs::write(
        "solverforge.app.toml",
        r#"
[[scalar_groups]]
name = "required_assignment"
kind = "assignment"
solver_config = true
enabled = true

[[scalar_groups.targets]]
entity = "task"
entity_plural = "tasks"
field = "resource_idx"
"#,
    )
    .expect("failed to write scalar group app spec");
}
