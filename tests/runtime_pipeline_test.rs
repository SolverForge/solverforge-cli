mod support;

use reqwest::blocking::Client;
use serde_json::Value;
use std::sync::{Mutex, OnceLock};
use std::thread;
use std::time::{Duration, Instant};
use support::generated_app::{seeded_mixed_data_module, seeded_standard_data_module, GeneratedApp};

fn test_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

#[test]
fn neutral_shell_pipeline() {
    let _guard = test_lock()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());

    // PHASE 1: scaffold a fresh generated app.
    let mut app = GeneratedApp::new("neutral_shell_pipeline", "neutral_shell_pipeline");
    app.scaffold_neutral();
    app.cargo_check("Compile generated neutral app");

    // PHASE 2: boot the generated server and verify shell endpoints.
    let port = app.start_server();
    let client = app.client();
    let base_url = app.base_url(port);

    let health: Value = client
        .get(format!("{base_url}/health"))
        .send()
        .expect("health request failed")
        .error_for_status()
        .expect("health should be successful")
        .json()
        .expect("health should return JSON");
    assert_eq!(health["status"], "UP");

    let info: Value = client
        .get(format!("{base_url}/info"))
        .send()
        .expect("info request failed")
        .error_for_status()
        .expect("info should be successful")
        .json()
        .expect("info should return JSON");
    assert_eq!(info["solverEngine"], "SolverForge");

    let demo: Value = client
        .get(format!("{base_url}/demo-data/STANDARD"))
        .send()
        .expect("demo data request failed")
        .error_for_status()
        .expect("demo data should be successful")
        .json()
        .expect("demo data should be JSON");
    assert!(demo["score"].is_null());

    app.mark_success();
}

#[test]
fn mixed_runtime_pipeline() {
    let _guard = test_lock()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());

    // PHASE 1: scaffold and build a mixed-shape generated app through real CLI commands.
    let mut app = GeneratedApp::new("mixed_runtime_pipeline", "mixed_runtime_pipeline");
    app.scaffold_neutral();
    app.run_cli("Generate fact resources", &["generate", "fact", "resource"]);
    app.run_cli("Generate entity tasks", &["generate", "entity", "task"]);
    app.run_cli(
        "Generate standard variable",
        &[
            "generate",
            "variable",
            "resource_idx",
            "--entity",
            "Task",
            "--kind",
            "standard",
            "--range",
            "resources",
            "--allows-unassigned",
        ],
    );
    app.run_cli("Generate fact items", &["generate", "fact", "item"]);
    app.run_cli(
        "Generate entity containers",
        &["generate", "entity", "container"],
    );
    app.run_cli(
        "Generate list variable",
        &[
            "generate",
            "variable",
            "item_order",
            "--entity",
            "Container",
            "--kind",
            "list",
            "--elements",
            "items",
        ],
    );
    app.phase("Seed non-empty mixed demo data");
    app.write_file("src/data/mod.rs", seeded_mixed_data_module());
    app.cargo_check("Compile generated mixed app");

    // PHASE 2: boot the generated server and verify the mixed runtime surface.
    let port = app.start_server();
    let client: Client = app.client();
    let base_url = app.base_url(port);

    let demo: Value = client
        .get(format!("{base_url}/demo-data/STANDARD"))
        .send()
        .expect("demo data request failed")
        .error_for_status()
        .expect("demo data should be successful")
        .json()
        .expect("demo data should be JSON");
    assert!(demo["resources"].is_array());
    assert!(demo["tasks"].is_array());
    assert!(demo["items"].is_array());
    assert!(demo["containers"].is_array());
    assert!(demo["resources"].as_array().expect("resources array").len() >= 3);
    assert!(demo["tasks"].as_array().expect("tasks array").len() >= 6);
    assert!(demo["items"].as_array().expect("items array").len() >= 8);
    assert!(
        demo["containers"]
            .as_array()
            .expect("containers array")
            .len()
            >= 3
    );

    app.mark_success();
}

