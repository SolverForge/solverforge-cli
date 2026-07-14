use super::{
    generators::{generate_entity, generate_fact, generate_solution},
    run::remove_neutral_scaffold,
    utils::{pluralize, snake_to_pascal, validate_score_type},
    wiring::{add_import, replace_score_type},
};
use crate::list_variable_metadata::ListVariableMetadata;
use crate::managed_block;
use crate::scalar_variable_hooks::ScalarVariableHooks;
use crate::test_support;

fn generate_builtin_entity(
    pascal: &str,
    planning_variable: Option<&str>,
    extra_fields: &[(String, String)],
) -> Result<String, String> {
    let _cwd_guard = test_support::lock_cwd();
    generate_entity(pascal, planning_variable, extra_fields)
}

fn generate_builtin_fact(pascal: &str, extra_fields: &[(String, String)]) -> String {
    let _cwd_guard = test_support::lock_cwd();
    generate_fact(pascal, extra_fields)
}

fn generate_builtin_solution(pascal: &str, score: &str) -> Result<String, String> {
    let _cwd_guard = test_support::lock_cwd();
    generate_solution(pascal, score)
}

#[test]
fn test_snake_to_pascal() {
    assert_eq!(snake_to_pascal("shift"), "Shift");
    assert_eq!(snake_to_pascal("employee_schedule"), "EmployeeSchedule");
    assert_eq!(
        snake_to_pascal("vehicle_routing_plan"),
        "VehicleRoutingPlan"
    );
    assert_eq!(snake_to_pascal("plan"), "Plan");
}

#[test]
fn test_pluralize() {
    assert_eq!(pluralize("shift"), "shifts");
    assert_eq!(pluralize("employee"), "employees");
    assert_eq!(pluralize("bus"), "buses");
    assert_eq!(pluralize("category"), "categories");
    assert_eq!(pluralize("day"), "days");
    assert_eq!(pluralize("key"), "keys");
    assert_eq!(pluralize("task"), "tasks");
}

#[test]
fn test_validate_score_type() {
    assert!(validate_score_type("HardSoftScore").is_ok());
    assert!(validate_score_type("HardSoftDecimalScore").is_ok());
    assert!(validate_score_type("HardMediumSoftScore").is_ok());
    assert!(validate_score_type("SoftScore").is_ok());
    assert!(validate_score_type("BendableScore<2, 3>").is_ok());
    assert!(validate_score_type("SimpleScore").is_err());
    assert!(validate_score_type("BendableScore").is_err());
    assert!(validate_score_type("FakeScore").is_err());
}

#[test]
fn test_generate_entity_no_var() {
    let src = generate_builtin_entity("Shift", None, &[])
        .expect("built-in entity template should render");
    assert!(src.contains("#[planning_entity]"));
    assert!(src.contains("pub struct Shift"));
    assert!(src.contains("#[planning_id]"));
    assert!(src.contains("pub id: String"));
    assert!(!src.contains("#[planning_variable]"));
    assert!(src.contains("@solverforge:begin entity-variables"));
    assert!(src.contains("@solverforge:end entity-variable-init"));
}

#[test]
fn test_generate_entity_with_var() {
    let src = generate_builtin_entity("Shift", Some("employee_idx"), &[])
        .expect("built-in entity template should render");
    assert!(src.contains("#[planning_variable(allows_unassigned = true)]"));
    assert!(src.contains("pub employee_idx: Option<usize>"));
    assert!(src.contains("employee_idx: None"));
}

#[test]
fn test_generate_fact() {
    let src = generate_builtin_fact("Employee", &[]);
    assert!(src.contains("#[problem_fact]"));
    assert!(src.contains("pub struct Employee"));
    assert!(src.contains("#[planning_id]"));
    assert!(src.contains("pub id: String"));
    assert!(src.contains("pub name: String"));
}

#[test]
fn test_generate_solution() {
    let src = generate_builtin_solution("Schedule", "HardSoftDecimalScore")
        .expect("built-in solution template should render");
    assert!(src.contains("#[planning_solution("));
    assert!(src.contains("constraints = \"crate::constraints::create_constraints\""));
    assert!(src.contains("solver_toml = \"../../solver.toml\""));
    assert!(src.contains("pub struct Schedule"));
    assert!(src.contains("#[planning_score]"));
    assert!(src.contains("pub score: Option<HardSoftDecimalScore>"));
    assert!(src.contains("@solverforge:begin solution-collections"));
    assert!(src.contains("@solverforge:begin solution-constructor-init"));
}

