use super::{
    mcp_servers_document, merge_vscode_config, resolve_binary, stdio_server_json, to_crate_name,
    vscode_servers_document,
};
use serde_json::{json, Value};

#[test]
fn stdio_entry_runs_the_binary_without_arguments() {
    let entry = stdio_server_json("/tmp/app/target/release/agent_optimizer");
    assert_eq!(entry["command"], "/tmp/app/target/release/agent_optimizer");
    assert_eq!(entry["args"], json!([]));
}

#[test]
fn generic_clients_use_the_mcp_servers_shape() {
    let document =
        mcp_servers_document("agent-optimizer", stdio_server_json("/bin/agent_optimizer"));
    assert_eq!(
        document["mcpServers"]["agent-optimizer"]["command"],
        "/bin/agent_optimizer"
    );
}

#[test]
fn vscode_uses_the_servers_shape() {
    let document =
        vscode_servers_document("agent-optimizer", stdio_server_json("/bin/agent_optimizer"));
    assert_eq!(
        document["servers"]["agent-optimizer"]["command"],
        "/bin/agent_optimizer"
    );
    assert!(document.get("mcpServers").is_none());
}

#[test]
fn merge_preserves_other_vscode_servers_and_adds_the_project_entry() {
    let existing = r#"{
      "servers": {
        "other-tool": { "command": "/bin/other" }
      }
    }"#;

    let merged = merge_vscode_config(Some(existing), "agent-optimizer", "/bin/agent_optimizer")
        .expect("merge succeeds");

    let parsed: Value = serde_json::from_str(&merged).expect("valid json");
    assert_eq!(parsed["servers"]["other-tool"]["command"], "/bin/other");
    assert_eq!(
        parsed["servers"]["agent-optimizer"]["command"],
        "/bin/agent_optimizer"
    );
    assert_eq!(parsed["servers"]["agent-optimizer"]["args"], json!([]));
    assert!(parsed["servers"]["agent-optimizer"].get("url").is_none());
    assert!(merged.ends_with('\n'));
}

#[test]
fn merge_creates_the_document_when_absent() {
    let merged = merge_vscode_config(None, "app", "/bin/app").expect("merge succeeds");
    let parsed: Value = serde_json::from_str(&merged).expect("valid json");
    assert_eq!(parsed["servers"]["app"]["command"], "/bin/app");
}

#[test]
fn merge_refuses_invalid_existing_json_instead_of_clobbering() {
    let error = merge_vscode_config(Some("not json"), "app", "/bin/app")
        .err()
        .expect("invalid JSON must be rejected");
    assert!(
        error.to_string().contains("not valid JSON"),
        "unexpected error: {error}"
    );
}

#[test]
fn merge_refuses_non_object_servers_entry() {
    let error = merge_vscode_config(Some(r#"{"servers": "nope"}"#), "app", "/bin/app")
        .err()
        .expect("non-object servers must be rejected");
    assert!(
        error.to_string().contains("non-object `servers`"),
        "unexpected error: {error}"
    );
}

#[test]
fn resolve_binary_prefers_release_and_reports_build_state() {
    let temp = tempfile::tempdir().expect("temp dir");
    let root = temp.path();

    let (path, built) = resolve_binary(root, "agent_optimizer");
    assert!(!built, "no binary should be found yet");
    assert!(path.ends_with("target/release/agent_optimizer"));

    let release_dir = root.join("target/release");
    std::fs::create_dir_all(&release_dir).expect("create release dir");
    std::fs::write(release_dir.join("agent_optimizer"), b"bin").expect("write binary");

    let (path, built) = resolve_binary(root, "agent_optimizer");
    assert!(built, "release binary must be detected");
    assert!(path.ends_with("target/release/agent_optimizer"));

    std::fs::remove_file(release_dir.join("agent_optimizer")).expect("remove release binary");
    let debug_dir = root.join("target/debug");
    std::fs::create_dir_all(&debug_dir).expect("create debug dir");
    std::fs::write(debug_dir.join("agent_optimizer"), b"bin").expect("write debug binary");

    let (path, built) = resolve_binary(root, "agent_optimizer");
    assert!(built, "debug binary must be detected as a fallback");
    assert!(path.ends_with("target/debug/agent_optimizer"));
}

#[test]
fn crate_name_matches_generated_binary_naming() {
    assert_eq!(to_crate_name("agent-optimizer"), "agent_optimizer");
    assert_eq!(to_crate_name("My-App"), "my_app");
}