#[test]
fn standard_solver_pipeline() {
    let _guard = test_lock()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());

    // PHASE 1: scaffold and build a standard-variable generated app through real CLI commands.
    let mut app = GeneratedApp::new("standard_solver_pipeline", "standard_solver_pipeline");
    app.scaffold_neutral();
    app.run_cli("Generate fact resources", &["generate", "fact", "resource"]);
    app.run_cli("Generate entity tasks", &["generate", "entity", "task"]);
    app.run_cli(
        "Generate standard variable",
        &[
            "generate",
            "variable",
            "resource_idx",
            "--entity",
            "Task",
            "--kind",
            "standard",
            "--range",
            "resources",
            "--allows-unassigned",
        ],
    );
    app.phase("Seed non-empty standard demo data");
    app.write_file("src/data/mod.rs", seeded_standard_data_module());
    app.write_file("solver.toml", short_runtime_solver_config());
    app.cargo_check("Compile generated standard app");

    // PHASE 2: boot the generated server and run real data through the solver.
    let port = app.start_server();
    let client: Client = app.client();
    let base_url = app.base_url(port);

    let demo: Value = client
        .get(format!("{base_url}/demo-data/STANDARD"))
        .send()
        .expect("demo data request failed")
        .error_for_status()
        .expect("demo data should be successful")
        .json()
        .expect("demo data should be JSON");
    assert!(demo["resources"].as_array().map(|rows| rows.len() >= 8) == Some(true));
    assert!(demo["tasks"].as_array().map(|rows| rows.len() >= 48) == Some(true));

    let live_job_id = app.create_job_from_demo(&client, port, "STANDARD");
    let solving = wait_for_job_snapshot(&client, &base_url, &live_job_id);
    assert_eq!(solving["lifecycleState"], "SOLVING");
    assert!(solving["snapshotRevision"].as_u64().is_some());
    assert!(solving.get("currentScore").is_some());
    assert!(solving.get("bestScore").is_some());

    let latest_snapshot = client
        .get(format!("{base_url}/jobs/{live_job_id}/snapshot"))
        .send()
        .expect("snapshot request failed")
        .error_for_status()
        .expect("snapshot should be successful")
        .json::<Value>()
        .expect("snapshot should return JSON");
    assert!(
        latest_snapshot["solution"]["resources"]
            .as_array()
            .map(|rows| !rows.is_empty())
            == Some(true),
        "expected seeded resources in retained snapshot"
    );
    assert!(
        latest_snapshot["solution"]["tasks"]
            .as_array()
            .map(|rows| !rows.is_empty())
            == Some(true),
        "expected seeded tasks in retained snapshot"
    );

    let latest_revision = latest_snapshot["snapshotRevision"]
        .as_u64()
        .expect("latest snapshot should expose a revision");
    let live_analysis = client
        .get(format!(
            "{base_url}/jobs/{live_job_id}/analysis?snapshot_revision={latest_revision}"
        ))
        .send()
        .expect("analysis request failed")
        .error_for_status()
        .expect("analysis should be successful")
        .json::<Value>()
        .expect("analysis should return JSON");
    assert_eq!(
        live_analysis["snapshotRevision"].as_u64(),
        Some(latest_revision)
    );
    assert!(live_analysis["analysis"]["constraints"].is_array());

    let pause_status = client
        .post(format!("{base_url}/jobs/{live_job_id}/pause"))
        .send()
        .expect("pause request failed")
        .status();
    assert_eq!(pause_status.as_u16(), 202);

    let paused = wait_for_job_state(&client, &base_url, &live_job_id, "PAUSED");
    assert_eq!(paused["lifecycleState"], "PAUSED");
    assert_eq!(paused["checkpointAvailable"], true);
    assert_eq!(
        app.read_first_sse_event(&client, port, &live_job_id)["eventType"],
        "paused"
    );

    let paused_revision = paused["snapshotRevision"]
        .as_u64()
        .expect("paused job should expose a retained snapshot revision");
    let paused_snapshot = client
        .get(format!(
            "{base_url}/jobs/{live_job_id}/snapshot?snapshot_revision={paused_revision}"
        ))
        .send()
        .expect("paused snapshot request failed")
        .error_for_status()
        .expect("paused snapshot should be successful")
        .json::<Value>()
        .expect("paused snapshot should return JSON");
    assert_eq!(
        paused_snapshot["snapshotRevision"].as_u64(),
        Some(paused_revision)
    );

    let paused_analysis = client
        .get(format!(
            "{base_url}/jobs/{live_job_id}/analysis?snapshot_revision={paused_revision}"
        ))
        .send()
        .expect("paused analysis request failed")
        .error_for_status()
        .expect("paused analysis should be successful")
        .json::<Value>()
        .expect("paused analysis should return JSON");
    assert_eq!(
        paused_analysis["snapshotRevision"].as_u64(),
        Some(paused_revision)
    );
    assert!(paused_analysis["analysis"]["constraints"].is_array());

    let resume_status = client
        .post(format!("{base_url}/jobs/{live_job_id}/resume"))
        .send()
        .expect("resume request failed")
        .status();
    assert_eq!(resume_status.as_u16(), 202);

    let resumed = wait_for_job_state(&client, &base_url, &live_job_id, "SOLVING");
    assert_eq!(resumed["lifecycleState"], "SOLVING");
    let resumed_bootstrap = app.read_first_sse_event(&client, port, &live_job_id);
    assert!(
        matches!(
            resumed_bootstrap["eventType"].as_str(),
            Some("resumed" | "progress" | "best_solution")
        ),
        "unexpected resumed bootstrap payload: {resumed_bootstrap:?}"
    );

    let cancel_status = client
        .post(format!("{base_url}/jobs/{live_job_id}/cancel"))
        .send()
        .expect("cancel request failed")
        .status();
    assert_eq!(cancel_status.as_u16(), 202);

    let cancelled = wait_for_job_state(&client, &base_url, &live_job_id, "CANCELLED");
    assert_eq!(cancelled["terminalReason"], "cancelled");
    assert_eq!(
        app.read_first_sse_event(&client, port, &live_job_id)["eventType"],
        "cancelled"
    );

    let delete_status = client
        .delete(format!("{base_url}/jobs/{live_job_id}"))
        .send()
        .expect("delete request failed")
        .status();
    assert_eq!(delete_status.as_u16(), 204);
    assert_eq!(
        client
            .get(format!("{base_url}/jobs/{live_job_id}"))
            .send()
            .expect("deleted job lookup failed")
            .status()
            .as_u16(),
        404
    );

    let completed_job_id = app.create_job_from_demo(&client, port, "SMALL");
    let completed = wait_for_job_state(&client, &base_url, &completed_job_id, "COMPLETED");
    assert!(matches!(
        completed["terminalReason"].as_str(),
        Some("completed" | "terminated_by_config")
    ));

    let completed_snapshot = client
        .get(format!("{base_url}/jobs/{completed_job_id}/snapshot"))
        .send()
        .expect("completed snapshot request failed")
        .error_for_status()
        .expect("completed snapshot should be successful")
        .json::<Value>()
        .expect("completed snapshot should return JSON");
    assert!(
        completed_snapshot["solution"]["tasks"]
            .as_array()
            .map(|rows| !rows.is_empty())
            == Some(true),
        "expected completed snapshot to keep the latest rendered plan"
    );

    let completed_analysis = client
        .get(format!("{base_url}/jobs/{completed_job_id}/analysis"))
        .send()
        .expect("completed analysis request failed")
        .error_for_status()
        .expect("completed analysis should be successful")
        .json::<Value>()
        .expect("completed analysis should return JSON");
    assert!(completed_analysis["analysis"]["constraints"].is_array());
    assert_eq!(
        app.read_first_sse_event(&client, port, &completed_job_id)["eventType"],
        "completed"
    );

    let completed_delete_status = client
        .delete(format!("{base_url}/jobs/{completed_job_id}"))
        .send()
        .expect("completed delete request failed")
        .status();
    assert_eq!(completed_delete_status.as_u16(), 204);

    app.mark_success();
}

