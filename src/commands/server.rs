use std::path::Path;
use std::process::Command;

use crate::app_spec;
use crate::error::{CliError, CliResult};
use crate::output;

pub fn run(port: u16, debug: bool) -> CliResult {
    let shell = project_shell()?;

    let mode = if debug { "debug" } else { "release" };

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

    // Set PORT env var for the server to pick up
    let status = Command::new("cargo")
        .args(server_args(shell.as_deref(), debug))
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

/// Builds the `cargo run` argument list for a shell.
///
/// MCP-shell binaries serve the MCP transport on stdout by default, so the
/// server command must select their stateless Streamable HTTP transport
/// explicitly; every other shell keeps its default entry point.
fn server_args(shell: Option<&str>, debug: bool) -> Vec<String> {
    let mut args = vec!["run".to_string()];
    if !debug {
        args.push("--release".to_string());
    }
    if shell == Some("mcp") {
        args.push("--".to_string());
        args.push("--http".to_string());
    }
    args
}

fn project_shell() -> CliResult<Option<String>> {
    if !Path::new("solverforge.app.toml").exists() {
        return Ok(None);
    }
    let spec = app_spec::load()?;
    Ok(Some(spec.app.shell))
}

#[cfg(test)]
mod tests {
    use super::server_args;

    #[test]
    fn mcp_shell_boots_the_http_transport() {
        assert_eq!(
            server_args(Some("mcp"), false),
            vec!["run", "--release", "--", "--http"]
        );
        assert_eq!(server_args(Some("mcp"), true), vec!["run", "--", "--http"]);
    }

    #[test]
    fn other_shells_keep_their_default_entry_point() {
        assert_eq!(server_args(Some("web"), false), vec!["run", "--release"]);
        assert_eq!(server_args(Some("api"), true), vec!["run"]);
        assert_eq!(server_args(None, false), vec!["run", "--release"]);
    }
}