#[test]
fn test_generate_entity_output_satisfies_managed_block_contract() {
    let src = generate_builtin_entity("Shift", Some("employee_idx"), &[])
        .expect("built-in entity template should render");
    managed_block::require_blocks(&src, managed_block::ENTITY_REQUIRED_BLOCKS)
        .expect("entity output should keep canonical managed blocks");
}

#[test]
fn test_generate_solution_output_satisfies_managed_block_contract() {
    let src = generate_builtin_solution("Schedule", "HardSoftDecimalScore")
        .expect("built-in solution template should render");
    managed_block::require_blocks(&src, managed_block::SOLUTION_REQUIRED_BLOCKS)
        .expect("solution output should keep canonical managed blocks");
}

#[test]
fn test_free_form_entity_template_output_is_rejected() {
    let err = managed_block::require_blocks(
        "pub struct Shift {\n    pub id: String,\n}\n",
        managed_block::ENTITY_REQUIRED_BLOCKS,
    )
    .expect_err("free-form entity output should be rejected");

    assert_eq!(
        err,
        "missing or duplicated managed block markers for 'entity-variables'"
    );
}

#[test]
fn test_free_form_solution_template_output_is_rejected() {
    let err = managed_block::require_blocks(
        "pub struct Schedule {\n    pub score: Option<HardSoftScore>,\n}\n",
        managed_block::SOLUTION_REQUIRED_BLOCKS,
    )
    .expect_err("free-form solution output should be rejected");

    assert_eq!(
        err,
        "missing or duplicated managed block markers for 'solution-imports'"
    );
}

#[test]
fn test_add_import_new() {
    let src = "use solverforge::prelude::*;\n\nstruct Foo;\n";
    let result = add_import(src, "use super::Bar;");
    assert!(result.contains("use super::Bar;"));
    let use_pos = result.find("use solverforge").unwrap();
    let bar_pos = result.find("use super::Bar;").unwrap();
    assert!(bar_pos > use_pos);
}

#[test]
fn test_add_import_idempotent() {
    let src = "use super::Bar;\nstruct Foo;\n";
    let result = add_import(src, "use super::Bar;");
    assert_eq!(result.matches("use super::Bar;").count(), 1);
}

#[test]
fn test_replace_score_type() {
    let src = "pub score: Option<HardSoftScore>,\n";
    let result = replace_score_type(src, "HardSoftScore", "HardSoftDecimalScore").unwrap();
    assert!(result.contains("HardSoftDecimalScore"));
    assert!(!result.contains("HardSoftScore"));
}

#[test]
fn test_replace_score_type_missing() {
    let src = "pub score: Option<HardSoftScore>,\n";
    let result = replace_score_type(src, "SoftScore", "HardSoftScore");
    assert!(result.is_err());
}

#[test]
fn test_inject_second_planning_variable() {
    use super::wiring::inject_planning_variable;

    let src = generate_builtin_entity("Surgery", Some("room_idx"), &[])
        .expect("built-in entity template should render");
    let result =
        inject_planning_variable(&src, "Surgery", "slot_idx").expect("inject should succeed");

    assert!(
        result.contains("slot_idx: None"),
        "slot_idx: None not found in output"
    );

    let self_start = result.find("Self {").expect("Self { not found");
    let self_block = &result[self_start..];
    let close = self_block.find('}').expect("} not found after Self {");
    let self_literal = &self_block[..=close];
    assert!(
        self_literal.contains("room_idx: None"),
        "room_idx: None not inside Self {{ }}: got: {self_literal}"
    );
    assert!(
        self_literal.contains("slot_idx: None"),
        "slot_idx: None not inside Self {{ }}: got:\n{result}"
    );
}

#[test]
fn test_inject_list_variable() {
    use super::wiring::inject_list_variable;

    let src = generate_builtin_entity("Route", None, &[])
        .expect("built-in entity template should render");
    let result = inject_list_variable(&src, "Route", "stops", "visits", &Default::default())
        .expect("inject should succeed");

    assert!(result.contains("#[planning_list_variable(element_collection = \"visits\")]"));
    assert!(result.contains("pub stops: Vec<usize>"));
    assert!(result.contains("stops: Vec::new()"));
}

