//! Phase-marked runtime pipeline for generated MCP-shell applications.
//!
//! Each scenario scaffolds and models a real app through the real CLI, builds
//! it, and drives the generated MCP server with the official `rmcp` client
//! over stdio and stateless Streamable HTTP. The MCP client is the delivery
//! surface's real consumer, so it is the acceptance test for this shell.

#[path = "support/app_harness.rs"]
mod app_harness;
#[path = "support/dependency_overrides.rs"]
mod dependency_overrides;
#[path = "support/mcp_client.rs"]
mod mcp_client;
#[path = "support/mcp_generated_app.rs"]
mod mcp_generated_app;

use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use app_harness::short_solver_config;
use mcp_client::{
    http_lifecycle_flow, http_task_flow, raw_stdio_stdout_lines, stdio_lifecycle_flow,
    stdio_task_flow, ToolSurface,
};
use mcp_generated_app::McpGeneratedApp;

const FLOW_TIMEOUT: Duration = Duration::from_secs(60);

fn test_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

/// Scaffolds an MCP-shell app, models a mixed scalar-plus-list problem, and
/// generates the compiler-owned sample data through the real CLI.
fn seed_modeled_mcp_app(test_name: &str) -> McpGeneratedApp {
    let mut app = McpGeneratedApp::new(test_name, test_name);
    app.scaffold_mcp();
    app.run_cli("Generate fact resources", &["generate", "fact", "resource"]);
    app.run_cli("Generate entity tasks", &["generate", "entity", "task"]);
    app.run_cli(
        "Generate scalar variable",
        &[
            "generate",
            "variable",
            "resource_idx",
            "--entity",
            "Task",
            "--kind",
            "scalar",
            "--range",
            "resources",
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
    app.run_cli("Generate demo data", &["generate", "data"]);
    app.write_file("solver.toml", short_solver_config());
    app.cargo_build("Build generated MCP app");
    app
}

fn assert_tool_surface(surface: &ToolSurface) {
    let mut names = surface.names.clone();
    names.sort();
    let mut expected = vec![
        "analyze_solution",
        "cancel",
        "delete",
        "get_best_solution",
        "get_candidate_trace",
        "get_demo_data",
        "get_status",
        "get_telemetry",
        "list_demo_data",
        "pause",
        "resume",
        "solve",
    ];
    expected.sort();
    assert_eq!(names, expected, "generated MCP tool surface changed");
    assert!(
        surface.all_annotated,
        "every generated tool must carry annotations"
    );
    assert!(
        surface.tools_with_output_schema >= 8,
        "expected typed structured outputs on the data tools, got {}",
        surface.tools_with_output_schema
    );
}

/// Asserts the terminal task payload carries a real mixed solution: scalar
/// assignment plus complete list placement.
fn assert_mixed_solution(terminal: &serde_json::Value) {
    assert!(
        terminal["jobId"].is_string(),
        "terminal payload must carry the job id: {terminal:?}"
    );
    let solution = &terminal["snapshot"]["solution"];
    let tasks = solution["tasks"]
        .as_array()
        .unwrap_or_else(|| panic!("solution should carry tasks: {solution:?}"));
    assert!(
        tasks.iter().any(|task| task["resource_idx"].is_number()),
        "terminal snapshot should assign at least one scalar variable: {solution:?}"
    );
    let containers = solution["containers"]
        .as_array()
        .unwrap_or_else(|| panic!("solution should carry containers: {solution:?}"));
    let assigned_items = containers
        .iter()
        .map(|container| {
            container["item_order"]
                .as_array()
                .map(Vec::len)
                .unwrap_or_default()
        })
        .sum::<usize>();
    let items = solution["items"]
        .as_array()
        .unwrap_or_else(|| panic!("solution should carry items: {solution:?}"))
        .len();
    assert_eq!(
        assigned_items, items,
        "terminal snapshot should place every list element: {solution:?}"
    );
}

fn assert_lifecycle_sequence(report: &mcp_client::LifecycleReport) {
    assert!(
        report.solve_summary["jobId"].is_string(),
        "legacy solve must return an immediate job summary: {:?}",
        report.solve_summary
    );
    assert_eq!(
        report.solve_summary["lifecycleState"], "SOLVING",
        "legacy solve should start solving immediately: {:?}",
        report.solve_summary
    );
    assert_eq!(
        report.states,
        vec!["SOLVING", "PAUSED", "SOLVING", "CANCELLED"],
        "unexpected lifecycle sequence: {:?}",
        report.states
    );
    assert!(
        report.operations.iter().all(|(_, accepted)| *accepted),
        "every lifecycle operation should be accepted: {:?}",
        report.operations
    );
    assert!(
        report.delete_then_status_is_error,
        "status after delete should be an error"
    );
}

#[test]
fn mcp_stdio_pipeline() {
    let _guard = test_lock()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());

    // PHASE 1: scaffold, model, and build a mixed MCP-shell app.
    let mut app = seed_modeled_mcp_app("mcp_stdio_pipeline");

    // PHASE 2: task-capable client over stdio.
    app.phase("Drive task-backed solve over stdio");
    let report = stdio_task_flow(&app.binary_path(), FLOW_TIMEOUT);
    assert_tool_surface(&report.surface);
    assert!(
        report
            .states
            .iter()
            .any(|state| state.contains("Completed")),
        "solve task never completed: {:?}",
        report.states
    );
    assert_mixed_solution(&report.terminal);
    assert!(
        report.cancel_acked,
        "cancelling the terminal task should be acknowledged"
    );

    // PHASE 3: legacy client over stdio drives the full lifecycle.
    app.phase("Drive retained lifecycle over stdio");
    assert_lifecycle_sequence(&stdio_lifecycle_flow(&app.binary_path(), FLOW_TIMEOUT));

    // PHASE 4: stdio stdout must stay pure JSON-RPC while a solve runs.
    app.phase("Verify stdio stdout purity during a solve");
    let lines = raw_stdio_stdout_lines(&app.binary_path(), Duration::from_secs(30));
    assert!(
        lines.iter().any(|line| line.contains("\"id\":2")),
        "raw stdio session never saw the solve response: {lines:?}"
    );
    for line in &lines {
        if line.trim().is_empty() {
            continue;
        }
        serde_json::from_str::<serde_json::Value>(line.trim())
            .unwrap_or_else(|error| panic!("non-JSON stdio stdout line {line:?}: {error}"));
    }

    app.mark_success();
}

#[test]
fn mcp_http_pipeline() {
    let _guard = test_lock()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());

    // PHASE 1: scaffold, model, and build a mixed MCP-shell app.
    let mut app = seed_modeled_mcp_app("mcp_http_pipeline");

    // PHASE 2: boot the stateless Streamable HTTP transport.
    let port = app.start_http_server();

    // PHASE 3: task-capable client over Streamable HTTP.
    app.phase("Drive task-backed solve over Streamable HTTP");
    let report = http_task_flow(port, FLOW_TIMEOUT);
    assert_tool_surface(&report.surface);
    assert!(
        report
            .states
            .iter()
            .any(|state| state.contains("Completed")),
        "solve task never completed: {:?}",
        report.states
    );
    assert_mixed_solution(&report.terminal);

    // PHASE 4: legacy client over Streamable HTTP drives the full lifecycle.
    app.phase("Drive retained lifecycle over Streamable HTTP");
    assert_lifecycle_sequence(&http_lifecycle_flow(port, FLOW_TIMEOUT));

    app.mark_success();
}
