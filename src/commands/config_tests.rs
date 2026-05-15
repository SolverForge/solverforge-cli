use super::*;
use crate::test_support::lock_cwd;
use tempfile::tempdir;

#[test]
fn parse_toml_value_parses_integer_float_bool_and_string() {
    assert_eq!(parse_toml_value("60"), toml::Value::Integer(60));
    assert_eq!(parse_toml_value("1.5"), toml::Value::Float(1.5));
    assert_eq!(parse_toml_value("true"), toml::Value::Boolean(true));
    assert_eq!(parse_toml_value("false"), toml::Value::Boolean(false));
    assert_eq!(
        parse_toml_value("construction"),
        toml::Value::String("construction".to_string())
    );
}

#[test]
fn set_toml_key_sets_top_level_key() {
    let mut doc: toml::Value =
        toml::from_str("[termination]\nseconds_spent_limit = 30\n").expect("valid toml");

    set_toml_key(&mut doc, "name", "demo").expect("top-level set should succeed");

    assert_eq!(doc["name"], toml::Value::String("demo".to_string()));
    assert_eq!(
        doc["termination"]["seconds_spent_limit"],
        toml::Value::Integer(30)
    );
}

#[test]
fn set_toml_key_creates_nested_tables_and_preserves_siblings() {
    let mut doc: toml::Value =
        toml::from_str("[termination]\nscore_calculation_count_limit = 10\n").expect("valid toml");

    set_toml_key(&mut doc, "termination.seconds_spent_limit", "60")
        .expect("nested set should succeed");

    assert_eq!(
        doc["termination"]["seconds_spent_limit"],
        toml::Value::Integer(60)
    );
    assert_eq!(
        doc["termination"]["score_calculation_count_limit"],
        toml::Value::Integer(10)
    );
}

#[test]
fn set_toml_key_updates_array_table_by_index() {
    let mut doc: toml::Value = toml::from_str(
        r#"
[[phases]]
type = "local_search"

[phases.termination]
step_count_limit = 10
"#,
    )
    .expect("valid toml");

    set_toml_key(&mut doc, "phases[0].termination.step_count_limit", "20")
        .expect("array-index set should succeed");

    assert_eq!(
        doc["phases"][0]["termination"]["step_count_limit"],
        toml::Value::Integer(20)
    );
}

#[test]
fn set_toml_key_errors_when_array_index_is_missing() {
    let mut doc: toml::Value =
        toml::from_str("[[phases]]\ntype = \"local_search\"\n").expect("valid toml");

    let err = set_toml_key(&mut doc, "phases[1].termination.step_count_limit", "20")
        .expect_err("missing array index should fail");

    assert_eq!(err.to_string(), "solver.toml `phases` has no index 1");
}

#[test]
fn set_toml_key_errors_when_root_is_not_a_table() {
    let mut doc = toml::Value::String("not-a-table".to_string());

    let err = set_toml_key(&mut doc, "termination.seconds_spent_limit", "60")
        .expect_err("scalar root should fail");

    assert_eq!(err.to_string(), "solver.toml root is not a TOML table");
}

#[test]
fn set_toml_key_errors_when_intermediate_value_is_not_a_table() {
    let mut doc: toml::Value = toml::from_str("termination = 5").expect("valid toml");

    let err = set_toml_key(&mut doc, "termination.seconds_spent_limit", "60")
        .expect_err("scalar intermediate should fail");

    assert_eq!(err.to_string(), "solver.toml root is not a TOML table");
}

#[test]
fn run_set_errors_when_solver_toml_is_missing() {
    let _cwd_guard = lock_cwd();
    let tmp = tempdir().expect("failed to create temp dir");
    let original_dir = std::env::current_dir().expect("failed to read current dir");
    std::env::set_current_dir(tmp.path()).expect("failed to enter temp dir");

    let result = run_set("termination.seconds_spent_limit", "60");

    std::env::set_current_dir(original_dir).expect("failed to restore current dir");

    match result.expect_err("missing solver.toml should fail") {
        CliError::NotInProject { missing } => assert_eq!(missing, "solver.toml"),
        other => panic!("expected NotInProject, got {}", other),
    }
}

