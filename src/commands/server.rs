use std::path::Path;
use std::process::Command;

use crate::app_spec;
use crate::error::{CliError, CliResult};
use crate::output;

pub fn run(port: u16, debug: bool) -> CliResult {
    let shell = project_shell()?;

    let mode = if debug { "debug" } else { "release" };

    // MCP-shell projects have no long-lived REST server; `server` boots their
    // stateless Streamable HTTP MCP transport instead.
    let mut extra_args: Vec<&str> = Vec::new();
    if shell.as_deref() == Some("mcp") {
        extra_args.push("--http");
    }

    if shell.as_deref() == Some("cli") {
        return Err(CliError::with_hint(
            "`solverforge server` is not available for CLI-shell projects",
            "run the generated command-line app with `cargo run -- demo-data`",
        ));
    }

    output::print_status(
        "start",
        &format!(
            "SolverForge server ({}){}",
            mode,
            if shell.as_deref() == Some("mcp") {
                " — MCP over Streamable HTTP at /mcp"
            } else {
                ""
            }
        ),
    );

    if debug {
        output::print_dim("  Compiling in debug mode... (this may take a moment on first run)");
    } else {
        output::print_dim("  Compiling in release mode... (this may take a minute on first run)");
    }

    let mut args = vec!["run"];
    if !debug {
        args.push("--release");
    }
    if !extra_args.is_empty() {
        args.push("--");
        args.extend(extra_args);
    }

    // Set PORT env var for the server to pick up
    let status = Command::new("cargo")
        .args(&args)
        .env("PORT", port.to_string())
        .status()
        .map_err(|e| CliError::IoError {
            context: "failed to run cargo".to_string(),
            source: e,
        })?;

    if status.success() {
        Ok(())
    } else {
        Err(CliError::SubprocessFailed {
            command: format!("cargo run --{}", mode),
        })
    }
}

fn project_shell() -> CliResult<Option<String>> {
    if !Path::new("solverforge.app.toml").exists() {
        return Ok(None);
    }
    let spec = app_spec::load()?;
    Ok(Some(spec.app.shell))
}
