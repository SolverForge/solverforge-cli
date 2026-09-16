use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use jsonc_parser::cst::{CstInputValue, CstObject, CstRootNode};
use jsonc_parser::ParseOptions;
use serde_json::{json, Map, Value};

use crate::app_spec;
use crate::error::{CliError, CliResult};
use crate::output;

/// In-project configuration targets that `solverforge connect --write` can
/// create. Global client configuration files are printed instead of written.
#[derive(Clone, Copy, Debug, Eq, PartialEq, clap::ValueEnum)]
pub enum ConnectWriteTarget {
    Vscode,
    Cursor,
    Claude,
    Opencode,
}

/// Renders the MCP client connection surface for the current project.
///
/// The command prints ready-to-paste configurations for the common agent
/// harnesses. It only writes in-project configuration files (`.vscode/mcp.json`,
/// `.cursor/mcp.json`, `.mcp.json`, `opencode.json`); global client
/// configuration files are printed with their exact path so the user stays in
/// control of machine-wide state.
pub fn run(port: u16, write: Option<ConnectWriteTarget>) -> CliResult {
    let spec = load_mcp_spec()?;

    let project_root = std::env::current_dir().map_err(|e| CliError::IoError {
        context: "failed to read the current directory".to_string(),
        source: e,
    })?;
    let crate_name = to_crate_name(&spec.app.name);
    let (binary, built) = resolve_binary(&project_root, &crate_name);
    let url = format!("http://127.0.0.1:{port}/mcp");

    output::print_heading(&format!("MCP connection for '{}'", spec.app.name));
    println!();

    if !built {
        output::print_dim(&format!(
            "  Note: {} does not exist yet; run `cargo build --release` first.",
            binary
        ));
        println!();
    }

    print_stdio_section(&spec.app.name, &binary);
    print_http_section(&spec.app.name, &url);

    match write {
        Some(target) => {
            let (path, label) = write_target(&project_root, target, &spec.app.name, &binary)?;
            println!();
            output::print_create(&path.display().to_string());
            output::print_success(&format!("  {label} MCP configuration written"));
        }
        None => {
            println!();
            output::print_dim(
                "  Write an in-project client config with `solverforge connect --write vscode|cursor|claude|opencode`.",
            );
        }
    }

    Ok(())
}

fn load_mcp_spec() -> CliResult<app_spec::AppSpec> {
    if !Path::new("solverforge.app.toml").exists() {
        return Err(CliError::NotInProject {
            missing: "solverforge.app.toml",
        });
    }

    let spec = app_spec::load()?;
    if spec.app.shell != "mcp" {
        return Err(CliError::with_hint(
            format!(
                "`solverforge connect` is only available for MCP-shell projects (this project's shell is `{}`)",
                spec.app.shell
            ),
            "scaffold an MCP project with `solverforge new <name> --shell mcp`",
        ));
    }

    Ok(spec)
}

/// Commands that a harness runs to speak MCP over stdio.
fn stdio_command(binary: &str) -> String {
    binary.to_string()
}

/// The stdio registration block shared by Claude Desktop, Cursor, and other
/// `mcpServers`-style clients.
fn stdio_server_json(binary: &str) -> Value {
    json!({
        "command": stdio_command(binary),
        "args": [],
    })
}

/// The HTTP registration block used by clients that support remote MCP.
fn http_server_json(url: &str) -> Value {
    json!({
        "type": "http",
        "url": url,
    })
}

fn mcp_servers_document(name: &str, entry: Value) -> Value {
    let mut servers = Map::new();
    servers.insert(name.to_string(), entry);
    let mut root = Map::new();
    root.insert("mcpServers".to_string(), Value::Object(servers));
    Value::Object(root)
}

fn vscode_servers_document(name: &str, entry: Value) -> Value {
    let mut servers = Map::new();
    servers.insert(name.to_string(), entry);
    let mut root = Map::new();
    root.insert("servers".to_string(), Value::Object(servers));
    Value::Object(root)
}

fn opencode_document(name: &str, binary: &str) -> Value {
    let mut servers = Map::new();
    servers.insert(name.to_string(), opencode_local_server_json(binary));
    let mut root = Map::new();
    root.insert(
        "$schema".to_string(),
        Value::String("https://opencode.ai/config.json".to_string()),
    );
    root.insert("mcp".to_string(), Value::Object(servers));
    Value::Object(root)
}

fn print_json_block(value: &Value) {
    match serde_json::to_string_pretty(value) {
        Ok(rendered) => {
            for line in rendered.lines() {
                println!("    {line}");
            }
        }
        Err(error) => output::print_error(&format!("failed to render JSON: {error}")),
    }
}

