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
    let src = test_support::constraint_sources(
        "mod all_assigned;\nmod extra;\n",
        "    (\n        super::all_assigned::constraint(),\n        super::extra::constraint(),\n    )\n",
    );

    let updated =
        remove_constraint_from_source(&src, "all_assigned").expect("source should be rewritten");
    assert!(!updated.module.contains("mod all_assigned;"));
    assert!(!updated.assemble.contains("all_assigned::constraint()"));
    assert!(updated.assemble.contains("super::extra::constraint(),"));
}

#[test]
fn removes_last_constraint_to_empty_tuple() {
    let src = test_support::constraint_sources(
        "mod extra;\n",
        "    (\n        super::extra::constraint(),\n    )\n",
    );

    let updated = remove_constraint_from_source(&src, "extra").expect("source should be rewritten");
    assert!(!updated.module.contains("mod extra;"));
    assert!(updated
        .assemble
        .contains("    // @solverforge:begin constraint-calls\n    ()\n"));
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
fn destroy_entity_resyncs_data_seed() {
    let _cwd_guard = test_support::lock_cwd();
    let tmp = tempfile::tempdir().expect("failed to create temp dir");
    let _dir_guard = CwdGuard::enter(tmp.path());
    write_minimal_domain_project(true);
    write_stale_data_seed();

    run_entity("task", true).expect("destroy entity should succeed");

    let data_seed = fs::read_to_string("src/data/data_seed.rs").expect("failed to read data seed");
    assert!(
        data_seed.contains("// @generated by solverforge-cli: data v1")
            && data_seed.contains("use crate::domain::{Plan, Resource};")
            && !data_seed.contains("Task"),
        "destroy entity should resync the compiler-owned data seed: {data_seed}"
    );
}

#[test]
fn destroy_fact_resyncs_data_seed() {
    let _cwd_guard = test_support::lock_cwd();
    let tmp = tempfile::tempdir().expect("failed to create temp dir");
    let _dir_guard = CwdGuard::enter(tmp.path());
    write_minimal_domain_project(true);
    write_stale_data_seed();

    run_fact("resource", true).expect("destroy fact should succeed");

    let data_seed = fs::read_to_string("src/data/data_seed.rs").expect("failed to read data seed");
    assert!(
        data_seed.contains("// @generated by solverforge-cli: data v1")
            && data_seed.contains("use crate::domain::{Plan, Task};")
            && !data_seed.contains("Resource"),
        "destroy fact should resync the compiler-owned data seed: {data_seed}"
    );
}

#[test]
fn destroy_constraint_resyncs_data_seed() {
    let _cwd_guard = test_support::lock_cwd();
    let tmp = tempfile::tempdir().expect("failed to create temp dir");
    let _dir_guard = CwdGuard::enter(tmp.path());
    write_minimal_domain_project(true);
    write_all_assigned_constraint();
    write_stale_data_seed();

    run_constraint("all_assigned", true).expect("destroy constraint should succeed");

    let data_seed = fs::read_to_string("src/data/data_seed.rs").expect("failed to read data seed");
    assert!(
        data_seed.contains("// @generated by solverforge-cli: data v1")
            && data_seed.contains("use crate::domain::{Plan, Resource, Task};")
            && data_seed.contains("Plan::new(resources, tasks)"),
        "destroy constraint should resync the compiler-owned data seed: {data_seed}"
    );
}

#[test]
fn destroy_constraint_keeps_hand_written_data_seed() {
    let _cwd_guard = test_support::lock_cwd();
    let tmp = tempfile::tempdir().expect("failed to create temp dir");
    let _dir_guard = CwdGuard::enter(tmp.path());
    write_minimal_domain_project(true);
    write_all_assigned_constraint();
    let hand_written = "// Hand-written benchmark data.\nmod large;\npub use large::generate;\n";
    fs::create_dir_all("src/data").expect("failed to create data dir");
    fs::write("src/data/data_seed.rs", hand_written).expect("failed to write data seed");

    run_constraint("all_assigned", true).expect("destroy constraint should succeed");

    assert_eq!(
        fs::read_to_string("src/data/data_seed.rs").expect("failed to read data seed"),
        hand_written,
        "a seed without the @generated header belongs to the app and must not be rewritten"
    );
}

#[test]
fn destroy_solution_skips_data_seed_resync_without_a_solution() {
    let _cwd_guard = test_support::lock_cwd();
    let tmp = tempfile::tempdir().expect("failed to create temp dir");
    let _dir_guard = CwdGuard::enter(tmp.path());
    write_minimal_domain_project(true);
    write_stale_data_seed();

    run_solution(true).expect("destroy solution should succeed");

    let data_seed = fs::read_to_string("src/data/data_seed.rs").expect("failed to read data seed");
    assert_eq!(
        data_seed, STALE_DATA_SEED,
        "without a planning solution there is nothing to re-render; the stale seed must be left untouched"
    );
}

const STALE_DATA_SEED: &str = r#"// @generated by solverforge-cli: data v1

use crate::domain::{Plan, Resource, Task};

pub fn generate() -> Plan {
    Plan::new(vec![], vec![], vec![])
}
"#;

fn write_stale_data_seed() {
    fs::create_dir_all("src/data").expect("failed to create data dir");
    fs::write("src/data/data_seed.rs", STALE_DATA_SEED).expect("failed to write stale data seed");
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

/// Writes constraint module `name` and declares it in the managed constraint
/// files, as `solverforge generate constraint` does.
fn write_task_constraint(name: &str, body: &str) {
    test_support::write_constraint_sources(
        &format!("mod {name};\n"),
        &format!("    super::{name}::constraint()\n"),
    );
    fs::write(format!("src/constraints/{name}.rs"), body).expect("failed to write constraint");
}

fn snapshot_domain() -> Vec<(String, String)> {
    [
        "src/domain/task.rs",
        "src/domain/resource.rs",
        "src/domain/plan.rs",
        "src/domain/mod.rs",
    ]
    .iter()
    .map(|path| {
        (
            path.to_string(),
            fs::read_to_string(path).expect("failed to read domain file"),
        )
    })
    .collect()
}

fn assert_domain_unchanged(before: &[(String, String)]) {
    for (path, contents) in before {
        assert_eq!(
            &fs::read_to_string(path).unwrap_or_else(|_| panic!("{path} must still exist")),
            contents,
            "failed dependency check must not rewrite {path}"
        );
    }
}

#[test]
fn destroy_entity_rejects_source_references_without_mutation() {
    let _cwd_guard = test_support::lock_cwd();
    let tmp = tempfile::tempdir().expect("failed to create temp dir");
    let _dir_guard = CwdGuard::enter(tmp.path());
    write_minimal_domain_project(true);
    write_task_constraint(
        "all_assigned",
        r#"use crate::domain::{Plan, Task};

fn unary_condition(_entity: &Task) -> bool {
    true
}
"#,
    );
    fs::create_dir_all("src/api").expect("failed to create api dir");
    fs::write(
        "src/api/stats.rs",
        "pub fn count(tasks: &[crate::domain::Task]) -> usize {\n    tasks.len()\n}\n",
    )
    .expect("failed to write api file");
    let before = snapshot_domain();

    let err = run_entity("task", true)
        .expect_err("destroy entity should fail while source still references the type");

    let message = err.to_string();
    assert!(
        message.contains(
            "cannot destroy entity 'Task' because source files still reference it: src/api/stats.rs, src/constraints/all_assigned.rs"
        ) && message.contains("`solverforge destroy --yes constraint all_assigned`")
            && message.contains("remove the references to 'Task' from src/api/stats.rs"),
        "unexpected error: {message}"
    );
    assert_domain_unchanged(&before);
}

#[test]
fn destroy_entity_rejects_solution_collection_accessor_references() {
    let _cwd_guard = test_support::lock_cwd();
    let tmp = tempfile::tempdir().expect("failed to create temp dir");
    let _dir_guard = CwdGuard::enter(tmp.path());
    write_minimal_domain_project(true);
    write_task_constraint(
        "all_assigned",
        r#"use crate::domain::Plan;

pub fn constraint() {
    ConstraintFactory::<Plan, HardSoftScore>::new()
        .for_each(Plan :: tasks())
        .named("all_assigned");
}
"#,
    );
    let before = snapshot_domain();

    let err = run_entity("task", true)
        .expect_err("destroy entity should fail while a constraint streams its collection");

    assert!(
        err.to_string().contains(
            "cannot destroy entity 'Task' because source files still reference it: src/constraints/all_assigned.rs"
        ),
        "unexpected error: {err}"
    );
    assert_domain_unchanged(&before);
}

#[test]
fn destroy_fact_rejects_constraint_references_without_mutation() {
    let _cwd_guard = test_support::lock_cwd();
    let tmp = tempfile::tempdir().expect("failed to create temp dir");
    let _dir_guard = CwdGuard::enter(tmp.path());
    write_minimal_domain_project(true);
    write_task_constraint(
        "skill",
        r#"use crate::domain::{Plan, Resource, Task};

fn join_condition(_entity: &Task, _fact: &Resource) -> bool {
    true
}
"#,
    );
    let before = snapshot_domain();

    let err = run_fact("resource", true)
        .expect_err("destroy fact should fail while a constraint still joins it");

    assert!(
        err.to_string().contains(
            "cannot destroy fact 'Resource' because source files still reference it: src/constraints/skill.rs"
        ) && err
            .to_string()
            .contains("`solverforge destroy --yes constraint skill`"),
        "unexpected error: {err}"
    );
    assert_domain_unchanged(&before);
}

#[test]
fn destroy_entity_ignores_mentions_in_comments_and_literals() {
    let _cwd_guard = test_support::lock_cwd();
    let tmp = tempfile::tempdir().expect("failed to create temp dir");
    let _dir_guard = CwdGuard::enter(tmp.path());
    write_minimal_domain_project(true);
    write_task_constraint(
        "notes",
        r##"// Task used to be modelled here; see Plan::tasks().
/* nested /* Task */ comment */
pub const LABEL: &str = "Task";
pub const RAW: &str = r#"Plan::tasks() and "Task""#;
pub const QUOTE: char = '"';
pub fn tasks_label<'a>(value: &'a str) -> &'a str {
    value
}
"##,
    );
    write_stale_data_seed();

    run_entity("task", true).expect("mentions outside code must not block destroy");

    assert!(!Path::new("src/domain/task.rs").exists());
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
    test_support::write_constraint_sources("", "    super::all_assigned::constraint()\n");
    let invalid_mod = "mod all_assigned;\nmod assemble;\n\npub use assemble::create_constraints;\n";
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
    test_support::write_constraint_sources(
        "mod all_assigned;\n",
        "    super::all_assigned::constraint()\n",
    );
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
