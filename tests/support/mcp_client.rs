//! Real MCP client harness for the generated-app runtime pipeline.
//!
//! These helpers drive a generated MCP-shell binary through the official
//! `rmcp` client over both transports. Sync wrappers keep the test file
//! phase-marked and readable while the async client runs on a private runtime.

use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use rmcp::model::{
    CallToolRequestParams, CallToolResponse, CancelTaskParams, ClientCapabilities, ClientInfo,
    GetTaskParams, Implementation, JsonObject, ProtocolVersion, TaskPayload,
};
use rmcp::service::{ClientLifecycleMode, ClientServiceExt, Peer, RoleClient};
use rmcp::transport::TokioChildProcess;
use rmcp::ServiceExt;
use serde_json::{json, Value};

const DEMO: &str = "STANDARD";

#[derive(Debug)]
pub struct ToolSurface {
    pub names: Vec<String>,
    pub all_annotated: bool,
    pub tools_with_output_schema: usize,
}

#[derive(Debug)]
pub struct TaskFlowReport {
    pub surface: ToolSurface,
    pub states: Vec<String>,
    pub terminal: Value,
    pub cancel_acked: bool,
}

#[derive(Debug)]
pub struct LifecycleReport {
    pub solve_summary: Value,
    pub states: Vec<String>,
    pub operations: Vec<(String, bool)>,
    pub delete_then_status_is_error: bool,
}

/// Drives a task-capable session: list tools, solve as a task, poll to the
/// terminal payload, then cancel. Used for both transports.
pub fn stdio_task_flow(binary: &Path, timeout: Duration) -> TaskFlowReport {
    runtime().block_on(async move {
        let transport = TokioChildProcess::new(tokio::process::Command::new(binary))
            .expect("failed to spawn generated MCP server");
        let client = task_client()
            .serve_with_lifecycle(transport, task_lifecycle())
            .await
            .expect("MCP initialize failed over stdio");
        let report = task_flow(client.peer(), timeout).await;
        let _ = client.cancel().await;
        report
    })
}

pub fn http_task_flow(port: u16, timeout: Duration) -> TaskFlowReport {
    runtime().block_on(async move {
        let transport = rmcp::transport::StreamableHttpClientTransport::from_uri(format!(
            "http://127.0.0.1:{port}/mcp"
        ));
        let client = task_client()
            .serve_with_lifecycle(transport, task_lifecycle())
            .await
            .expect("MCP initialize failed over streamable HTTP");
        let report = task_flow(client.peer(), timeout).await;
        let _ = client.cancel().await;
        report
    })
}

/// Drives a legacy client without task support through the full retained
/// lifecycle using the polling tools.
pub fn stdio_lifecycle_flow(binary: &Path, timeout: Duration) -> LifecycleReport {
    runtime().block_on(async move {
        let transport = TokioChildProcess::new(tokio::process::Command::new(binary))
            .expect("failed to spawn generated MCP server");
        let client = plain_client()
            .serve(transport)
            .await
            .expect("MCP initialize failed over stdio");
        let report = lifecycle_flow(client.peer(), timeout).await;
        let _ = client.cancel().await;
        report
    })
}

pub fn http_lifecycle_flow(port: u16, timeout: Duration) -> LifecycleReport {
    runtime().block_on(async move {
        let transport = rmcp::transport::StreamableHttpClientTransport::from_uri(format!(
            "http://127.0.0.1:{port}/mcp"
        ));
        let client = plain_client()
            .serve(transport)
            .await
            .expect("MCP initialize failed over streamable HTTP");
        let report = lifecycle_flow(client.peer(), timeout).await;
        let _ = client.cancel().await;
        report
    })
}

