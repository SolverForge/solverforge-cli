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
    assert!(demo["resources"].as_array().map(|rows| rows.len() >= 3) == Some(true));
    assert!(demo["tasks"].as_array().map(|rows| rows.len() >= 6) == Some(true));

    let job_id = app.create_schedule_from_demo(&client, port);

    let schedules: Value = client
        .get(format!("{base_url}/schedules"))
        .send()
        .expect("list schedules request failed")
        .error_for_status()
        .expect("list schedules should be successful")
        .json()
        .expect("list schedules should return JSON");
    assert!(schedules
        .as_array()
        .expect("schedules should be an array")
        .iter()
        .any(|value| value.as_str() == Some(&job_id)));

    let schedule: Value = client
        .get(format!("{base_url}/schedules/{job_id}"))
        .send()
        .expect("get schedule request failed")
        .error_for_status()
        .expect("get schedule should be successful")
        .json()
        .expect("schedule should return JSON");
    assert!(
        schedule["resources"]
            .as_array()
            .map(|rows| !rows.is_empty())
            == Some(true),
        "expected seeded resources in solved payload"
    );
    assert!(
        schedule["tasks"].as_array().map(|rows| !rows.is_empty()) == Some(true),
        "expected seeded tasks in solved payload"
    );

    let status: Value = client
        .get(format!("{base_url}/schedules/{job_id}/status"))
        .send()
        .expect("status request failed")
        .error_for_status()
        .expect("status should be successful")
        .json()
        .expect("status should return JSON");
    assert!(status["solverStatus"].is_string());
    assert!(status.get("currentScore").is_some());
    assert!(status.get("bestScore").is_some());

    let event_types = app.read_sse_event_types(&client, port, &job_id, 3);
    assert!(
        !event_types.is_empty(),
        "expected at least one SSE event type from the generated app"
    );
    assert!(
        event_types.iter().all(|event_type| matches!(
            event_type.as_str(),
            "progress" | "best_solution" | "finished"
        )),
        "unexpected event types: {:?}",
        event_types
    );

    let analyze: Value = client
        .get(format!("{base_url}/schedules/{job_id}/analyze"))
        .send()
        .expect("analyze request failed")
        .error_for_status()
        .expect("analyze should be successful")
        .json()
        .expect("analyze should return JSON");
    assert!(analyze.get("score").is_some());
    assert!(analyze["constraints"].is_array());

    let delete_status = client
        .delete(format!("{base_url}/schedules/{job_id}"))
        .send()
        .expect("delete request failed")
        .status();
    assert_eq!(delete_status.as_u16(), 204);

    let stopped_status = wait_for_schedule_status(&client, &base_url, &job_id, "NOT_SOLVING");
    assert_eq!(stopped_status["solverStatus"], "NOT_SOLVING");

    let stopped_schedule: Value = client
        .get(format!("{base_url}/schedules/{job_id}"))
        .send()
        .expect("get stopped schedule request failed")
        .error_for_status()
        .expect("stopped schedule should remain available")
        .json()
        .expect("stopped schedule should return JSON");
    assert!(
        stopped_schedule["tasks"]
            .as_array()
            .map(|rows| !rows.is_empty())
            == Some(true),
        "expected stopped schedule snapshot to remain available"
    );

    let resumed: Value = client
        .post(format!("{base_url}/schedules"))
        .json(&stopped_schedule)
        .send()
        .expect("resume create schedule request failed")
        .error_for_status()
        .expect("resume create schedule should succeed")
        .json()
        .expect("resume create schedule should return JSON");
    let resumed_id = resumed["id"]
        .as_str()
        .expect("resumed schedule id should be a string");
    assert_ne!(resumed_id, job_id);

    let resumed_event_types = app.read_sse_event_types(&client, port, resumed_id, 3);
    assert!(
        !resumed_event_types.is_empty(),
        "expected resumed solve to emit SSE events"
    );

    let resumed_delete_status = client
        .delete(format!("{base_url}/schedules/{resumed_id}"))
        .send()
        .expect("resume delete request failed")
        .status();
    assert_eq!(resumed_delete_status.as_u16(), 204);

    let resumed_status = wait_for_schedule_status(&client, &base_url, resumed_id, "NOT_SOLVING");
    assert_eq!(resumed_status["solverStatus"], "NOT_SOLVING");

    app.mark_success();
}

fn wait_for_schedule_status(client: &Client, base_url: &str, id: &str, expected: &str) -> Value {
    let started = Instant::now();
    loop {
        let status: Value = client
            .get(format!("{base_url}/schedules/{id}/status"))
            .send()
            .expect("status request failed")
            .error_for_status()
            .expect("status should be successful")
            .json()
            .expect("status should return JSON");
        if status["solverStatus"].as_str() == Some(expected) {
            return status;
        }
        assert!(
            started.elapsed() < Duration::from_secs(10),
            "timed out waiting for schedule {id} to reach status {expected}: {status:?}"
        );
        thread::sleep(Duration::from_millis(100));
    }
}
