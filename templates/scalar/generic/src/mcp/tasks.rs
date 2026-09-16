use std::sync::Arc;
use std::time::Duration;

use rmcp::model::Task;
use rmcp::model::{CallToolResult, ProgressNotificationParam, ProgressToken};
use rmcp::service::Peer;
use rmcp::task_manager::{TaskExit, TaskManager, TaskOptions};
use rmcp::{ErrorData, RoleServer};

use crate::api::dto::{lifecycle_state_label, JobSnapshotDto};
use crate::solver::SolverService;
use solverforge::{HardSoftScore, SolverStatus};

use super::dto::SolveResultDto;

/// How often the solve watcher samples the retained job status.
const STATUS_POLL_INTERVAL: Duration = Duration::from_millis(250);

/// Spawns the MCP task that watches a retained solver job until it reaches a
/// terminal lifecycle state. Progress rides the request progress token while
/// the caller is attached; detached clients observe the task through
/// `tasks/get`, whose terminal payload carries the best-solution snapshot.
/// Active tasks have no TTL because SolverForge jobs have no maximum run time;
/// task records remain available for the lifetime of this server process.
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
            .with_ttl_ms(None)
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
                                    return Err(TaskExit::Error(ErrorData::internal_error(
                                        format!(
                                            "job {job_id} was deleted before reaching a terminal state"
                                        ),
                                        None,
                                    )));
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
                                        solverforge::SolverLifecycleState::Completed => {
                                            return terminal_result(
                                                &solver,
                                                &job_id,
                                                &status,
                                                status.latest_snapshot_revision,
                                            )
                                            .map_err(TaskExit::Error);
                                        }
                                        solverforge::SolverLifecycleState::Cancelled => {
                                            return Err(TaskExit::Cancelled);
                                        }
                                        solverforge::SolverLifecycleState::Failed => {
                                            return Err(TaskExit::Error(ErrorData::internal_error(
                                                format!("solver job {job_id} failed"),
                                                None,
                                            )));
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
    status: &SolverStatus<HardSoftScore>,
    snapshot_revision: Option<u64>,
) -> Result<CallToolResult, ErrorData> {
    let numeric_id = job_id.parse::<usize>().map_err(|_| {
        ErrorData::internal_error(format!("retained job id '{job_id}' is not numeric"), None)
    })?;

    let mut result = SolveResultDto::from_status(numeric_id, status);
    match solver.get_snapshot(job_id, snapshot_revision) {
        Ok(snapshot) => result.snapshot = Some(JobSnapshotDto::from_snapshot(&snapshot)),
        Err(error) => {
            // Cancelled or failed jobs may legitimately have no snapshot; the
            // terminal lifecycle state itself is the result then.
            result.snapshot_error = Some(error.to_string());
        }
    }

    Ok(CallToolResult::structured(
        serde_json::to_value(result).expect("solve result serializes to JSON"),
    ))
}