#[test]
fn test_inject_list_variable_with_current_runtime_metadata() {
    use super::wiring::inject_list_variable;

    let src = generate_builtin_entity("Route", None, &[])
        .expect("built-in entity template should render");
    let metadata = ListVariableMetadata {
        domain: None,
        distance_meter: Some("crate::distance::CrossRouteDistance".to_string()),
        intra_distance_meter: Some("crate::distance::WithinRouteDistance".to_string()),
        route_hooks: Some("crate::route_hooks".to_string()),
        savings_hooks: Some("crate::savings_hooks".to_string()),
        savings_metric_class_fn: Some("crate::savings_metric_class".to_string()),
        element_owner_fn: Some("crate::fixed_owner".to_string()),
        construction_element_order_key: Some("crate::visit_order".to_string()),
        precedence_duration_fn: Some("crate::visit_duration".to_string()),
        precedence_successors_fn: Some("crate::visit_successors".to_string()),
        solution_trait: Some("crate::RouteSolution".to_string()),
    };
    let result = inject_list_variable(&src, "Route", "stops", "visits", &metadata)
        .expect("inject should succeed");

    for expected in [
        "element_collection = \"visits\"",
        "distance_meter = \"crate::distance::CrossRouteDistance\"",
        "intra_distance_meter = \"crate::distance::WithinRouteDistance\"",
        "route_hooks = \"crate::route_hooks\"",
        "savings_hooks = \"crate::savings_hooks\"",
        "savings_metric_class_fn = \"crate::savings_metric_class\"",
        "element_owner_fn = \"crate::fixed_owner\"",
        "construction_element_order_key = \"crate::visit_order\"",
        "precedence_duration_fn = \"crate::visit_duration\"",
        "precedence_successors_fn = \"crate::visit_successors\"",
        "solution_trait = \"crate::RouteSolution\"",
    ] {
        assert!(result.contains(expected), "missing {expected}: {result}");
    }
    syn::parse_file(&result).expect("generated entity should remain valid Rust syntax");
}

#[test]
fn test_inject_list_variable_with_cvrp_profile() {
    use super::wiring::inject_list_variable;

    let src = generate_builtin_entity("Route", None, &[])
        .expect("built-in entity template should render");
    let metadata = ListVariableMetadata {
        domain: Some("cvrp".to_string()),
        construction_element_order_key: Some("crate::visit_order".to_string()),
        ..ListVariableMetadata::default()
    };
    metadata
        .validate()
        .expect("stock CVRP profile should validate");
    let result = inject_list_variable(&src, "Route", "stops", "visits", &metadata)
        .expect("inject should succeed");

    assert!(result.contains("domain = \"cvrp\""));
    assert!(result.contains("construction_element_order_key = \"crate::visit_order\""));
    assert!(!result.contains("route_hooks ="));
}

#[test]
fn test_inject_scalar_variable_with_hook_metadata() {
    use super::wiring::{inject_scalar_variable, ScalarVariableWiring};

    let src =
        generate_builtin_entity("Task", None, &[]).expect("built-in entity template should render");
    let hooks = ScalarVariableHooks {
        candidate_values: Some("resource_candidates".to_string()),
        nearby_value_candidates: Some("nearby_resources".to_string()),
        nearby_entity_candidates: Some("nearby_tasks".to_string()),
        nearby_value_distance_meter: Some("resource_distance".to_string()),
        nearby_entity_distance_meter: Some("task_distance".to_string()),
        construction_entity_order_key: Some("task_priority".to_string()),
        construction_value_order_key: Some("resource_priority".to_string()),
    };
    let result = inject_scalar_variable(
        &src,
        "Task",
        "resource_idx",
        ScalarVariableWiring {
            range: "resources",
            countable_range: None,
            allows_unassigned: true,
            hooks: &hooks,
        },
    )
    .expect("inject should succeed");

    assert!(result.contains(
        r#"    #[planning_variable(
        value_range_provider = "resources",
        allows_unassigned = true,
        candidate_values = "resource_candidates",
        nearby_value_candidates = "nearby_resources",
        nearby_entity_candidates = "nearby_tasks",
        nearby_value_distance_meter = "resource_distance",
        nearby_entity_distance_meter = "task_distance",
        construction_entity_order_key = "task_priority",
        construction_value_order_key = "resource_priority"
    )]"#
    ));
    assert!(result.contains("pub resource_idx: Option<usize>,"));
    assert!(result.contains("resource_idx: None,"));
}

