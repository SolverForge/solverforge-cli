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
        Some(ConnectWriteTarget::Vscode) => {
            let path = write_vscode_config(&project_root, &spec.app.name, &binary)?;
            println!();
            output::print_create(&path.display().to_string());
            output::print_success("  VS Code MCP configuration written");
        }
        None => {
            println!();
            output::print_dim(
                "  Write the in-project VS Code config with `solverforge connect --write vscode`.",
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

fn write_vscode_config(project_root: &Path, name: &str, binary: &str) -> CliResult<PathBuf> {
    let directory = project_root.join(".vscode");
    fs::create_dir_all(&directory).map_err(|e| CliError::IoError {
        context: "failed to create .vscode/".to_string(),
        source: e,
    })?;

    let path = directory.join("mcp.json");
    let existing = if path.exists() {
        Some(fs::read_to_string(&path).map_err(|e| CliError::IoError {
            context: format!("failed to read {}", path.display()),
            source: e,
        })?)
    } else {
        None
    };

    let rendered = merge_vscode_config(existing.as_deref(), name, binary)?;
    fs::write(&path, rendered).map_err(|e| CliError::IoError {
        context: format!("failed to write {}", path.display()),
        source: e,
    })?;
    Ok(path)
}

/// Merges the project's stdio server entry into an existing `.vscode/mcp.json`
/// document, preserving every other server. A document that cannot be parsed
/// as JSON is left untouched so user configuration is never clobbered.
fn merge_vscode_config(existing: Option<&str>, name: &str, binary: &str) -> CliResult<String> {
    let mut root = match existing {
        Some(contents) if !contents.trim().is_empty() => {
            let parsed: Value = serde_json::from_str(contents).map_err(|error| {
                CliError::with_hint(
                    format!("existing .vscode/mcp.json is not valid JSON: {error}"),
                    "fix or remove .vscode/mcp.json and run `solverforge connect --write vscode` again",
                )
            })?;
            let Value::Object(map) = parsed else {
                return Err(CliError::with_hint(
                    "existing .vscode/mcp.json is not a JSON object",
                    "fix or remove .vscode/mcp.json and run `solverforge connect --write vscode` again",
                ));
            };
            map
        }
        _ => Map::new(),
    };

    let servers = root
        .entry("servers".to_string())
        .or_insert_with(|| Value::Object(Map::new()));
    let Value::Object(servers) = servers else {
        return Err(CliError::with_hint(
            "existing .vscode/mcp.json has a non-object `servers` entry",
            "fix or remove .vscode/mcp.json and run `solverforge connect --write vscode` again",
        ));
    };

    servers.insert(name.to_string(), stdio_server_json(binary));

    serde_json::to_string_pretty(&Value::Object(root))
        .map(|mut rendered| {
            rendered.push('\n');
            rendered
        })
        .map_err(|error| CliError::general(format!("failed to render VS Code config: {error}")))
}

fn resolve_binary(project_root: &Path, crate_name: &str) -> (String, bool) {
    for profile in ["release", "debug"] {
        let candidate = project_root.join("target").join(profile).join(crate_name);
        if candidate.exists() {
            return (candidate.display().to_string(), true);
        }
    }

    (
        project_root
            .join("target")
            .join("release")
            .join(crate_name)
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
