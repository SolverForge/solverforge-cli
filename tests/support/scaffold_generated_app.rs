use crate::dependency_overrides::{
    apply_generated_project_dependency_overrides, DependencyOverrideMode, USE_LOCAL_PATCHES_ENV,
};
use reqwest::blocking::Client;
use serde_json::Value;
use std::fs;
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tempfile::TempDir;

const CLI_MANIFEST_PATH: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.toml");

pub struct ScaffoldGeneratedApp {
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

impl ScaffoldGeneratedApp {
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

    pub fn project_dir(&self) -> &Path {
        &self.project_dir
    }

    pub fn scaffold_neutral(&self) {
        self.phase("Scaffold neutral app");
        let output = cli_command()
            .args([
                "new",
                &self.project_name,
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
            "scaffold failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        match apply_generated_project_dependency_overrides(&self.project_dir) {
            DependencyOverrideMode::CratesIo => {
                println!(
                    "=== INFO: {} :: Using published SolverForge crate targets for generated-app validation (set {}=1 to apply explicit local Cargo patches) ===",
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

    pub fn cargo_check(&self, label: &str) {
        self.phase(label);
        let output = Command::new("cargo")
            .arg("check")
            .current_dir(&self.project_dir)
            .output()
            .expect("failed to run cargo check");
        self.record_command("cargo-check", &output.stdout, &output.stderr);
        assert!(
            output.status.success(),
            "cargo check failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    pub fn cargo_build(&mut self, label: &str) {
        self.phase(label);
        let output = Command::new("cargo")
            .args(["build", "--message-format=json-render-diagnostics"])
            .current_dir(&self.project_dir)
            .output()
            .expect("failed to run cargo build");
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

    pub fn start_server(&mut self) -> u16 {
        self.phase("Boot generated server");
        let port = find_free_port();
        let stdout = fs::File::create(&self.stdout_log).expect("failed to create stdout log");
        let stderr = fs::File::create(&self.stderr_log).expect("failed to create stderr log");
        let child = Command::new(self.project_binary_path())
            .env("PORT", port.to_string())
            .current_dir(&self.project_dir)
            .stdout(Stdio::from(stdout))
            .stderr(Stdio::from(stderr))
            .spawn()
            .expect("failed to start generated server");
        self.server = Some(child);
        self.wait_for_ready(port);
        port
    }

    pub fn client(&self) -> Client {
        Client::builder()
            .timeout(Duration::from_secs(15))
            .build()
            .expect("failed to build reqwest client")
    }

    pub fn base_url(&self, port: u16) -> String {
        format!("http://127.0.0.1:{port}")
    }

    pub fn mark_success(&mut self) {
        self.success = true;
        let _ = fs::remove_dir_all(&self.artifact_dir);
    }

    fn phase(&self, title: &str) {
        println!("\n=== PHASE: {} :: {} ===", self.test_name, title);
    }

    fn wait_for_ready(&mut self, port: u16) {
        let client = self.client();
        let start = Instant::now();
        let url = format!("{}/health", self.base_url(port));
        while start.elapsed() < Duration::from_secs(40) {
            if let Some(child) = self.server.as_mut() {
                if let Some(status) = child
                    .try_wait()
                    .expect("failed to inspect generated server status")
                {
                    panic!(
                        "server exited before becoming ready with status {}. stdout: {} stderr: {}",
                        status,
                        self.stdout_log.display(),
                        self.stderr_log.display()
                    );
                }
            }
            if let Ok(response) = client.get(&url).send() {
                if response.status().is_success() {
                    return;
                }
            }
            thread::sleep(Duration::from_millis(250));
        }
        panic!(
            "server never became ready on port {}. stdout: {} stderr: {}",
            port,
            self.stdout_log.display(),
            self.stderr_log.display()
        );
    }

    fn project_binary_path(&self) -> PathBuf {
        self.built_binary_path.clone().unwrap_or_else(|| {
            panic!(
                "generated server binary path is unavailable; call cargo_build before start_server"
            )
        })
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

impl Drop for ScaffoldGeneratedApp {
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

fn cli_command() -> Command {
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

fn resolve_built_executable(stdout: &[u8]) -> Option<PathBuf> {
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

fn artifact_root(test_name: &str) -> PathBuf {
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

fn find_free_port() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").expect("failed to allocate free port");
    listener
        .local_addr()
        .expect("listener should have a local addr")
        .port()
}

fn copy_dir_recursive(src: &Path, dst: &Path) -> std::io::Result<()> {
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