#[test]
fn test_inject_scalar_variable_with_countable_range() {
    use super::wiring::{inject_scalar_variable, ScalarVariableWiring};

    let src =
        generate_builtin_entity("Task", None, &[]).expect("built-in entity template should render");
    let result = inject_scalar_variable(
        &src,
        "Task",
        "priority",
        ScalarVariableWiring {
            range: "",
            countable_range: Some("2..7"),
            allows_unassigned: false,
            hooks: &ScalarVariableHooks::default(),
        },
    )
    .expect("inject should succeed");

    assert!(result
        .contains(r#"#[planning_variable(countable_range = "2..7", allows_unassigned = false)]"#));
    assert!(result.contains("pub priority: Option<usize>"));
}

#[test]
fn test_remove_variable_field() {
    use super::wiring::{inject_list_variable, inject_planning_variable, remove_variable_field};

    let src = generate_builtin_entity("Route", Some("driver_idx"), &[])
        .expect("built-in entity template should render");
    let src = inject_list_variable(&src, "Route", "stops", "visits", &Default::default())
        .expect("list inject");
    let src = inject_planning_variable(&src, "Route", "backup_idx").expect("var inject");
    let result = remove_variable_field(&src, "stops").expect("remove should succeed");

    assert!(!result.contains("pub stops: Vec<usize>"));
    assert!(!result.contains("#[planning_list_variable"));
    assert!(!result.contains("stops: Vec::new()"));
    assert!(result.contains("pub driver_idx: Option<usize>"));
    assert!(result.contains("pub backup_idx: Option<usize>"));
}

#[test]
fn test_remove_variable_field_removes_multiline_hook_attribute() {
    use super::wiring::{
        inject_list_variable, inject_scalar_variable, remove_variable_field, ScalarVariableWiring,
    };

    let src =
        generate_builtin_entity("Task", None, &[]).expect("built-in entity template should render");
    let hooks = ScalarVariableHooks {
        candidate_values: Some("resource_candidates".to_string()),
        nearby_value_candidates: Some("nearby_resources".to_string()),
        nearby_entity_candidates: None,
        nearby_value_distance_meter: Some("resource_distance".to_string()),
        nearby_entity_distance_meter: None,
        construction_entity_order_key: Some("task_priority".to_string()),
        construction_value_order_key: None,
    };
    let src = inject_scalar_variable(
        &src,
        "Task",
        "resource_idx",
        ScalarVariableWiring {
            range: "resources",
            countable_range: None,
            allows_unassigned: true,
            hooks: &hooks,
        },
    )
    .expect("scalar inject should succeed");
    let src = inject_list_variable(&src, "Task", "stops", "visits", &Default::default())
        .expect("list inject should succeed");

    let result = remove_variable_field(&src, "resource_idx").expect("remove should succeed");

    assert!(!result.contains("pub resource_idx: Option<usize>"));
    assert!(!result.contains("resource_idx: None,"));
    assert!(!result.contains("resource_candidates"));
    assert!(!result.contains("nearby_resources"));
    assert!(!result.contains("resource_distance"));
    assert!(!result.contains("task_priority"));
    assert!(result.contains("#[planning_list_variable(element_collection = \"visits\")]"));
    assert!(result.contains("pub stops: Vec<usize>,"));
    assert!(result.contains("stops: Vec::new()"));
    syn::parse_file(&result).expect("entity source should still parse");
}

#[test]
fn test_update_domain_mod_format() {
    let mod_line = format!("mod {};", "shift");
    let use_line = format!("pub use {}::{};", "shift", "Shift");
    assert_eq!(mod_line, "mod shift;");
    assert_eq!(use_line, "pub use shift::Shift;");
}

#[test]
fn test_rewrite_domain_mod_source_groups_mods_before_uses() {
    use super::wiring::rewrite_domain_mod_source;

    let src = r#"// @solverforge:begin domain-exports
mod plan;

pub use plan::Plan;
// @solverforge:end domain-exports
"#;

    let rewritten = rewrite_domain_mod_source(src, "task", "Task", Some("plan")).unwrap();

    assert_eq!(
        rewritten,
        "// @solverforge:begin domain-exports\nmod task;\nmod plan;\n\npub use task::Task;\npub use plan::Plan;\n// @solverforge:end domain-exports\n"
    );
}

#[test]
fn test_rewrite_domain_mod_source_preserves_user_owned_tail_content() {
    use super::wiring::rewrite_domain_mod_source;

    let src = r#"// header
// @solverforge:begin domain-exports
mod plan;

pub use plan::Plan;
// @solverforge:end domain-exports

pub use self::plan::Helper;

#[cfg(test)]
mod domain_tests;
"#;

    let rewritten = rewrite_domain_mod_source(src, "resource", "Resource", Some("plan")).unwrap();

    assert_eq!(
        rewritten,
        "// header\n// @solverforge:begin domain-exports\nmod resource;\nmod plan;\n\npub use resource::Resource;\npub use plan::Plan;\n// @solverforge:end domain-exports\n\npub use self::plan::Helper;\n\n#[cfg(test)]\nmod domain_tests;\n"
    );
}

#[test]
fn test_rewrite_domain_mod_source_preserves_self_uses_and_multiple_exports() {
    use super::wiring::rewrite_domain_mod_source;

    let src = r#"// @solverforge:begin domain-exports
mod plan;

pub use self::plan::Plan;
pub use self::plan::PlanBuilder;
// @solverforge:end domain-exports
"#;

    let rewritten = rewrite_domain_mod_source(src, "task", "Task", Some("plan")).unwrap();

    assert!(rewritten.contains("pub use task::Task;"));
    assert!(rewritten.contains("pub use self::plan::Plan;"));
    assert!(rewritten.contains("pub use self::plan::PlanBuilder;"));
}

#[test]
fn test_rewrite_domain_mod_source_requires_managed_exports() {
    use super::wiring::rewrite_domain_mod_source;

    let src = r#"// domain exports
mod plan;

pub use self::plan::Plan;
pub use self::plan::PlanBuilder;

#[cfg(test)]
mod tests;
"#;

    let err = rewrite_domain_mod_source(src, "task", "Task", Some("plan"))
        .expect_err("unmanaged domain mod should fail");

    assert_eq!(
        err,
        "missing or duplicated managed block markers for 'domain-exports'"
    );
}

#[test]
fn test_rewrite_domain_mod_source_requires_primary_re_export_for_each_module() {
    use super::wiring::rewrite_domain_mod_source;

    let src = r#"// @solverforge:begin domain-exports
mod task;
mod plan;

pub use plan::Plan;
pub use self::plan::PlanBuilder;
// @solverforge:end domain-exports
"#;

    let err = rewrite_domain_mod_source(src, "resource", "Resource", Some("plan"))
        .expect_err("missing primary re-export should fail");

    assert_eq!(
        err,
        "managed domain block is missing the primary re-export 'pub use task::Task;' for module 'task'"
    );
}

#[test]
fn test_validate_domain_mod_source_rejects_undeclared_re_export() {
    use super::wiring::validate_domain_mod_source;

    let src = r#"solverforge::planning_model! {
    root = "src/domain";

    // @solverforge:begin domain-exports
mod task;
mod plan;

pub use task::Task;
pub use plan::Plan;
pub use helper::Helper;
// @solverforge:end domain-exports
}
"#;

    let err = validate_domain_mod_source(src).expect_err("undeclared re-export should fail");

    assert_eq!(
        err,
        "managed domain block contains a re-export for undeclared module 'helper'"
    );
}

#[test]
fn test_validate_domain_mod_source_accepts_planning_model_manifest() {
    use super::wiring::validate_domain_mod_source;

    let src = r#"solverforge::planning_model! {
    root = "src/domain";

    // @solverforge:begin domain-exports
mod task;
mod plan;

pub use task::Task;
pub use plan::Plan;
// @solverforge:end domain-exports
}
"#;

    let modules = validate_domain_mod_source(src).expect("manifest should validate");

    assert_eq!(modules, vec!["task".to_string(), "plan".to_string()]);
}