/// Speaks raw JSON-RPC to the generated binary over stdio and returns every
/// stdout line observed while a solve runs. The stdio channel is the MCP
/// transport, so every returned line must parse as JSON.
pub fn raw_stdio_stdout_lines(binary: &Path, timeout: Duration) -> Vec<String> {
    let mut child = Command::new(binary)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("failed to spawn generated MCP server");

    let stdout = child.stdout.take().expect("stdout piped");
    let mut stdin = child.stdin.take().expect("stdin piped");

    let (sender, receiver) = mpsc::channel::<String>();
    let reader = thread::spawn(move || {
        let reader = BufReader::new(stdout);
        for line in reader.lines() {
            match line {
                Ok(line) => {
                    if sender.send(line).is_err() {
                        return;
                    }
                }
                Err(_) => return,
            }
        }
    });

    let initialize = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {
            "protocolVersion": "2025-11-25",
            "capabilities": {},
            "clientInfo": { "name": "raw-stdout-check", "version": "0.1.0" }
        }
    });
    let initialized = json!({
        "jsonrpc": "2.0",
        "method": "notifications/initialized"
    });
    let solve = json!({
        "jsonrpc": "2.0",
        "id": 2,
        "method": "tools/call",
        "params": { "name": "solve", "arguments": { "demo": DEMO } }
    });

    for message in [initialize, initialized, solve] {
        writeln!(stdin, "{message}").expect("failed to write JSON-RPC line");
        stdin.flush().expect("failed to flush JSON-RPC line");
    }

    let mut lines = Vec::new();
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        match receiver.recv_timeout(Duration::from_millis(200)) {
            Ok(line) => {
                let saw_solve = line.trim().starts_with('{')
                    && serde_json::from_str::<Value>(line.trim())
                        .ok()
                        .is_some_and(|value| value.get("id").and_then(Value::as_i64) == Some(2));
                lines.push(line);
                if saw_solve {
                    break;
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) => continue,
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }

    let _ = child.kill();
    let _ = child.wait();
    let _ = reader.join();
    lines
}

async fn task_flow(peer: &Peer<RoleClient>, timeout: Duration) -> TaskFlowReport {
    let surface = tool_surface(peer).await;

    let response = peer
        .call_tool_once(call("solve", demo_arguments()))
        .await
        .expect("solve tool call failed");
    let task_id = match response {
        CallToolResponse::Task(created) => created.task.task_id.to_string(),
        other => panic!("task-capable client must receive a task handle, got {other:?}"),
    };

    let mut states = Vec::new();
    let deadline = Instant::now() + timeout;
    let terminal = loop {
        let status = peer
            .get_task(GetTaskParams::new(task_id.clone()))
            .await
            .expect("tasks/get failed");
        states.push(format!("{:?}", status.task.task.status));
        if let TaskPayload::Completed { result } = &status.task.payload {
            break result
                .get("structuredContent")
                .cloned()
                .unwrap_or(Value::Null);
        }
        assert!(
            Instant::now() < deadline,
            "solve task never completed; observed states: {states:?}"
        );
        tokio::time::sleep(Duration::from_millis(50)).await;
    };

    let cancel_acked = peer
        .cancel_task(CancelTaskParams::new(task_id))
        .await
        .is_ok();

    TaskFlowReport {
        surface,
        states,
        terminal,
        cancel_acked,
    }
}