fn print_stdio_section(name: &str, binary: &str) {
    println!("  stdio (default transport)");
    println!();
    println!("    Claude Code:");
    println!("      claude mcp add {name} -- {binary}");
    println!();
    println!("    Claude Desktop (claude_desktop_config.json):");
    println!("      macOS:   ~/Library/Application Support/Claude/claude_desktop_config.json");
    println!("      Linux:   ~/.config/Claude/claude_desktop_config.json");
    println!("      Windows: %APPDATA%\\Claude\\claude_desktop_config.json");
    print_json_block(&mcp_servers_document(name, stdio_server_json(binary)));
    println!();
    println!("    Cursor (.cursor/mcp.json):");
    print_json_block(&mcp_servers_document(name, stdio_server_json(binary)));
    println!();
    println!("    opencode (opencode.json):");
    print_json_block(&opencode_document(name, binary));
    println!();
    println!("    VS Code (.vscode/mcp.json):");
    print_json_block(&vscode_servers_document(name, stdio_server_json(binary)));
    println!();
    println!("    Any other MCP client: run `{binary}` and speak JSON-RPC over stdio.");
}

fn print_http_section(name: &str, url: &str) {
    println!();
    println!("  Streamable HTTP (start with `cargo run --release -- --http`)");
    println!();
    println!("    Claude Code:");
    println!("      claude mcp add --transport http {name} {url}");
    println!();
    println!("    VS Code (.vscode/mcp.json):");
    print_json_block(&vscode_servers_document(name, http_server_json(url)));
    println!();
    println!("    Any other MCP client: connect to {url}");
}

/// Writes the in-project client config for one harness target.
fn write_target(
    project_root: &Path,
    target: ConnectWriteTarget,
    name: &str,
    binary: &str,
) -> CliResult<(PathBuf, &'static str)> {
    let (relative, root_key, entry, schema, label) = match target {
        ConnectWriteTarget::Vscode => (
            ".vscode/mcp.json",
            "servers",
            stdio_server_json(binary),
            None,
            "VS Code",
        ),
        ConnectWriteTarget::Cursor => (
            ".cursor/mcp.json",
            "mcpServers",
            stdio_server_json(binary),
            None,
            "Cursor",
        ),
        ConnectWriteTarget::Claude => (
            ".mcp.json",
            "mcpServers",
            stdio_server_json(binary),
            None,
            "Claude Code",
        ),
        ConnectWriteTarget::Opencode => (
            "opencode.json",
            "mcp",
            opencode_local_server_json(binary),
            Some("https://opencode.ai/config.json"),
            "opencode",
        ),
    };

    let path = write_server_config(project_root, relative, root_key, name, entry, schema)?;
    Ok((path, label))
}

/// The opencode local-server entry shape (`mcp` map, `type: "local"`).
fn opencode_local_server_json(binary: &str) -> Value {
    json!({
        "type": "local",
        "command": [binary],
        "enabled": true,
    })
}

fn write_server_config(
    project_root: &Path,
    relative: &str,
    root_key: &str,
    name: &str,
    entry: Value,
    schema: Option<&str>,
) -> CliResult<PathBuf> {
    let path = project_root.join(relative);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| CliError::IoError {
            context: format!("failed to create {}", parent.display()),
            source: e,
        })?;
    }

    if let Ok(metadata) = fs::symlink_metadata(&path) {
        if metadata.file_type().is_symlink() {
            return Err(CliError::with_hint(
                format!(
                    "{} is a symlink; the client config would be written outside this project",
                    path.display()
                ),
                "remove the symlink or point the client at the real file, then run `solverforge connect --write` again",
            ));
        }
        if !metadata.is_file() {
            return Err(CliError::with_hint(
                format!("{} is not a regular file", path.display()),
                "remove or rename it, then run `solverforge connect --write` again",
            ));
        }
    }

    let existing = if path.exists() {
        Some(fs::read_to_string(&path).map_err(|e| CliError::IoError {
            context: format!("failed to read {}", path.display()),
            source: e,
        })?)
    } else {
        None
    };

    let rendered = merge_server_config(existing.as_deref(), root_key, name, entry, schema)?;
    replace_file_atomically(&path, &rendered, existing.as_deref())?;
    Ok(path)
}

