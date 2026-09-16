use super::{
    mcp_servers_document, merge_server_config, opencode_document, opencode_local_server_json,
    resolve_binary, stdio_server_json, to_crate_name, vscode_servers_document,
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

    let merged = merge_server_config(
        Some(existing),
        "servers",
        "agent-optimizer",
        stdio_server_json("/bin/agent_optimizer"),
        None,
    )
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
    let merged = merge_server_config(None, "servers", "app", stdio_server_json("/bin/app"), None)
        .expect("merge succeeds");
    let parsed: Value = serde_json::from_str(&merged).expect("valid json");
    assert_eq!(parsed["servers"]["app"]["command"], "/bin/app");
}

#[test]
fn merge_refuses_invalid_existing_jsonc_instead_of_clobbering() {
    let error = merge_server_config(
        Some("{ \"servers\": "),
        "servers",
        "app",
        stdio_server_json("/bin/app"),
        None,
    )
    .expect_err("invalid JSONC must be rejected");
    assert!(
        error.to_string().contains("not valid JSONC"),
        "unexpected error: {error}"
    );
}

#[test]
fn merge_refuses_non_object_root() {
    let error = merge_server_config(
        Some("[1, 2, 3]"),
        "servers",
        "app",
        stdio_server_json("/bin/app"),
        None,
    )
    .expect_err("a non-object root must be rejected");
    assert!(
        error.to_string().contains("not a JSON object"),
        "unexpected error: {error}"
    );
}

#[test]
fn merge_refuses_non_object_servers_entry() {
    let error = merge_server_config(
        Some(r#"{"servers": "nope"}"#),
        "servers",
        "app",
        stdio_server_json("/bin/app"),
        None,
    )
    .expect_err("non-object servers must be rejected");
    assert!(
        error.to_string().contains("non-object `servers`"),
        "unexpected error: {error}"
    );
}

#[test]
fn merge_refuses_duplicate_relevant_keys() {
    let error = merge_server_config(
        Some(
            r#"{
                "servers": { "app": { "command": "/bin/one" } },
                "servers": { "other": { "command": "/bin/other" } }
            }"#,
        ),
        "servers",
        "app",
        stdio_server_json("/bin/app"),
        None,
    )
    .expect_err("a duplicated root key must be rejected");
    assert!(
        error.to_string().contains("repeats the `servers` key"),
        "unexpected error: {error}"
    );

    let error = merge_server_config(
        Some(
            r#"{ "servers": {
                "app": { "command": "/bin/one" },
                "app": { "command": "/bin/two" }
            } }"#,
        ),
        "servers",
        "app",
        stdio_server_json("/bin/app"),
        None,
    )
    .expect_err("a duplicated server key must be rejected");
    assert!(
        error
            .to_string()
            .contains("repeats the `app` key inside `servers`"),
        "unexpected error: {error}"
    );
}

#[test]
fn merge_preserves_comments_trailing_commas_and_unrelated_members() {
    let existing = concat!(
        "{\n",
        "  // the team's shared editor servers\n",
        "  \"servers\": {\n",
        "    // keep this one\n",
        "    \"other-tool\": { \"command\": \"/bin/other\", },\n",
        "  },\n",
        "  \"editor.formatOnSave\": true,\n",
        "}"
    );

    let merged = merge_server_config(
        Some(existing),
        "servers",
        "agent-optimizer",
        stdio_server_json("/bin/agent_optimizer"),
        None,
    )
    .expect("merge succeeds");

    assert!(
        merged.contains("// the team's shared editor servers")
            && merged.contains("// keep this one"),
        "comments must survive: {merged}"
    );
    assert!(
        merged.contains("\"other-tool\": { \"command\": \"/bin/other\", },"),
        "untouched members keep their exact bytes: {merged}"
    );
    assert!(
        merged.contains("\"editor.formatOnSave\": true"),
        "unrelated root members survive: {merged}"
    );
    let parsed: Value = jsonc_parser::parse_to_serde_value::<Value>(&merged, &Default::default())
        .expect("still parseable as JSONC");
    assert_eq!(
        parsed["servers"]["agent-optimizer"]["command"],
        "/bin/agent_optimizer"
    );
}

#[test]
fn merge_preserves_crlf_newlines() {
    let existing =
        "{\r\n  \"servers\": {\r\n    \"other\": { \"command\": \"/bin/other\" }\r\n  }\r\n}\r\n";

    let merged = merge_server_config(
        Some(existing),
        "servers",
        "app",
        stdio_server_json("/bin/app"),
        None,
    )
    .expect("merge succeeds");

    assert!(
        merged.contains("\r\n"),
        "CRLF documents must stay CRLF: {merged:?}"
    );
    assert!(
        !merged.replace("\r\n", "").contains('\n'),
        "no bare LF may be introduced into a CRLF document: {merged:?}"
    );
    let parsed: Value = serde_json::from_str(&merged).expect("valid json");
    assert_eq!(parsed["servers"]["app"]["command"], "/bin/app");
}