#[test]
fn test_validate_domain_mod_source_requires_managed_block_inside_manifest() {
    use super::wiring::validate_domain_mod_source;

    let src = r#"solverforge::planning_model! {
    root = "src/domain";
}

// @solverforge:begin domain-exports
mod task;
pub use task::Task;
// @solverforge:end domain-exports
"#;

    let err =
        validate_domain_mod_source(src).expect_err("managed block outside manifest should fail");

    assert_eq!(
        err,
        "missing or duplicated managed block markers for 'domain-exports'"
    );
}

#[test]
fn test_wire_collection_into_solution_updates_neutral_constructor() {
    use super::wiring::insert_field_and_import;

    let src = r#"use serde::{Deserialize, Serialize};
use solverforge::prelude::*;
// @solverforge:begin solution-imports
// @solverforge:end solution-imports

#[planning_solution]
#[derive(Serialize, Deserialize)]
pub struct Plan {
    // @solverforge:begin solution-collections
    // @solverforge:end solution-collections
    #[planning_score]
    pub score: Option<HardSoftScore>,
}

impl Plan {
    pub fn new(
        // @solverforge:begin solution-constructor-params
        // @solverforge:end solution-constructor-params
    ) -> Self {
        Self {
            // @solverforge:begin solution-constructor-init
            // @solverforge:end solution-constructor-init
            score: None,
        }
    }
}
"#;

    let result = insert_field_and_import(
        src,
        "Plan",
        "Resource",
        "resources",
        "    #[problem_fact_collection]\n    pub resources: Vec<Resource>,",
    )
    .expect("insert should succeed");

    assert!(
        result.contains("        resources: Vec<Resource>,"),
        "{result}"
    );
    assert!(result.contains("            resources,"), "{result}");
    assert!(result.contains("use super::Resource;"), "{result}");
}