#[test]
fn run_set_updates_solver_toml_with_typed_nested_value() {
    let _cwd_guard = lock_cwd();
    let tmp = tempdir().expect("failed to create temp dir");
    let original_dir = std::env::current_dir().expect("failed to read current dir");
    std::env::set_current_dir(tmp.path()).expect("failed to enter temp dir");
    fs::write(
        "solver.toml",
        "[termination]\nscore_calculation_count_limit = 10\n",
    )
    .expect("failed to write solver.toml");

    run_set("termination.seconds_spent_limit", "60").expect("run_set should succeed");

    let saved = fs::read_to_string("solver.toml").expect("failed to read solver.toml");
    std::env::set_current_dir(original_dir).expect("failed to restore current dir");

    let saved_doc: toml::Value = toml::from_str(&saved).expect("saved toml should be valid");
    assert_eq!(
        saved_doc["termination"]["seconds_spent_limit"],
        toml::Value::Integer(60)
    );
    assert_eq!(
        saved_doc["termination"]["score_calculation_count_limit"],
        toml::Value::Integer(10)
    );
}

#[test]
fn run_set_updates_nested_value_in_inline_table() {
    let _cwd_guard = lock_cwd();
    let tmp = tempdir().expect("failed to create temp dir");
    let original_dir = std::env::current_dir().expect("failed to read current dir");
    std::env::set_current_dir(tmp.path()).expect("failed to enter temp dir");
    fs::write(
        "solver.toml",
        "termination = { seconds_spent_limit = 30, score_calculation_count_limit = 10 }\n",
    )
    .expect("failed to write solver.toml");

    run_set("termination.seconds_spent_limit", "60").expect("run_set should succeed");

    let saved = fs::read_to_string("solver.toml").expect("failed to read solver.toml");
    std::env::set_current_dir(original_dir).expect("failed to restore current dir");

    let saved_doc: toml::Value = toml::from_str(&saved).expect("saved toml should be valid");
    assert_eq!(
        saved_doc["termination"]["seconds_spent_limit"],
        toml::Value::Integer(60)
    );
    assert_eq!(
        saved_doc["termination"]["score_calculation_count_limit"],
        toml::Value::Integer(10)
    );
}

#[test]
fn run_set_stores_string_values_as_strings() {
    let _cwd_guard = lock_cwd();
    let tmp = tempdir().expect("failed to create temp dir");
    let original_dir = std::env::current_dir().expect("failed to read current dir");
    std::env::set_current_dir(tmp.path()).expect("failed to enter temp dir");
    fs::write("solver.toml", "[phase]\nenabled = true\n").expect("failed to write solver.toml");

    run_set("phase.name", "construction").expect("run_set should succeed");

    let saved = fs::read_to_string("solver.toml").expect("failed to read solver.toml");
    std::env::set_current_dir(original_dir).expect("failed to restore current dir");

    let saved_doc: toml::Value = toml::from_str(&saved).expect("saved toml should be valid");
    assert_eq!(
        saved_doc["phase"]["name"],
        toml::Value::String("construction".to_string())
    );
    assert_eq!(saved_doc["phase"]["enabled"], toml::Value::Boolean(true));
}

#[test]
fn run_set_reports_parse_errors_for_invalid_toml() {
    let _cwd_guard = lock_cwd();
    let tmp = tempdir().expect("failed to create temp dir");
    let original_dir = std::env::current_dir().expect("failed to read current dir");
    std::env::set_current_dir(tmp.path()).expect("failed to enter temp dir");
    fs::write("solver.toml", "[termination").expect("failed to write invalid solver.toml");

    let err =
        run_set("termination.seconds_spent_limit", "60").expect_err("invalid toml should fail");

    std::env::set_current_dir(original_dir).expect("failed to restore current dir");

    assert!(err.to_string().starts_with("failed to parse solver.toml:"));
}

#[test]
fn run_show_errors_when_solver_toml_is_missing() {
    let _cwd_guard = lock_cwd();
    let tmp = tempdir().expect("failed to create temp dir");
    let original_dir = std::env::current_dir().expect("failed to read current dir");
    std::env::set_current_dir(tmp.path()).expect("failed to enter temp dir");

    let result = run_show();

    std::env::set_current_dir(original_dir).expect("failed to restore current dir");

    match result.expect_err("missing solver.toml should fail") {
        CliError::NotInProject { missing } => assert_eq!(missing, "solver.toml"),
        other => panic!("expected NotInProject, got {}", other),
    }
}

#[test]
fn run_show_succeeds_when_solver_toml_exists() {
    let _cwd_guard = lock_cwd();
    let tmp = tempdir().expect("failed to create temp dir");
    let original_dir = std::env::current_dir().expect("failed to read current dir");
    std::env::set_current_dir(tmp.path()).expect("failed to enter temp dir");
    fs::write("solver.toml", "[termination]\nseconds_spent_limit = 60\n")
        .expect("failed to write solver.toml");

    let result = run_show();

    std::env::set_current_dir(original_dir).expect("failed to restore current dir");

    assert!(
        result.is_ok(),
        "run_show should succeed when solver.toml exists"
    );
}

