use std::fs;
use std::path::{Path, PathBuf};

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
/// harnesses. It only writes in-project configuration (`.vscode/mcp.json`);
/// global client configuration files are printed with their exact path so the
/// user stays in control of machine-wide state.
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

    let existing = if path.exists() {
        Some(fs::read_to_string(&path).map_err(|e| CliError::IoError {
            context: format!("failed to read {}", path.display()),
            source: e,
        })?)
    } else {
        None
    };

    let rendered = merge_server_config(existing.as_deref(), root_key, name, entry, schema)?;
    fs::write(&path, rendered).map_err(|e| CliError::IoError {
        context: format!("failed to write {}", path.display()),
        source: e,
    })?;
    Ok(path)
}

/// Merges a server entry into an existing JSON document under `root_key`,
/// preserving every other server and key. A document that cannot be parsed as
/// JSON is left untouched so user configuration is never clobbered.
fn merge_server_config(
    existing: Option<&str>,
    root_key: &str,
    name: &str,
    entry: Value,
    schema: Option<&str>,
) -> CliResult<String> {
    let mut root = match existing {
        Some(contents) if !contents.trim().is_empty() => {
            let parsed: Value = serde_json::from_str(contents).map_err(|error| {
                CliError::with_hint(
                    format!("existing config is not valid JSON: {error}"),
                    "fix or remove the client config file and run `solverforge connect --write` again",
                )
            })?;
            let Value::Object(map) = parsed else {
                return Err(CliError::with_hint(
                    "existing client config is not a JSON object",
                    "fix or remove the client config file and run `solverforge connect --write` again",
                ));
            };
            map
        }
        _ => Map::new(),
    };

    if let Some(schema) = schema {
        root.entry("$schema".to_string())
            .or_insert_with(|| Value::String(schema.to_string()));
    }

    let servers = root
        .entry(root_key.to_string())
        .or_insert_with(|| Value::Object(Map::new()));
    let Value::Object(servers) = servers else {
        return Err(CliError::with_hint(
            format!("existing client config has a non-object `{root_key}` entry"),
            "fix or remove the client config file and run `solverforge connect --write` again",
        ));
    };

    servers.insert(name.to_string(), entry);

    serde_json::to_string_pretty(&Value::Object(root))
        .map(|mut rendered| {
            rendered.push('\n');
            rendered
        })
        .map_err(|error| CliError::general(format!("failed to render client config: {error}")))
}

/// Backwards-compatible wrapper used by the connect tests.
#[cfg(test)]
fn merge_vscode_config(existing: Option<&str>, name: &str, binary: &str) -> CliResult<String> {
    merge_server_config(existing, "servers", name, stdio_server_json(binary), None)
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
