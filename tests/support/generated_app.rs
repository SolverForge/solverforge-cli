use super::dependency_overrides::{
    apply_generated_project_dependency_overrides, DependencyOverrideMode, USE_LOCAL_PATCHES_ENV,
};
use reqwest::blocking::Client;
use serde_json::Value;
use std::fs;
use std::io::{BufRead, BufReader};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tempfile::TempDir;

const CLI_MANIFEST_PATH: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.toml");

pub struct GeneratedApp {
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

impl GeneratedApp {
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
            "{} failed: {}",
            label,
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

    pub fn write_file(&self, relative_path: &str, contents: &str) {
        let path = self.project_dir.join(relative_path);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("failed to create parent dir");
        }
        fs::write(&path, contents).expect("failed to write generated test fixture file");
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

    pub fn create_job_from_demo(&self, client: &Client, port: u16, demo_name: &str) -> String {
        self.phase("Create job from demo data");
        let base_url = self.base_url(port);
        let dto: Value = client
            .get(format!("{base_url}/demo-data/{demo_name}"))
            .send()
            .expect("failed to fetch demo data")
            .error_for_status()
            .expect("demo data request failed")
            .json()
            .expect("demo data should be JSON");
        let response: Value = client
            .post(format!("{base_url}/jobs"))
            .json(&dto)
            .send()
            .expect("failed to create job")
            .error_for_status()
            .expect("create job request failed")
            .json()
            .expect("create job should return JSON");
        response["id"]
            .as_str()
            .expect("job id should be a string")
            .to_string()
    }

    pub fn read_first_sse_event(&self, client: &Client, port: u16, id: &str) -> Value {
        self.phase("Observe SSE bootstrap event");
        let response = client
            .get(format!("{}/jobs/{id}/events", self.base_url(port)))
            .send()
            .expect("failed to open SSE stream")
            .error_for_status()
            .expect("SSE endpoint returned error");
        let mut reader = BufReader::new(response);
        let mut line = String::new();
        let start = Instant::now();
        while start.elapsed() < Duration::from_secs(10) {
            line.clear();
            let read = reader
                .read_line(&mut line)
                .expect("failed to read SSE line");
            if read == 0 {
                break;
            }
            if let Some(rest) = line.strip_prefix("data: ") {
                if let Ok(value) = serde_json::from_str::<Value>(rest.trim()) {
                    return value;
                }
            }
        }
        panic!("timed out waiting for first SSE event from job {id}");
    }

    pub fn mark_success(&mut self) {
        self.success = true;
        let _ = fs::remove_dir_all(&self.artifact_dir);
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

impl Drop for GeneratedApp {
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

pub fn seeded_mixed_data_module() -> &'static str {
    r#"/* Seeded demo data for end-to-end pipeline tests. */

use std::str::FromStr;

use crate::domain::{Container, Item, Plan, Resource, Task};

#[derive(Debug, Clone, Copy)]
pub enum DemoData {
    Small,
    Standard,
    Large,
}

const AVAILABLE_DEMO_DATA: &[DemoData] = &[DemoData::Small, DemoData::Standard, DemoData::Large];
const DEFAULT_DEMO_DATA: DemoData = DemoData::Standard;

pub fn default_demo_data() -> DemoData {
    DEFAULT_DEMO_DATA
}

pub fn available_demo_data() -> &'static [DemoData] {
    AVAILABLE_DEMO_DATA
}

impl DemoData {
    pub fn id(self) -> &'static str {
        match self {
            DemoData::Small => "SMALL",
            DemoData::Standard => "STANDARD",
            DemoData::Large => "LARGE",
        }
    }

    pub fn default_demo_data() -> Self {
        default_demo_data()
    }

    pub fn available_demo_data() -> &'static [Self] {
        available_demo_data()
    }
}

impl FromStr for DemoData {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_uppercase().as_str() {
            "SMALL" => Ok(DemoData::Small),
            "STANDARD" => Ok(DemoData::Standard),
            "LARGE" => Ok(DemoData::Large),
            _ => Err(()),
        }
    }
}

pub fn generate(demo: DemoData) -> Plan {
    match demo {
        DemoData::Small => generate_plan(2, 3, 2, 4),
        DemoData::Standard => generate_plan(3, 6, 3, 8),
        DemoData::Large => generate_plan(6, 12, 6, 24),
    }
}

fn generate_plan(n_resources: usize, n_tasks: usize, n_containers: usize, n_items: usize) -> Plan {
    let resources = (0..n_resources)
        .map(|idx| Resource::new(format!("resource-{idx}"), format!("resource-{idx}")))
        .collect::<Vec<_>>();
    let tasks = (0..n_tasks)
        .map(|idx| Task::new(format!("task-{idx}")))
        .collect::<Vec<_>>();
    let items = (0..n_items)
        .map(|idx| Item::new(format!("item-{idx}"), format!("item-{idx}")))
        .collect::<Vec<_>>();
    let containers = (0..n_containers)
        .map(|idx| Container::new(format!("container-{idx}")))
        .collect::<Vec<_>>();

    Plan::new(resources, tasks, items, containers)
}
"#
}

pub fn seeded_scalar_data_module() -> &'static str {
    r#"/* Seeded scalar-variable demo data for end-to-end pipeline tests. */

use std::str::FromStr;

use crate::domain::{Plan, Resource, Task};

#[derive(Debug, Clone, Copy)]
pub enum DemoData {
    Small,
    Standard,
    Large,
}

const AVAILABLE_DEMO_DATA: &[DemoData] = &[DemoData::Small, DemoData::Standard, DemoData::Large];
const DEFAULT_DEMO_DATA: DemoData = DemoData::Standard;

pub fn default_demo_data() -> DemoData {
    DEFAULT_DEMO_DATA
}

pub fn available_demo_data() -> &'static [DemoData] {
    AVAILABLE_DEMO_DATA
}

impl DemoData {
    pub fn id(self) -> &'static str {
        match self {
            DemoData::Small => "SMALL",
            DemoData::Standard => "STANDARD",
            DemoData::Large => "LARGE",
        }
    }

    pub fn default_demo_data() -> Self {
        default_demo_data()
    }

    pub fn available_demo_data() -> &'static [Self] {
        available_demo_data()
    }
}

impl FromStr for DemoData {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_uppercase().as_str() {
            "SMALL" => Ok(DemoData::Small),
            "STANDARD" => Ok(DemoData::Standard),
            "LARGE" => Ok(DemoData::Large),
            _ => Err(()),
        }
    }
}

pub fn generate(demo: DemoData) -> Plan {
    match demo {
        DemoData::Small => generate_plan(4, 16),
        DemoData::Standard => generate_plan(8, 48),
        DemoData::Large => generate_plan(16, 192),
    }
}

fn generate_plan(n_resources: usize, n_tasks: usize) -> Plan {
    let resources = (0..n_resources)
        .map(|idx| Resource::new(format!("resource-{idx}"), format!("resource-{idx}")))
        .collect::<Vec<_>>();
    let tasks = (0..n_tasks)
        .map(|idx| Task::new(format!("task-{idx}")))
        .collect::<Vec<_>>();

    Plan::new(resources, tasks)
}
"#
}
