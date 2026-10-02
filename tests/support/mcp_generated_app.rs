//! Focused generated-app harness for the MCP-shell runtime suite.
//!
//! The REST suite's `GeneratedApp` carries HTTP-job helpers that the MCP
//! suite never uses; this harness keeps only the MCP-relevant lifecycle:
//! scaffold an MCP app, model it through the CLI, build it, and boot the
//! stateless Streamable HTTP transport.

use super::app_harness::{
    artifact_root, cli_command, copy_dir_recursive, find_free_port, resolve_built_executable,
    spawn_generated_server, wait_for_health,
};
use super::dependency_overrides::{
    apply_generated_project_dependency_overrides, DependencyOverrideMode, USE_LOCAL_PATCHES_ENV,
};
use super::network_retry::cargo_with_network_retry;
use std::fs;
use std::path::PathBuf;
use std::process::Child;
use tempfile::TempDir;

pub struct McpGeneratedApp {
    test_name: String,
    temp_dir: TempDir,
    project_name: String,
    project_dir: PathBuf,
    artifact_dir: PathBuf,
    stdout_log: PathBuf,
    stderr_log: PathBuf,
    built_binary_path: Option<PathBuf>,
    server: Option<Child>,
    success: bool,
}

impl McpGeneratedApp {
    pub fn new(test_name: &str, project_name: &str) -> Self {
        let temp_dir = tempfile::tempdir().expect("failed to create temp dir");
        let project_dir = temp_dir.path().join(project_name);
        let artifact_dir = artifact_root(test_name);
        let stdout_log = artifact_dir.join("server.stdout.log");
        let stderr_log = artifact_dir.join("server.stderr.log");
        fs::create_dir_all(&artifact_dir).expect("failed to create artifact dir");
        Self {
            test_name: test_name.to_string(),
            temp_dir,
            project_name: project_name.to_string(),
            project_dir,
            artifact_dir,
            stdout_log,
            stderr_log,
            built_binary_path: None,
            server: None,
            success: false,
        }
    }

    pub fn phase(&self, title: &str) {
        println!("\n=== PHASE: {} :: {} ===", self.test_name, title);
    }

    pub fn scaffold_mcp(&self) {
        self.phase("Scaffold MCP app");
        let output = cli_command()
            .args([
                "new",
                &self.project_name,
                "--shell",
                "mcp",
                "--skip-git",
                "--skip-readme",
                "--quiet",
            ])
            .current_dir(self.temp_dir.path())
            .output()
            .expect("failed to run solverforge new");
        self.record_command("scaffold", &output.stdout, &output.stderr);
        assert!(
            output.status.success(),
            "mcp scaffold failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        match apply_generated_project_dependency_overrides(&self.project_dir) {
            DependencyOverrideMode::CratesIo => {
                println!(
                    "=== INFO: {} :: Using registry SolverForge dependency targets for generated-app validation (set {}=1 to apply explicit local Cargo patches) ===",
                    self.test_name, USE_LOCAL_PATCHES_ENV
                );
            }
            DependencyOverrideMode::LocalPatches => {
                println!(
                    "=== INFO: {} :: Using explicit local Cargo patches from generated .cargo/config.toml ===",
                    self.test_name
                );
            }
        }
    }

    pub fn run_cli(&self, label: &str, args: &[&str]) {
        self.phase(label);
        let output = cli_command()
            .args(args)
            .current_dir(&self.project_dir)
            .output()
            .expect("failed to run solverforge CLI command");
        self.record_command(label, &output.stdout, &output.stderr);
        assert!(
            output.status.success(),
            "{label} failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    pub fn write_file(&self, relative_path: &str, contents: &str) {
        let path = self.project_dir.join(relative_path);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("failed to create parent dir");
        }
        fs::write(&path, contents).expect("failed to write generated test fixture file");
    }

    pub fn cargo_build(&mut self, label: &str) {
        self.phase(label);
        let output = cargo_with_network_retry(
            &self.project_dir,
            &["build", "--message-format=json-render-diagnostics"],
            "cargo build",
        );
        self.record_command("cargo-build", &output.stdout, &output.stderr);
        assert!(
            output.status.success(),
            "cargo build failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        self.built_binary_path =
            Some(resolve_built_executable(&output.stdout).unwrap_or_else(|| {
                panic!(
                    "cargo build succeeded but no executable artifact was reported. log: {}",
                    self.artifact_dir.join("cargo-build.log").display()
                )
            }));
    }

    /// Boots the generated binary in Streamable HTTP mode on a free port and
    /// waits for its `/health` route.
    pub fn start_http_server(&mut self) -> u16 {
        self.phase("Boot generated MCP HTTP server");
        let port = find_free_port();
        let args = vec!["--http".to_string(), "--port".to_string(), port.to_string()];
        let child = spawn_generated_server(
            &self.binary_path(),
            &args,
            &self.project_dir,
            port,
            &self.stdout_log,
            &self.stderr_log,
        );
        self.server = Some(child);
        if let Some(server) = self.server.as_mut() {
            wait_for_health(port, server, &self.stdout_log, &self.stderr_log);
        }
        port
    }

    /// Path to the binary produced by [`Self::cargo_build`].
    pub fn binary_path(&self) -> PathBuf {
        self.built_binary_path.clone().unwrap_or_else(|| {
            panic!("generated binary path is unavailable; call cargo_build first")
        })
    }

    pub fn mark_success(&mut self) {
        self.success = true;
        let _ = fs::remove_dir_all(&self.artifact_dir);
    }

    fn record_command(&self, label: &str, stdout: &[u8], stderr: &[u8]) {
        let path = self.artifact_dir.join(format!("{label}.log"));
        let content = format!(
            "=== STDOUT ===\n{}\n=== STDERR ===\n{}\n",
            String::from_utf8_lossy(stdout),
            String::from_utf8_lossy(stderr)
        );
        fs::write(path, content).expect("failed to write command log");
    }
}

impl Drop for McpGeneratedApp {
    fn drop(&mut self) {
        if let Some(mut child) = self.server.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
        if !self.success {
            let snapshot_dir = self.artifact_dir.join("project-snapshot");
            let _ = copy_dir_recursive(&self.project_dir, &snapshot_dir);
        }
    }
}
