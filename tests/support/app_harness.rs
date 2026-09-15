//! Shared plumbing for generated-app test harnesses.
//!
//! Suite-specific harnesses (`generated_app`, `mcp_generated_app`) build on
//! these helpers so process spawning, health waiting, artifact paths, and
//! binary resolution stay identical across suites.

use std::fs;
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use reqwest::blocking::Client;
use serde_json::Value;

pub const CLI_MANIFEST_PATH: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.toml");

pub fn cli_command() -> Command {
    let mut command = Command::new("cargo");
    command.args([
        "run",
        "--quiet",
        "--manifest-path",
        CLI_MANIFEST_PATH,
        "--bin",
        "solverforge",
        "--",
    ]);
    command
}

pub fn artifact_root(test_name: &str) -> PathBuf {
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock should be after epoch")
        .as_secs();
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("target")
        .join("test-artifacts")
        .join(test_name)
        .join(ts.to_string())
}

pub fn find_free_port() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").expect("failed to allocate free port");
    listener
        .local_addr()
        .expect("listener should have a local addr")
        .port()
}

pub fn resolve_built_executable(stdout: &[u8]) -> Option<PathBuf> {
    String::from_utf8_lossy(stdout)
        .lines()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .filter(|message| message["reason"] == "compiler-artifact")
        .filter(|message| {
            message["target"]["kind"]
                .as_array()
                .map(|kinds| kinds.iter().any(|kind| kind.as_str() == Some("bin")))
                .unwrap_or(false)
        })
        .filter_map(|message| message["executable"].as_str().map(PathBuf::from))
        .next_back()
}

pub fn copy_dir_recursive(src: &Path, dst: &Path) -> std::io::Result<()> {
    if !src.exists() {
        return Ok(());
    }
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        let target = dst.join(entry.file_name());
        if file_type.is_dir() {
            copy_dir_recursive(&entry.path(), &target)?;
        } else if file_type.is_file() {
            fs::copy(entry.path(), target)?;
        }
    }
    Ok(())
}

/// Spawns a built generated binary with the given args and both streams
/// captured to log files.
pub fn spawn_generated_server(
    binary: &Path,
    args: &[String],
    project_dir: &Path,
    port: u16,
    stdout_log: &Path,
    stderr_log: &Path,
) -> Child {
    let stdout = fs::File::create(stdout_log).expect("failed to create stdout log");
    let stderr = fs::File::create(stderr_log).expect("failed to create stderr log");
    Command::new(binary)
        .args(args)
        .env("PORT", port.to_string())
        .current_dir(project_dir)
        .stdout(Stdio::from(stdout))
        .stderr(Stdio::from(stderr))
        .spawn()
        .expect("failed to start generated server")
}

/// Polls `/health` until the spawned server answers or fails.
pub fn wait_for_health(port: u16, child: &mut Child, stdout_log: &Path, stderr_log: &Path) {
    let client = Client::builder()
        .timeout(Duration::from_secs(15))
        .build()
        .expect("failed to build reqwest client");
    let start = Instant::now();
    let url = format!("http://127.0.0.1:{port}/health");
    while start.elapsed() < Duration::from_secs(40) {
        if let Some(status) = child
            .try_wait()
            .expect("failed to inspect generated server status")
        {
            panic!(
                "server exited before becoming ready with status {status}. stdout: {} stderr: {}",
                stdout_log.display(),
                stderr_log.display()
            );
        }
        if let Ok(response) = client.get(&url).send() {
            if response.status().is_success() {
                return;
            }
        }
        thread::sleep(Duration::from_millis(250));
    }
    panic!(
        "server never became ready on port {port}. stdout: {} stderr: {}",
        stdout_log.display(),
        stderr_log.display()
    );
}

/// A short, deterministic solver configuration shared by the generated-app
/// runtime suites so solves reach terminal states inside test timeouts.
pub fn short_solver_config() -> &'static str {
    r#"[[phases]]
type = "construction_heuristic"
construction_heuristic_type = "first_fit"

[[phases]]
type = "local_search"
[phases.acceptor]
type = "late_acceptance"
late_acceptance_size = 400
[phases.forager]
type = "accepted_count"
limit = 4

[termination]
seconds_spent_limit = 5

[candidate_trace]
max_entries = 128
"#
}