async fn lifecycle_flow(peer: &Peer<RoleClient>, timeout: Duration) -> LifecycleReport {
    let response = peer
        .call_tool_once(call("solve", demo_arguments()))
        .await
        .expect("solve tool call failed");
    let solve_summary = match response {
        CallToolResponse::Complete(result) => result.structured_content.unwrap_or(Value::Null),
        other => panic!("legacy client must receive an immediate summary, got {other:?}"),
    };
    let job_id = solve_summary["jobId"]
        .as_str()
        .expect("solve summary must carry a jobId")
        .to_string();

    let mut states = Vec::new();
    states.push(lifecycle_state(peer, &job_id).await);
    assert_eq!(
        states.last().map(String::as_str),
        Some("SOLVING"),
        "expected the job to start solving"
    );

    let mut operations = Vec::new();

    let pause = complete_result(peer, "pause", job_arguments(&job_id)).await;
    operations.push(("pause".to_string(), accepted(&pause)));
    let paused = wait_for_state(peer, &job_id, "PAUSED", timeout).await;
    states.push(paused);

    let resume = complete_result(peer, "resume", job_arguments(&job_id)).await;
    operations.push(("resume".to_string(), accepted(&resume)));
    let solving = wait_for_state(peer, &job_id, "SOLVING", timeout).await;
    states.push(solving);

    let cancel = complete_result(peer, "cancel", job_arguments(&job_id)).await;
    operations.push(("cancel".to_string(), accepted(&cancel)));
    let cancelled = wait_for_state(peer, &job_id, "CANCELLED", timeout).await;
    states.push(cancelled);

    let delete = complete_result(peer, "delete", job_arguments(&job_id)).await;
    operations.push(("delete".to_string(), accepted(&delete)));

    let delete_then_status_is_error = peer
        .call_tool_once(call("get_status", job_arguments(&job_id)))
        .await
        .is_err();

    LifecycleReport {
        solve_summary,
        states,
        operations,
        delete_then_status_is_error,
    }
}

async fn tool_surface(peer: &Peer<RoleClient>) -> ToolSurface {
    let tools = peer
        .list_tools(Default::default())
        .await
        .expect("tools/list failed");
    ToolSurface {
        names: tools
            .tools
            .iter()
            .map(|tool| tool.name.to_string())
            .collect(),
        all_annotated: tools.tools.iter().all(|tool| tool.annotations.is_some()),
        tools_with_output_schema: tools
            .tools
            .iter()
            .filter(|tool| tool.output_schema.is_some())
            .count(),
    }
}

async fn complete_result(peer: &Peer<RoleClient>, tool: &str, arguments: JsonObject) -> Value {
    match peer.call_tool_once(call(tool, arguments)).await {
        Ok(CallToolResponse::Complete(result)) => result.structured_content.unwrap_or(Value::Null),
        other => panic!("{tool} tool call failed: {other:?}"),
    }
}

async fn lifecycle_state(peer: &Peer<RoleClient>, job_id: &str) -> String {
    let status = complete_result(peer, "get_status", job_arguments(job_id)).await;
    status["lifecycleState"]
        .as_str()
        .expect("status must carry lifecycleState")
        .to_string()
}

async fn wait_for_state(
    peer: &Peer<RoleClient>,
    job_id: &str,
    expected: &str,
    timeout: Duration,
) -> String {
    let deadline = Instant::now() + timeout;
    loop {
        let state = lifecycle_state(peer, job_id).await;
        if state == expected {
            return state;
        }
        assert!(
            Instant::now() < deadline,
            "timed out waiting for job {job_id} to reach {expected}; last state {state}"
        );
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

fn accepted(result: &Value) -> bool {
    result["accepted"].as_bool().unwrap_or(false)
}

fn call(name: &str, arguments: JsonObject) -> CallToolRequestParams {
    CallToolRequestParams::new(name.to_string()).with_arguments(arguments)
}

fn demo_arguments() -> JsonObject {
    JsonObject::from_iter([("demo".to_string(), json!(DEMO))])
}

fn job_arguments(job_id: &str) -> JsonObject {
    JsonObject::from_iter([("jobId".to_string(), json!(job_id))])
}

fn task_client() -> ClientInfo {
    let mut info = ClientInfo::default();
    info.capabilities = ClientCapabilities::builder().enable_tasks().build();
    info.client_info = Implementation::new("solverforge-mcp-pipeline", "0.1.0");
    info
}

fn plain_client() -> ClientInfo {
    let mut info = ClientInfo::default();
    info.client_info = Implementation::new("solverforge-mcp-pipeline-legacy", "0.1.0");
    info
}

fn task_lifecycle() -> ClientLifecycleMode {
    ClientLifecycleMode::Auto {
        preferred_versions: vec![ProtocolVersion::V_2026_07_28],
        legacy_version: Some(ProtocolVersion::V_2025_11_25),
    }
}

fn runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("failed to build tokio runtime")
}
