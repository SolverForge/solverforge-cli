use super::*;
use crate::test_support;

#[test]
fn preserves_arbitrary_view_shape_when_updating_constraints() {
    let guard = test_support::lock_cwd();

    let tmp = tempfile::tempdir().expect("failed to create temp dir");
    let original_dir = std::env::current_dir().expect("failed to get current dir");

    std::fs::create_dir_all(tmp.path().join("static")).expect("failed to create static dir");
    std::fs::write(
        tmp.path().join("static").join("sf-config.json"),
        r#"{
  "title": "demo",
  "subtitle": "demo",
  "constraints": ["all_assigned"],
  "view": {
    "type": "assignment_board",
    "columns": [{"id": "todo"}],
    "meta": {"accent": "red"}
  }
}"#,
    )
    .expect("failed to write sf-config");

    std::env::set_current_dir(tmp.path()).expect("failed to enter temp dir");
    add_constraint("capacity_limit").expect("add_constraint should succeed");
    let saved = std::fs::read_to_string(tmp.path().join("static").join("sf-config.json"))
        .expect("failed to read saved sf-config");
    std::env::set_current_dir(original_dir).expect("failed to restore current dir");
    drop(guard);

    assert!(saved.contains("\"capacity_limit\""));
    assert!(saved.contains("\"assignment_board\""));
    assert!(saved.contains("\"columns\""));
    assert!(saved.contains("\"accent\": \"red\""));
}

#[test]
fn preserves_unknown_top_level_keys_when_updating_constraints() {
    let guard = test_support::lock_cwd();

    let tmp = tempfile::tempdir().expect("failed to create temp dir");
    let original_dir = std::env::current_dir().expect("failed to get current dir");

    std::fs::create_dir_all(tmp.path().join("static")).expect("failed to create static dir");
    std::fs::write(
        tmp.path().join("static").join("sf-config.json"),
        r#"{
  "title": "demo",
  "subtitle": "demo",
  "constraints": ["all_assigned"],
  "theme": {"accent": "red"},
  "layout": "rail"
}"#,
    )
    .expect("failed to write sf-config");

    std::env::set_current_dir(tmp.path()).expect("failed to enter temp dir");
    add_constraint("capacity_limit").expect("add_constraint should succeed");
    let saved = std::fs::read_to_string(tmp.path().join("static").join("sf-config.json"))
        .expect("failed to read saved sf-config");
    std::env::set_current_dir(original_dir).expect("failed to restore current dir");
    drop(guard);

    assert!(saved.contains("\"capacity_limit\""));
    assert!(saved.contains("\"theme\""));
    assert!(saved.contains("\"layout\": \"rail\""));
}