#[test]
fn merge_replaces_an_existing_solverforge_entry_in_place() {
    let existing = r#"{
  "servers": {
    "app": { "command": "/stale/path/app" }
  }
}"#;

    let merged = merge_server_config(
        Some(existing),
        "servers",
        "app",
        stdio_server_json("/bin/app"),
        None,
    )
    .expect("merge succeeds");

    let parsed: Value = serde_json::from_str(&merged).expect("valid json");
    assert_eq!(parsed["servers"]["app"]["command"], "/bin/app");
    assert!(
        !merged.contains("/stale/path/app"),
        "the stale entry must be replaced: {merged}"
    );
}

#[test]
fn merge_is_idempotent() {
    let once = merge_server_config(
        Some("{\n  \"servers\": {\n    \"other\": { \"command\": \"/bin/other\" }\n  }\n}"),
        "servers",
        "app",
        stdio_server_json("/bin/app"),
        None,
    )
    .expect("first merge succeeds");
    let twice = merge_server_config(
        Some(&once),
        "servers",
        "app",
        stdio_server_json("/bin/app"),
        None,
    )
    .expect("second merge succeeds");
    assert_eq!(once, twice, "re-running the merge must not change the file");
}

#[test]
fn resolve_binary_prefers_release_and_reports_build_state() {
    let temp = tempfile::tempdir().expect("temp dir");
    let root = temp.path();
    let executable = format!("agent_optimizer{}", std::env::consts::EXE_SUFFIX);

    let (path, built) = resolve_binary(root, "agent_optimizer");
    assert!(!built, "no binary should be found yet");
    assert!(std::path::Path::new(&path)
        .ends_with(std::path::Path::new("target/release").join(&executable)));

    let release_dir = root.join("target/release");
    std::fs::create_dir_all(&release_dir).expect("create release dir");
    std::fs::write(release_dir.join(&executable), b"bin").expect("write binary");

    let (path, built) = resolve_binary(root, "agent_optimizer");
    assert!(built, "release binary must be detected");
    assert!(std::path::Path::new(&path)
        .ends_with(std::path::Path::new("target/release").join(&executable)));

    std::fs::remove_file(release_dir.join(&executable)).expect("remove release binary");
    let debug_dir = root.join("target/debug");
    std::fs::create_dir_all(&debug_dir).expect("create debug dir");
    std::fs::write(debug_dir.join(&executable), b"bin").expect("write debug binary");

    let (path, built) = resolve_binary(root, "agent_optimizer");
    assert!(built, "debug binary must be detected as a fallback");
    assert!(std::path::Path::new(&path)
        .ends_with(std::path::Path::new("target/debug").join(executable)));
}

#[test]
fn crate_name_matches_generated_binary_naming() {
    assert_eq!(to_crate_name("agent-optimizer"), "agent_optimizer");
    assert_eq!(to_crate_name("My-App"), "my_app");
}

#[test]
fn opencode_entry_uses_the_local_mcp_shape() {
    let entry = opencode_local_server_json("/bin/agent_optimizer");
    assert_eq!(entry["type"], "local");
    assert_eq!(entry["command"], json!(["/bin/agent_optimizer"]));
    assert_eq!(entry["enabled"], json!(true));
}

#[test]
fn opencode_document_uses_the_mcp_root_and_schema() {
    let document = opencode_document("agent-optimizer", "/bin/agent_optimizer");
    assert_eq!(document["$schema"], "https://opencode.ai/config.json");
    assert_eq!(document["mcp"]["agent-optimizer"]["type"], "local");
    assert!(document.get("mcpServers").is_none());
}

#[test]
fn merge_supports_the_mcp_servers_root_and_preserves_other_keys() {
    let existing = r#"{"mcpServers":{"other":{"command":"/bin/other"}},"extra":1}"#;
    let merged = merge_server_config(
        Some(existing),
        "mcpServers",
        "agent-optimizer",
        stdio_server_json("/bin/agent_optimizer"),
        None,
    )
    .expect("merge succeeds");

    let parsed: Value = serde_json::from_str(&merged).expect("valid json");
    assert_eq!(parsed["mcpServers"]["other"]["command"], "/bin/other");
    assert_eq!(
        parsed["mcpServers"]["agent-optimizer"]["command"],
        "/bin/agent_optimizer"
    );
    assert_eq!(parsed["extra"], json!(1));
}

#[test]
fn merge_adds_schema_for_opencode_and_preserves_an_existing_one() {
    let created = merge_server_config(
        None,
        "mcp",
        "app",
        opencode_local_server_json("/bin/app"),
        Some("https://opencode.ai/config.json"),
    )
    .expect("merge succeeds");
    let parsed: Value = serde_json::from_str(&created).expect("valid json");
    assert_eq!(parsed["$schema"], "https://opencode.ai/config.json");

    let existing = r#"{"$schema":"https://example.com/schema.json","mcp":{}}"#;
    let merged = merge_server_config(
        Some(existing),
        "mcp",
        "app",
        opencode_local_server_json("/bin/app"),
        Some("https://opencode.ai/config.json"),
    )
    .expect("merge succeeds");
    let parsed: Value = serde_json::from_str(&merged).expect("valid json");
    assert_eq!(parsed["$schema"], "https://example.com/schema.json");
}
