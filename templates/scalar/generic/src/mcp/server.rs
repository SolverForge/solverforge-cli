use std::sync::Arc;

use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::handler::server::wrapper::{Json, Parameters};
use rmcp::model::{
    CallToolRequestParams, CallToolResponse, CancelTaskParams, CreateTaskResult, GetTaskParams,
    GetTaskResult, Implementation, ServerCapabilities, ServerInfo, UpdateTaskParams,
};
use rmcp::service::RequestContext;
use rmcp::task_manager::TaskManager;
use rmcp::RoleServer;
use rmcp::{tool, tool_handler, tool_router, ErrorData, ServerHandler};
use schemars::JsonSchema;
use serde::Deserialize;

use crate::api::dto::{
    analysis_response, JobAnalysisDto, JobSnapshotDto, JobSummaryDto, JobTelemetryDetailDto,
    PlanDto,
};
use crate::api::telemetry::CandidateTraceDto;
use crate::data::{generate, DemoData};
use crate::solver::SolverService;

use super::tasks;

/// Input for `solve`: a full plan or a demo data size, never both.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SolveRequest {
    /// Complete plan instance to optimize.
    pub plan: Option<PlanDto>,
    /// Demo data size to optimize instead of a plan (small, standard, or large).
    pub demo: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct JobRequest {
    /// Retained solver job identifier.
    pub job_id: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SnapshotRequest {
    /// Retained solver job identifier.
    pub job_id: String,
    /// Specific snapshot revision; omit for the latest retained snapshot.
    pub snapshot_revision: Option<u64>,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct DemoDataRequest {
    /// Demo data size (small, standard, or large).
    pub demo: String,
}

#[derive(Debug, schemars::JsonSchema, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JobOpResult {
    /// Retained solver job identifier.
    pub job_id: String,
    /// Whether the runtime accepted the operation.
    pub accepted: bool,
}

/// MCP delivery surface for this planning application. The handler projects
/// the retained solver job lifecycle onto agent-facing tools: task-capable
/// clients receive a task handle for `solve`, every other client receives an
/// immediate job summary and drives the lifecycle through the polling tools.
pub struct SolverMcp {
    tool_router: ToolRouter<Self>,
    tasks: TaskManager,
    solver: Arc<SolverService>,
}

impl SolverMcp {
    pub fn new(solver: Arc<SolverService>, tasks: TaskManager) -> Self {
        Self {
            tool_router: Self::tool_router(),
            tasks,
            solver,
        }
    }

    fn start_from_request(&self, request: SolveRequest) -> Result<String, ErrorData> {
        match (request.plan, request.demo) {
            (Some(plan), None) => {
                let domain = plan.to_domain().map_err(|error| {
                    ErrorData::invalid_params(format!("invalid plan: {error}"), None)
                })?;
                self.solver.start_job(domain).map_err(solver_error)
            }
            (None, Some(demo)) => {
                let size: DemoData = demo.parse().map_err(|_| {
                    ErrorData::invalid_params(format!("unknown demo data size '{demo}'"), None)
                })?;
                self.solver.start_job(generate(size)).map_err(solver_error)
            }
            (Some(_), Some(_)) | (None, None) => Err(ErrorData::invalid_params(
                "provide exactly one of `plan` or `demo`",
                None,
            )),
        }
    }

    fn job_summary(&self, job_id: &str) -> Result<JobSummaryDto, ErrorData> {
        let status = self.solver.get_status(job_id).map_err(solver_error)?;
        let numeric_id = parse_job_id(job_id)?;
        Ok(JobSummaryDto::from_status(numeric_id, &status))
    }
}

#[tool_router]
impl SolverMcp {
    #[tool(
        description = "Start a retained solver job. Returns an MCP task handle when the client negotiates task support, otherwise returns the job summary immediately and the job is driven through the polling tools.",
        annotations(
            title = "Solve",
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = false,
            open_world_hint = false
        )
    )]
    fn solve(
        &self,
        Parameters(request): Parameters<SolveRequest>,
    ) -> Result<Json<JobSummaryDto>, ErrorData> {
        let job_id = self.start_from_request(request)?;
        Ok(Json(self.job_summary(&job_id)?))
    }

    #[tool(
        description = "List the available demo data sizes for this planning application.",
        annotations(
            title = "List demo data",
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    fn list_demo_data(&self) -> Json<Vec<String>> {
        Json(
            DemoData::available_demo_data()
                .iter()
                .map(|demo| demo.id().to_string())
                .collect(),
        )
    }

    #[tool(
        description = "Return the generated plan instance for one demo data size.",
        annotations(
            title = "Get demo data",
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    fn get_demo_data(
        &self,
        Parameters(request): Parameters<DemoDataRequest>,
    ) -> Result<Json<PlanDto>, ErrorData> {
        let size: DemoData = request.demo.parse().map_err(|_| {
            ErrorData::invalid_params(format!("unknown demo data size '{}'", request.demo), None)
        })?;
        Ok(Json(PlanDto::from_plan(&generate(size))))
    }

    #[tool(
        description = "Return the live status of a retained solver job, including the complete compact telemetry aggregate.",
        annotations(
            title = "Job status",
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    fn get_status(
        &self,
        Parameters(request): Parameters<JobRequest>,
    ) -> Result<Json<JobSummaryDto>, ErrorData> {
        Ok(Json(self.job_summary(&request.job_id)?))
    }

    #[tool(
        description = "Return the best solution snapshot of a retained solver job.",
        annotations(
            title = "Best solution",
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    fn get_best_solution(
        &self,
        Parameters(request): Parameters<SnapshotRequest>,
    ) -> Result<Json<JobSnapshotDto>, ErrorData> {
        let snapshot = self
            .solver
            .get_snapshot(&request.job_id, request.snapshot_revision)
            .map_err(solver_error)?;
        Ok(Json(JobSnapshotDto::from_snapshot(&snapshot)))
    }

    #[tool(
        description = "Return the snapshot-bound score analysis of a retained solver job, broken down per constraint.",
        annotations(
            title = "Analyze solution",
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    fn analyze_solution(
        &self,
        Parameters(request): Parameters<SnapshotRequest>,
    ) -> Result<Json<JobAnalysisDto>, ErrorData> {
        let snapshot_analysis = self
            .solver
            .analyze_snapshot(&request.job_id, request.snapshot_revision)
            .map_err(solver_error)?;
        let analysis = analysis_response(&snapshot_analysis.analysis);
        Ok(Json(JobAnalysisDto::from_snapshot_analysis(
            &snapshot_analysis,
            analysis,
        )))
    }

    #[tool(
        description = "Return the retained status telemetry of a solver job, including bounded candidate pulls when candidate tracing is enabled.",
        annotations(
            title = "Job telemetry",
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    fn get_telemetry(
        &self,
        Parameters(request): Parameters<JobRequest>,
    ) -> Result<Json<JobTelemetryDetailDto>, ErrorData> {
        let detail = self
            .solver
            .get_telemetry_detail(&request.job_id)
            .map_err(solver_error)?;
        Ok(Json(JobTelemetryDetailDto::from_runtime(&detail)))
    }

    #[tool(
        description = "Return only the bounded, retained candidate-pull trace of a solver job. Empty unless candidate tracing is enabled through candidate_trace.max_entries.",
        annotations(
            title = "Candidate trace",
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    fn get_candidate_trace(
        &self,
        Parameters(request): Parameters<JobRequest>,
    ) -> Result<Json<Option<CandidateTraceDto>>, ErrorData> {
        let detail = self
            .solver
            .get_telemetry_detail(&request.job_id)
            .map_err(solver_error)?;
        Ok(Json(
            detail
                .candidate_trace
                .as_ref()
                .map(CandidateTraceDto::from_runtime),
        ))
    }

    #[tool(
        description = "Pause a running solver job. Resume with the resume tool; the runtime continues from the retained checkpoint.",
        annotations(
            title = "Pause job",
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    fn pause(
        &self,
        Parameters(request): Parameters<JobRequest>,
    ) -> Result<Json<JobOpResult>, ErrorData> {
        let accepted = self.solver.pause(&request.job_id).is_ok();
        Ok(Json(JobOpResult {
            job_id: request.job_id,
            accepted,
        }))
    }

    #[tool(
        description = "Resume a paused solver job from its retained checkpoint.",
        annotations(
            title = "Resume job",
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    fn resume(
        &self,
        Parameters(request): Parameters<JobRequest>,
    ) -> Result<Json<JobOpResult>, ErrorData> {
        let accepted = self.solver.resume(&request.job_id).is_ok();
        Ok(Json(JobOpResult {
            job_id: request.job_id,
            accepted,
        }))
    }

    #[tool(
        description = "Cancel a solver job. The best solution found so far stays retained and readable.",
        annotations(
            title = "Cancel job",
            read_only_hint = false,
            destructive_hint = true,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    fn cancel(
        &self,
        Parameters(request): Parameters<JobRequest>,
    ) -> Result<Json<JobOpResult>, ErrorData> {
        let accepted = self.solver.cancel(&request.job_id).is_ok();
        Ok(Json(JobOpResult {
            job_id: request.job_id,
            accepted,
        }))
    }

    #[tool(
        description = "Delete a terminal solver job and its retained artifacts. This is irreversible cleanup and the only way to remove terminal jobs.",
        annotations(
            title = "Delete job",
            read_only_hint = false,
            destructive_hint = true,
            idempotent_hint = false,
            open_world_hint = false
        )
    )]
    fn delete(
        &self,
        Parameters(request): Parameters<JobRequest>,
    ) -> Result<Json<JobOpResult>, ErrorData> {
        let accepted = self.solver.delete(&request.job_id).is_ok();
        Ok(Json(JobOpResult {
            job_id: request.job_id,
            accepted,
        }))
    }
}

#[tool_handler]
impl ServerHandler for SolverMcp {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(
            ServerCapabilities::builder()
                .enable_tools()
                .enable_tasks()
                .build(),
        )
        .with_server_info(Implementation::new(
            env!("CARGO_PKG_NAME"),
            env!("CARGO_PKG_VERSION"),
        ))
        .with_instructions(concat!(
            "SolverForge planning server. Solve optimization problems over demo data or ",
            "full plan instances, inspect live status, telemetry, and score analysis, ",
            "and drive the retained job lifecycle: pause, resume, cancel, and delete."
        ))
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        context: RequestContext<RoleServer>,
    ) -> Result<CallToolResponse, ErrorData> {
        tracing::info!(tool = %request.name, "mcp tool invocation");

        let client_supports_tasks = context
            .client_capabilities()
            .is_some_and(|caps| caps.supports_tasks());

        if request.name == "solve" && client_supports_tasks {
            let solve_request: SolveRequest = match &request.arguments {
                Some(arguments) => {
                    serde_json::from_value(arguments.clone().into()).map_err(|error| {
                        ErrorData::invalid_params(format!("invalid solve request: {error}"), None)
                    })?
                }
                None => SolveRequest {
                    plan: None,
                    demo: None,
                },
            };
            let job_id = self.start_from_request(solve_request)?;
            let task = tasks::spawn_solve_task(
                &self.tasks,
                Arc::clone(&self.solver),
                context.peer.clone(),
                context.meta.get_progress_token(),
                job_id,
            );
            return Ok(CallToolResponse::Task(CreateTaskResult::new(task)));
        }

        let tool_context =
            rmcp::handler::server::tool::ToolCallContext::new(self, request, context);
        self.tool_router.call(tool_context).await
    }

    async fn get_task(
        &self,
        request: GetTaskParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<GetTaskResult, ErrorData> {
        Ok(GetTaskResult::new(self.tasks.get_task(&request.task_id)?))
    }

    async fn update_task(
        &self,
        request: UpdateTaskParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<(), ErrorData> {
        self.tasks
            .update_task(&request.task_id, request.input_responses)
    }

    async fn cancel_task(
        &self,
        request: CancelTaskParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<(), ErrorData> {
        // Cancellation is cooperative: the solve watcher observes the task
        // cancellation and cancels the underlying solver job.
        self.tasks.cancel_task(&request.task_id)
    }
}

fn parse_job_id(job_id: &str) -> Result<usize, ErrorData> {
    job_id
        .parse::<usize>()
        .map_err(|_| ErrorData::invalid_params(format!("invalid job id '{job_id}'"), None))
}

fn solver_error(error: solverforge::SolverManagerError) -> ErrorData {
    let message = error.to_string();
    match error {
        solverforge::SolverManagerError::JobNotFound { .. }
        | solverforge::SolverManagerError::SnapshotNotFound { .. } => {
            ErrorData::invalid_params(message, None)
        }
        solverforge::SolverManagerError::NoFreeJobSlots => ErrorData::internal_error(message, None),
        solverforge::SolverManagerError::InvalidStateTransition { .. }
        | solverforge::SolverManagerError::NoSnapshotAvailable { .. } => {
            ErrorData::invalid_params(message, None)
        }
    }
}
