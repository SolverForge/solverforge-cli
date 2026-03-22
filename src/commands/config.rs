use std::fs;
use std::path::Path;

use crate::error::{CliError, CliResult};
use crate::output;

const CONFIG_PATH: &str = "solver.toml";

// Print the contents of solver.toml.
pub fn run_show() -> CliResult {
    let path = Path::new(CONFIG_PATH);
    if !path.exists() {
        return Err(CliError::NotInProject {
            missing: "solver.toml",
        });
    }

    let content = fs::read_to_string(path).map_err(|e| CliError::IoError {
        context: "failed to read solver.toml".to_string(),
        source: e,
    })?;

    output::print_status("config", CONFIG_PATH);
    println!();
    println!("{}", content.trim_end());
    println!();

    Ok(())
}

// Set a dotted key path in solver.toml to the given value.
pub fn run_set(key: &str, value: &str) -> CliResult {
    let path = Path::new(CONFIG_PATH);
    if !path.exists() {
        return Err(CliError::NotInProject {
            missing: "solver.toml",
        });
    }

    let content = fs::read_to_string(path).map_err(|e| CliError::IoError {
        context: "failed to read solver.toml".to_string(),
        source: e,
    })?;

    let mut doc: toml::Value = toml::from_str(&content).map_err(|e: toml::de::Error| {
        CliError::general(format!("failed to parse solver.toml: {}", e))
    })?;

    set_toml_key(&mut doc, key, value)?;

    let new_content = toml::to_string_pretty(&doc)
        .map_err(|e| CliError::general(format!("failed to serialize solver.toml: {}", e)))?;

    fs::write(path, &new_content).map_err(|e| CliError::IoError {
        context: "failed to write solver.toml".to_string(),
        source: e,
    })?;

    output::print_update(CONFIG_PATH);
    Ok(())
}

// Navigate and set a dotted key path (e.g. "termination.time_spent_seconds") in a toml::Value.
fn set_toml_key(doc: &mut toml::Value, key: &str, value: &str) -> CliResult {
    let parts: Vec<&str> = key.splitn(2, '.').collect();

    if parts.len() == 1 {
        let leaf = parts[0];
        let table = doc
            .as_table_mut()
            .ok_or_else(|| CliError::general("solver.toml root is not a TOML table"))?;
        table.insert(leaf.to_string(), parse_toml_value(value));
        return Ok(());
    }

    let section = parts[0];
    let rest = parts[1];

    let table = doc
        .as_table_mut()
        .ok_or_else(|| CliError::general("solver.toml root is not a TOML table"))?;

    let child = table
        .entry(section.to_string())
        .or_insert_with(|| toml::Value::Table(toml::map::Map::new()));

    set_toml_key(child, rest, value)
}

// Parse a string into a toml::Value: try integer, then float, then bool, then string.
fn parse_toml_value(s: &str) -> toml::Value {
    if let Ok(i) = s.parse::<i64>() {
        return toml::Value::Integer(i);
    }
    if let Ok(f) = s.parse::<f64>() {
        return toml::Value::Float(f);
    }
    match s {
        "true" => return toml::Value::Boolean(true),
        "false" => return toml::Value::Boolean(false),
        _ => {}
    }
    toml::Value::String(s.to_string())
}

#[cfg(test)]
mod tests {
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
            toml::from_str("[termination]\ntime_spent_seconds = 30\n").expect("valid toml");

        set_toml_key(&mut doc, "name", "demo").expect("top-level set should succeed");

        assert_eq!(doc["name"], toml::Value::String("demo".to_string()));
        assert_eq!(
            doc["termination"]["time_spent_seconds"],
            toml::Value::Integer(30)
        );
    }

    #[test]
    fn set_toml_key_creates_nested_tables_and_preserves_siblings() {
        let mut doc: toml::Value =
            toml::from_str("[termination]\nscore_calculation_count_limit = 10\n")
                .expect("valid toml");

        set_toml_key(&mut doc, "termination.time_spent_seconds", "60")
            .expect("nested set should succeed");

        assert_eq!(
            doc["termination"]["time_spent_seconds"],
            toml::Value::Integer(60)
        );
        assert_eq!(
            doc["termination"]["score_calculation_count_limit"],
            toml::Value::Integer(10)
        );
    }

    #[test]
    fn set_toml_key_errors_when_root_is_not_a_table() {
        let mut doc = toml::Value::String("not-a-table".to_string());

        let err = set_toml_key(&mut doc, "termination.time_spent_seconds", "60")
            .expect_err("scalar root should fail");

        assert_eq!(err.to_string(), "solver.toml root is not a TOML table");
    }

    #[test]
    fn set_toml_key_errors_when_intermediate_value_is_not_a_table() {
        let mut doc: toml::Value = toml::from_str("termination = 5").expect("valid toml");

        let err = set_toml_key(&mut doc, "termination.time_spent_seconds", "60")
            .expect_err("scalar intermediate should fail");

        assert_eq!(err.to_string(), "solver.toml root is not a TOML table");
    }

    #[test]
    fn run_set_errors_when_solver_toml_is_missing() {
        let _cwd_guard = lock_cwd();
        let tmp = tempdir().expect("failed to create temp dir");
        let original_dir = std::env::current_dir().expect("failed to read current dir");
        std::env::set_current_dir(tmp.path()).expect("failed to enter temp dir");

        let result = run_set("termination.time_spent_seconds", "60");

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

        run_set("termination.time_spent_seconds", "60").expect("run_set should succeed");

        let saved = fs::read_to_string("solver.toml").expect("failed to read solver.toml");
        std::env::set_current_dir(original_dir).expect("failed to restore current dir");

        let saved_doc: toml::Value = toml::from_str(&saved).expect("saved toml should be valid");
        assert_eq!(
            saved_doc["termination"]["time_spent_seconds"],
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
            run_set("termination.time_spent_seconds", "60").expect_err("invalid toml should fail");

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
        fs::write("solver.toml", "[termination]\ntime_spent_seconds = 60\n")
            .expect("failed to write solver.toml");

        let result = run_show();

        std::env::set_current_dir(original_dir).expect("failed to restore current dir");

        assert!(
            result.is_ok(),
            "run_show should succeed when solver.toml exists"
        );
    }
}