fn wait_for_job_snapshot(client: &Client, base_url: &str, id: &str) -> Value {
    let started = Instant::now();
    loop {
        let status: Value = client
            .get(format!("{base_url}/jobs/{id}"))
            .send()
            .expect("job summary request failed")
            .error_for_status()
            .expect("job summary should be successful")
            .json()
            .expect("job summary should return JSON");
        let lifecycle_state = status["lifecycleState"].as_str();
        if matches!(
            lifecycle_state,
            Some("SOLVING" | "PAUSE_REQUESTED" | "PAUSED")
        ) && status["snapshotRevision"].as_u64().is_some()
        {
            return status;
        }
        assert!(
            started.elapsed() < Duration::from_secs(20),
            "timed out waiting for job {id} to expose a retained snapshot: {status:?}"
        );
        thread::sleep(Duration::from_millis(100));
    }
}

fn wait_for_job_state(client: &Client, base_url: &str, id: &str, expected: &str) -> Value {
    let started = Instant::now();
    loop {
        let status: Value = client
            .get(format!("{base_url}/jobs/{id}"))
            .send()
            .expect("job summary request failed")
            .error_for_status()
            .expect("job summary should be successful")
            .json()
            .expect("job summary should return JSON");
        if status["lifecycleState"].as_str() == Some(expected) {
            return status;
        }
        assert!(
            started.elapsed() < Duration::from_secs(20),
            "timed out waiting for job {id} to reach lifecycle state {expected}: {status:?}"
        );
        thread::sleep(Duration::from_millis(100));
    }
}

fn short_runtime_solver_config() -> &'static str {
    r#"[[phases]]
type = "construction_heuristic"
construction_heuristic_type = "first_fit"

[[phases]]
type = "local_search"
[phases.acceptor]
type = "late_acceptance"
late_acceptance_size = 400
[phases.forager]
accepted_count_limit = 4

[termination]
seconds_spent_limit = 5
"#
}