/// Replaces `path` through a temporary sibling plus rename so a failed or
/// interrupted write can never truncate the existing client config. Aborts
/// without writing when the file changed since it was read.
fn replace_file_atomically(path: &Path, rendered: &str, original: Option<&str>) -> CliResult<()> {
    let directory = path.parent().unwrap_or_else(|| Path::new("."));
    let mut temporary = tempfile::Builder::new()
        .prefix(".solverforge-connect-")
        .suffix(".tmp")
        .tempfile_in(directory)
        .map_err(|e| CliError::IoError {
            context: format!(
                "failed to create a temporary file next to {}",
                path.display()
            ),
            source: e,
        })?;
    temporary
        .write_all(rendered.as_bytes())
        .and_then(|()| temporary.flush())
        .and_then(|()| temporary.as_file().sync_all())
        .map_err(|e| CliError::IoError {
            context: format!("failed to write the temporary file for {}", path.display()),
            source: e,
        })?;

    if let Some(original) = original {
        let current = fs::read_to_string(path).map_err(|e| CliError::IoError {
            context: format!("failed to re-read {}", path.display()),
            source: e,
        })?;
        if current != original {
            return Err(CliError::with_hint(
                format!(
                    "{} changed while the update was prepared; nothing was written",
                    path.display()
                ),
                "review the current file and run `solverforge connect --write` again",
            ));
        }
    }

    #[cfg(unix)]
    if original.is_some() {
        if let Ok(metadata) = fs::metadata(path) {
            fs::set_permissions(temporary.path(), metadata.permissions()).map_err(|e| {
                CliError::IoError {
                    context: format!("failed to carry over the permissions of {}", path.display()),
                    source: e,
                }
            })?;
        }
    }

    temporary.persist(path).map_err(|e| {
        CliError::general(format!(
            "failed to replace {} with the updated config: {}",
            path.display(),
            e
        ))
    })?;
    Ok(())
}

/// Merges a server entry into an existing JSON/JSONC document under `root_key`
/// while keeping the author's comments, trailing commas, indentation, newline
/// style, and every unrelated member byte-for-byte. When no document exists a
/// minimal pretty-printed one is created. Input that cannot be parsed, has a
/// non-object root or server map, or repeats the relevant keys is rejected
/// with the original left untouched.
fn merge_server_config(
    existing: Option<&str>,
    root_key: &str,
    name: &str,
    entry: Value,
    schema: Option<&str>,
) -> CliResult<String> {
    let text = existing.unwrap_or("");
    let root = CstRootNode::parse(text, &ParseOptions::default()).map_err(|error| {
        CliError::with_hint(
            format!("existing config is not valid JSONC: {error}"),
            "fix or remove the client config file and run `solverforge connect --write` again",
        )
    })?;

    let root_object =
        match root.object_value() {
            Some(object) => object,
            None if root.value().is_none() => root
                .object_value_or_create()
                .expect("an empty document becomes an object"),
            None => return Err(CliError::with_hint(
                "existing client config is not a JSON object",
                "fix or remove the client config file and run `solverforge connect --write` again",
            )),
        };
    reject_duplicate_keys(&root_object, root_key, root_key)?;

    if let Some(schema) = schema {
        if root_object.get("$schema").is_none() {
            root_object.insert(0, "$schema", CstInputValue::String(schema.to_string()));
        }
    }

    let servers =
        match root_object.object_value(root_key) {
            Some(servers) => servers,
            None if root_object.get(root_key).is_none() => root_object
                .object_value_or_create(root_key)
                .expect("a just-created member is an object"),
            None => return Err(CliError::with_hint(
                format!("existing client config has a non-object `{root_key}` entry"),
                "fix or remove the client config file and run `solverforge connect --write` again",
            )),
        };
    reject_duplicate_keys(&servers, name, root_key)?;

    match servers.get(name) {
        Some(member) => member.set_value(cst_input(&entry)),
        None => {
            servers.append(name, cst_input(&entry));
        }
    }

    let mut rendered = root.to_string();
    if !rendered.ends_with('\n') {
        rendered.push('\n');
    }
    Ok(rendered)
}

/// A repeated key makes the merge target ambiguous, so the document is
/// rejected instead of guessing which member the author meant.
fn reject_duplicate_keys(object: &CstObject, key: &str, container_key: &str) -> CliResult<()> {
    let occurrences = object
        .properties()
        .iter()
        .filter(|member| member.decoded_name().as_deref() == Some(key))
        .count();
    if occurrences > 1 {
        return Err(CliError::with_hint(
            format!(
                "existing client config repeats the `{key}` key{in_container}",
                in_container = if container_key == key {
                    String::new()
                } else {
                    format!(" inside `{container_key}`")
                }
            ),
            "fix or remove the client config file and run `solverforge connect --write` again",
        ));
    }
    Ok(())
}

fn cst_input(value: &Value) -> CstInputValue {
    match value {
        Value::Null => CstInputValue::Null,
        Value::Bool(inner) => CstInputValue::Bool(*inner),
        Value::Number(inner) => CstInputValue::Number(inner.to_string()),
        Value::String(inner) => CstInputValue::String(inner.clone()),
        Value::Array(inner) => CstInputValue::Array(inner.iter().map(cst_input).collect()),
        Value::Object(inner) => CstInputValue::Object(
            inner
                .iter()
                .map(|(key, value)| (key.clone(), cst_input(value)))
                .collect(),
        ),
    }
}