#[test]
fn test_wire_collection_into_solution_requires_managed_blocks() {
    use super::wiring::insert_field_and_import;

    let src = r#"use serde::{Deserialize, Serialize};
use solverforge::prelude::*;

#[planning_solution]
#[derive(Serialize, Deserialize)]
pub struct Plan {
    #[planning_score]
    pub score: Option<HardSoftScore>,
}

impl Plan {
    pub fn new(
    ) -> Self {
        Self {
            score: None,
        }
    }
}
"#;

    let err = insert_field_and_import(
        src,
        "Plan",
        "Resource",
        "resources",
        "    #[problem_fact_collection]\n    pub resources: Vec<Resource>,",
    )
    .expect_err("unmanaged solution should fail");

    assert_eq!(
        err,
        "missing or duplicated managed block markers for 'solution-collections'"
    );
}

#[test]
fn test_variable_injection_requires_managed_blocks() {
    use super::wiring::{inject_scalar_variable, ScalarVariableWiring};

    let src = r#"use serde::{Deserialize, Serialize};
use solverforge::prelude::*;

#[planning_entity]
#[derive(Serialize, Deserialize)]
pub struct Task {
    #[planning_id]
    pub id: String,
}

impl Task {
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
        }
    }
}
"#;

    let err = inject_scalar_variable(
        src,
        "Task",
        "resource_idx",
        ScalarVariableWiring {
            range: "resources",
            countable_range: None,
            allows_unassigned: true,
            hooks: &ScalarVariableHooks::default(),
        },
    )
    .expect_err("unmanaged entity should fail");

    assert_eq!(
        err,
        "missing or duplicated managed block markers for 'entity-variables'"
    );
}

