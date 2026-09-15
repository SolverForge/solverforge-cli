use std::sync::Arc;
use std::time::Duration;

use rmcp::model::Task;
use rmcp::model::{CallToolResult, ContentBlock, ProgressNotificationParam, ProgressToken};
use rmcp::service::Peer;
use rmcp::task_manager::{TaskExit, TaskManager, TaskOptions};
use rmcp::RoleServer;
use serde_json::json;

use crate::api::dto::{lifecycle_state_label, JobSnapshotDto};
use crate::solver::SolverService;

/// How often the solve watcher samples the retained job status.
const STATUS_POLL_INTERVAL: Duration = Duration::from_millis(250);

/// How long completed task state stays addressable through `tasks/get`.
const TASK_TTL: Duration = Duration::from_secs(3_600);

/// Spawns the MCP task that watches a retained solver job until it reaches a
/// terminal lifecycle state. Progress rides the request progress token while
/// the caller is attached; detached clients observe the task through
/// `tasks/get`, whose terminal payload carries the best-solution snapshot.
pub fn spawn_solve_task(
    tasks: &TaskManager,
    solver: Arc<SolverService>,
    peer: Peer<RoleServer>,
    progress_token: Option<ProgressToken>,
    job_id: String,
) -> Task {
    tasks.spawn(
        TaskOptions::new()
            .with_poll_interval_ms(STATUS_POLL_INTERVAL.as_millis() as u64)
            .with_ttl_ms(TASK_TTL.as_millis() as u64)
            .with_status_message("solving"),
        move |ctx| {
            Box::pin(async move {
                loop {
                    tokio::select! {
                        _ = ctx.cancelled() => {
                            // Protocol cancellation maps onto runtime
                            // cancellation; the best solution stays retained.
                            let _ = solver.cancel(&job_id);
                            return Err(TaskExit::Cancelled);
                        }
                        _ = tokio::time::sleep(STATUS_POLL_INTERVAL) => {
                            match solver.get_status(&job_id) {
                                Err(_) => {
                                    // The job was deleted while the task was
                                    // still watching it.
                                    return Ok(CallToolResult::error(vec![
                                        ContentBlock::text(format!(
                                            "job {job_id} was deleted before reaching a terminal state"
                                        )),
                                    ]));
                                }
                                Ok(status) => {
                                    if let Some(token) = &progress_token {
                                        let _ = peer
                                            .notify_progress(
                                                ProgressNotificationParam::new(
                                                    token.clone(),
                                                    status.telemetry.step_count as f64,
                                                )
                                                .with_message(format!(
                                                    "{} (revision {:?})",
                                                    lifecycle_state_label(status.lifecycle_state),
                                                    status.latest_snapshot_revision,
                                                )),
                                            )
                                            .await;
                                    }

                                    match status.lifecycle_state {
                                        solverforge::SolverLifecycleState::Solving
                                        | solverforge::SolverLifecycleState::PauseRequested => {}
                                        solverforge::SolverLifecycleState::Paused => {
                                            ctx.set_status_message("paused");
                                        }
                                        solverforge::SolverLifecycleState::Completed
                                        | solverforge::SolverLifecycleState::Cancelled
                                        | solverforge::SolverLifecycleState::Failed => {
                                            return Ok(terminal_result(
                                                &solver,
                                                &job_id,
                                                status.latest_snapshot_revision,
                                            ));
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            })
        },
    )
}

fn terminal_result(
    solver: &SolverService,
    job_id: &str,
    snapshot_revision: Option<u64>,
) -> CallToolResult {
    let mut payload = json!({
        "jobId": job_id,
    });

    match solver.get_snapshot(job_id, snapshot_revision) {
        Ok(snapshot) => {
            payload["bestScore"] = json!(snapshot.best_score.map(|score| score.to_string()));
            payload["snapshot"] = json!(JobSnapshotDto::from_snapshot(&snapshot));
        }
        Err(error) => {
            // Cancelled or failed jobs may legitimately have no snapshot; the
            // terminal lifecycle state itself is the result then.
            payload["snapshotError"] = json!(error.to_string());
        }
    }

    CallToolResult::structured(payload)
}
