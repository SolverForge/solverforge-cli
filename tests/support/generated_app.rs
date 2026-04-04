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
        let child = Command::new("cargo")
            .args(["run", "--quiet"])
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

    pub fn create_schedule_from_demo(&self, client: &Client, port: u16) -> String {
        self.phase("Create schedule from demo data");
        let base_url = self.base_url(port);
        let dto: Value = client
            .get(format!("{base_url}/demo-data/STANDARD"))
            .send()
            .expect("failed to fetch demo data")
            .error_for_status()
            .expect("demo data request failed")
            .json()
            .expect("demo data should be JSON");
        let response: Value = client
            .post(format!("{base_url}/schedules"))
            .json(&dto)
            .send()
            .expect("failed to create schedule")
            .error_for_status()
            .expect("create schedule request failed")
            .json()
            .expect("create schedule should return JSON");
        response["id"]
            .as_str()
            .expect("schedule id should be a string")
            .to_string()
    }

    pub fn read_sse_event_types(
        &self,
        client: &Client,
        port: u16,
        id: &str,
        max_events: usize,
    ) -> Vec<String> {
        self.phase("Observe SSE events");
        let response = client
            .get(format!("{}/schedules/{id}/events", self.base_url(port)))
            .send()
            .expect("failed to open SSE stream")
            .error_for_status()
            .expect("SSE endpoint returned error");
        let mut reader = BufReader::new(response);
        let mut line = String::new();
        let mut event_types = Vec::new();
        let start = Instant::now();
        while event_types.len() < max_events && start.elapsed() < Duration::from_secs(10) {
            line.clear();
            let read = reader
                .read_line(&mut line)
                .expect("failed to read SSE line");
            if read == 0 {
                break;
            }
            if let Some(rest) = line.strip_prefix("data: ") {
                if let Ok(value) = serde_json::from_str::<Value>(rest.trim()) {
                    if let Some(event_type) = value["eventType"].as_str() {
                        event_types.push(event_type.to_string());
                        if event_type == "finished" {
                            break;
                        }
                    }
                }
            }
        }
        event_types
    }

    pub fn mark_success(&mut self) {
        self.success = true;
        let _ = fs::remove_dir_all(&self.artifact_dir);
    }

    fn wait_for_ready(&self, port: u16) {
        let client = self.client();
        let start = Instant::now();
        let url = format!("{}/health", self.base_url(port));
        while start.elapsed() < Duration::from_secs(40) {
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
}

impl FromStr for DemoData {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_uppercase().as_str() {
            "SMALL" => Ok(DemoData::Small),
            "STANDARD" => Ok(DemoData::Standard),
            _ => Err(()),
        }
    }
}

pub fn generate(demo: DemoData) -> Plan {
    match demo {
        DemoData::Small => generate_plan(2, 3, 2, 4),
        DemoData::Standard => generate_plan(3, 6, 3, 8),
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

pub fn seeded_standard_data_module() -> &'static str {
    r#"/* Seeded standard-variable demo data for end-to-end pipeline tests. */

use std::str::FromStr;

use crate::domain::{Plan, Resource, Task};

#[derive(Debug, Clone, Copy)]
pub enum DemoData {
    Small,
    Standard,
}

impl FromStr for DemoData {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_uppercase().as_str() {
            "SMALL" => Ok(DemoData::Small),
            "STANDARD" => Ok(DemoData::Standard),
            _ => Err(()),
        }
    }
}

pub fn generate(demo: DemoData) -> Plan {
    match demo {
        DemoData::Small => generate_plan(2, 4),
        DemoData::Standard => generate_plan(3, 6),
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