#[test]
fn test_remove_neutral_scaffold_preserves_user_owned_data_wrapper() {
    let guard = test_support::lock_cwd();

    let tmp = tempfile::tempdir().expect("failed to create temp dir");
    let original_dir = std::env::current_dir().expect("failed to get current dir");

    std::fs::create_dir_all(tmp.path().join("src/domain")).expect("failed to create domain dir");
    std::fs::create_dir_all(tmp.path().join("src/constraints"))
        .expect("failed to create constraints dir");
    std::fs::create_dir_all(tmp.path().join("src/api")).expect("failed to create api dir");
    std::fs::create_dir_all(tmp.path().join("src/solver")).expect("failed to create solver dir");
    std::fs::create_dir_all(tmp.path().join("src/data")).expect("failed to create data dir");
    std::fs::write(
        tmp.path().join("src/domain/mod.rs"),
        "pub mod plan;\npub mod task;\npub mod resource;\n",
    )
    .expect("failed to write domain mod");
    std::fs::write(
        tmp.path().join("src/domain/plan.rs"),
        "// @solverforge:neutral-solution\n",
    )
    .expect("failed to write plan");
    std::fs::write(tmp.path().join("src/domain/task.rs"), "placeholder")
        .expect("failed to write task");
    std::fs::write(tmp.path().join("src/domain/resource.rs"), "placeholder")
        .expect("failed to write resource");
    std::fs::write(
        tmp.path().join("src/constraints/all_assigned.rs"),
        "placeholder",
    )
    .expect("failed to write all_assigned");
    std::fs::write(
        tmp.path().join("src/constraints/mod.rs"),
        "mod all_assigned;\n(all_assigned::constraint(),)\n",
    )
    .expect("failed to write constraints mod");
    let custom_data_wrapper = r#"mod data_seed;

pub use data_seed::{generate, DemoData};

pub fn custom_source() -> &'static str {
    "csv"
}
"#;
    std::fs::write(tmp.path().join("src/data/mod.rs"), custom_data_wrapper)
        .expect("failed to write data mod");
    std::fs::write(
        tmp.path().join("src/api/dto.rs"),
        "use solverforge::{HardSoftScore, SolverStatus};\nuse crate::domain::Plan;\npub struct PlanDto;\npub fn dto(plan: &Plan, status: &SolverStatus<HardSoftScore>) {}\n",
    )
    .expect("failed to write dto");
    std::fs::write(
        tmp.path().join("src/solver/service.rs"),
        "use solverforge::{HardSoftScore, SolverEventMetadata, SolverStatus};\nuse crate::domain::Plan;\nstatic MANAGER: SolverManager<Plan> = SolverManager::new();\nfn status() -> SolverStatus<HardSoftScore> {}\nfn event(metadata: &SolverEventMetadata<HardSoftScore>) {}\n",
    )
    .expect("failed to write service");
    std::fs::write(
        tmp.path().join("src/lib.rs"),
        "/* Plan solution */\npub mod domain;\n",
    )
    .expect("failed to write lib");

    std::env::set_current_dir(tmp.path()).expect("failed to enter temp dir");
    let result = remove_neutral_scaffold("Schedule", "HardSoftDecimalScore");
    std::env::set_current_dir(original_dir).expect("failed to restore current dir");
    drop(guard);

    result.expect("remove_neutral_scaffold should succeed");

    let data_mod = std::fs::read_to_string(tmp.path().join("src/data/mod.rs"))
        .expect("failed to read data mod");
    assert_eq!(data_mod, custom_data_wrapper);

    let domain_mod = std::fs::read_to_string(tmp.path().join("src/domain/mod.rs"))
        .expect("failed to read rewritten domain mod");
    assert!(domain_mod.contains("@solverforge:begin domain-exports"));
    assert!(!tmp.path().join("src/domain/plan.rs").exists());
    assert!(tmp.path().join("src/domain/task.rs").exists());
    assert!(tmp.path().join("src/domain/resource.rs").exists());
    assert!(tmp.path().join("src/constraints/all_assigned.rs").exists());

    let constraints_mod = std::fs::read_to_string(tmp.path().join("src/constraints/mod.rs"))
        .expect("failed to read rewritten constraints mod");
    assert!(constraints_mod.contains("@solverforge:begin constraint-modules"));
    assert!(constraints_mod.contains("use crate::domain::Schedule;"));
    assert!(constraints_mod.contains("ConstraintSet<Schedule, HardSoftDecimalScore>"));

    let dto = std::fs::read_to_string(tmp.path().join("src/api/dto.rs"))
        .expect("failed to read rewritten dto");
    assert!(dto.contains("use crate::domain::Schedule;"));
    assert!(dto.contains("plan: &Schedule"));
    assert!(dto.contains("SolverStatus<HardSoftDecimalScore>"));
    assert!(!dto.contains("HardSoftScore,"));
    assert!(!dto.contains("<HardSoftScore>"));
    assert!(
        dto.contains("pub struct PlanDto;"),
        "rewrite should not rename DTO type names: {dto}"
    );
    let service = std::fs::read_to_string(tmp.path().join("src/solver/service.rs"))
        .expect("failed to read rewritten service");
    assert!(service.contains("SolverManager<Schedule>"));
    assert!(service.contains("SolverStatus<HardSoftDecimalScore>"));
    assert!(service.contains("SolverEventMetadata<HardSoftDecimalScore>"));
    assert!(!service.contains("HardSoftScore,"));
    assert!(!service.contains("<HardSoftScore>"));
    let lib = std::fs::read_to_string(tmp.path().join("src/lib.rs"))
        .expect("failed to read rewritten lib");
    assert!(lib.contains("Schedule solution"));
}