#[cfg(test)]
mod write_tests {
    use super::{replace_file_atomically, write_server_config};
    use serde_json::json;
    use std::fs;
    use std::path::Path;

    fn sample_entry() -> serde_json::Value {
        json!({ "command": "/bin/app", "args": [] })
    }

    #[test]
    fn write_creates_missing_config_directories_and_file() {
        let temp = tempfile::tempdir().expect("temp dir");
        let path = write_server_config(
            temp.path(),
            ".vscode/mcp.json",
            "servers",
            "app",
            sample_entry(),
            None,
        )
        .expect("write succeeds");
        assert_eq!(path, temp.path().join(".vscode/mcp.json"));
        let contents = fs::read_to_string(&path).expect("written file");
        assert!(contents.contains("\"servers\"") && contents.contains("\"app\""));
        assert!(contents.ends_with('\n'));
    }

    #[test]
    fn write_refuses_a_symlinked_config_without_touching_either_file() {
        let temp = tempfile::tempdir().expect("temp dir");
        let real = temp.path().join("real-config.json");
        fs::write(&real, "{\n}\n").expect("seed real file");
        let link = temp.path().join(".mcp.json");
        make_symlink(&real, &link);

        let error = write_server_config(
            temp.path(),
            ".mcp.json",
            "mcpServers",
            "app",
            sample_entry(),
            None,
        )
        .expect_err("symlinked config must be refused");
        assert!(
            error.to_string().contains("symlink"),
            "unexpected error: {error}"
        );
        assert_eq!(
            fs::read_to_string(&real).expect("real file untouched"),
            "{\n}\n"
        );
        assert_eq!(
            fs::read_link(&link).expect("link untouched"),
            real,
            "the symlink itself must stay in place"
        );
    }

    #[test]
    fn write_refuses_a_non_regular_config_target() {
        let temp = tempfile::tempdir().expect("temp dir");
        let path = temp.path().join(".mcp.json");
        fs::create_dir_all(&path).expect("plant a directory");

        let error = write_server_config(
            temp.path(),
            ".mcp.json",
            "mcpServers",
            "app",
            sample_entry(),
            None,
        )
        .expect_err("a directory must be refused");
        assert!(
            error.to_string().contains("not a regular file"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn atomic_replace_aborts_when_the_file_changed_since_read() {
        let temp = tempfile::tempdir().expect("temp dir");
        let path = temp.path().join(".mcp.json");
        fs::write(&path, "changed by someone else").expect("seed file");

        let error = replace_file_atomically(&path, "rendered", Some("read earlier"))
            .expect_err("a concurrent change must abort the write");
        assert!(
            error.to_string().contains("changed while"),
            "unexpected error: {error}"
        );
        assert_eq!(
            fs::read_to_string(&path).expect("file preserved"),
            "changed by someone else"
        );
        let leftovers: Vec<_> = fs::read_dir(temp.path())
            .expect("read dir")
            .collect::<Result<_, _>>()
            .expect("entries");
        assert_eq!(leftovers.len(), 1, "no temporary files may linger");
    }

    #[test]
    fn atomic_replace_writes_and_carries_permissions() {
        let temp = tempfile::tempdir().expect("temp dir");
        let path = temp.path().join(".mcp.json");
        fs::write(&path, "old").expect("seed file");

        replace_file_atomically(&path, "new contents", Some("old")).expect("replace succeeds");
        assert_eq!(fs::read_to_string(&path).expect("replaced"), "new contents");

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).expect("restrict");
            replace_file_atomically(&path, "newer contents", Some("new contents"))
                .expect("second replace succeeds");
            let mode = fs::metadata(&path).expect("meta").permissions().mode();
            assert_eq!(mode & 0o777, 0o600, "permissions must carry over");
        }
    }

    fn make_symlink(target: &Path, link: &Path) {
        #[cfg(unix)]
        std::os::unix::fs::symlink(target, link).expect("create symlink");
        #[cfg(windows)]
        std::os::windows::fs::symlink_file(target, link).expect("create symlink");
    }
}

fn resolve_binary(project_root: &Path, crate_name: &str) -> (String, bool) {
    let executable = format!("{crate_name}{}", std::env::consts::EXE_SUFFIX);
    for profile in ["release", "debug"] {
        let candidate = project_root.join("target").join(profile).join(&executable);
        if candidate.exists() {
            return (candidate.display().to_string(), true);
        }
    }

    (
        project_root
            .join("target")
            .join("release")
            .join(executable)
            .display()
            .to_string(),
        false,
    )
}

/// Converts a project name to its generated binary name (underscores, lowercase).
fn to_crate_name(name: &str) -> String {
    name.chars()
        .map(|c| {
            if c == '-' {
                '_'
            } else {
                c.to_ascii_lowercase()
            }
        })
        .collect()
}

#[cfg(test)]
#[path = "connect_tests.rs"]
mod tests;