#[test]
fn run_set_preserves_managed_solver_config_blocks_for_non_phase_keys() {
    let _cwd_guard = lock_cwd();
    let tmp = tempdir().expect("failed to create temp dir");
    let original_dir = std::env::current_dir().expect("failed to read current dir");
    std::env::set_current_dir(tmp.path()).expect("failed to enter temp dir");
    fs::write(
        "solver.toml",
        r#"
[termination]
score_calculation_count_limit = 10

# @solverforge:begin solver-config
# @solverforge:owner scalar-group required_assignment construction
[[phases]]
type = "construction_heuristic"
construction_heuristic_type = "first_fit"
construction_obligation = "assign_when_candidate_exists"
group_name = "required_assignment"
# @solverforge:end solver-config
"#,
    )
    .expect("failed to write solver.toml");

    run_set("termination.seconds_spent_limit", "60").expect("run_set should succeed");

    let saved = fs::read_to_string("solver.toml").expect("failed to read solver.toml");
    std::env::set_current_dir(original_dir).expect("failed to restore current dir");

    assert!(saved.contains("seconds_spent_limit = 60"));
    assert!(saved.contains("# @solverforge:begin solver-config"));
    assert!(saved.contains("# @solverforge:owner scalar-group required_assignment construction"));
    assert!(saved.contains("group_name = \"required_assignment\""));
    assert!(saved.contains("# @solverforge:end solver-config"));
}

#[test]
fn run_set_preserves_interleaved_managed_solver_phase_order_for_non_phase_keys() {
    let _cwd_guard = lock_cwd();
    let tmp = tempdir().expect("failed to create temp dir");
    let original_dir = std::env::current_dir().expect("failed to read current dir");
    std::env::set_current_dir(tmp.path()).expect("failed to enter temp dir");
    fs::write(
        "solver.toml",
        r#"
[[phases]]
type = "construction_heuristic"
construction_heuristic_type = "first_fit"

# @solverforge:begin solver-config
# @solverforge:owner scalar-group required_assignment construction
[[phases]]
type = "construction_heuristic"
construction_heuristic_type = "first_fit"
construction_obligation = "assign_when_candidate_exists"
group_name = "required_assignment"
# @solverforge:owner scalar-group required_assignment search
[[phases]]
type = "local_search"

[phases.move_selector]
type = "grouped_scalar_move_selector"
group_name = "required_assignment"
require_hard_improvement = true
# @solverforge:end solver-config

[[phases]]
type = "local_search"

[phases.acceptor]
type = "late_acceptance"
late_acceptance_size = 400

[termination]
score_calculation_count_limit = 10
"#,
    )
    .expect("failed to write solver.toml");
    let before = fs::read_to_string("solver.toml").expect("failed to read solver.toml");

    run_set("termination.seconds_spent_limit", "60").expect("run_set should succeed");

    let saved = fs::read_to_string("solver.toml").expect("failed to read solver.toml");
    std::env::set_current_dir(original_dir).expect("failed to restore current dir");

    let user_construction = saved
        .find("construction_heuristic_type = \"first_fit\"")
        .expect("user construction should remain");
    let managed_region = saved
        .find("# @solverforge:begin solver-config")
        .expect("managed region should remain");
    let user_search = saved
        .rfind("late_acceptance_size = 400")
        .expect("user local search should remain");
    assert!(user_construction < managed_region);
    assert!(managed_region < user_search);
    assert!(saved.contains("seconds_spent_limit = 60"));
    assert!(before.contains("# @solverforge:begin solver-config"));
}

#[test]
fn run_set_rejects_phase_edits() {
    let _cwd_guard = lock_cwd();
    let tmp = tempdir().expect("failed to create temp dir");
    let original_dir = std::env::current_dir().expect("failed to read current dir");
    std::env::set_current_dir(tmp.path()).expect("failed to enter temp dir");
    fs::write(
        "solver.toml",
        r#"
[[phases]]
type = "local_search"
"#,
    )
    .expect("failed to write solver.toml");
    let before = fs::read_to_string("solver.toml").expect("failed to read solver.toml");

    let err =
        run_set("phases[0].type", "construction_heuristic").expect_err("phase edits should fail");

    assert!(err
        .to_string()
        .contains("cannot edit ordered solver.toml `phases`"));
    assert_eq!(
        fs::read_to_string("solver.toml").expect("failed to read solver.toml"),
        before
    );
    std::env::set_current_dir(original_dir).expect("failed to restore current dir");
}
