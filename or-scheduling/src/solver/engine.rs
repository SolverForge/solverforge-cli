use parking_lot::RwLock;
use rand::Rng;
use solverforge::prelude::*;
use solverforge::TypedScoreDirector;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::oneshot;
use tracing::info;

use super::config::SolverConfig;
use super::service::SolveJob;
use crate::constraints::create_constraints;
use crate::domain::Plan;

const LATE_ACCEPTANCE_SIZE: usize = 400;

pub fn solve_blocking(
    job: Arc<RwLock<SolveJob>>,
    mut stop_rx: oneshot::Receiver<()>,
    config: SolverConfig,
) {
    let initial_plan = job.read().plan.clone();
    let job_id = job.read().id.clone();
    let solve_start = Instant::now();

    info!(
        job_id = %job_id,
        tasks = initial_plan.tasks.len(),
        resources = initial_plan.resources.len(),
        "Starting solver"
    );

    let constraints = create_constraints();
    let mut director = TypedScoreDirector::new(initial_plan, constraints);

    // Phase 1: Construction — assign each task to resources round-robin
    let n_tasks = director.working_solution().tasks.len();
    let n_resources = director.working_solution().resources.len();
    let _ = director.calculate_score();

    if n_resources > 0 {
        for task_idx in 0..n_tasks {
            if director.working_solution().tasks[task_idx].resource_idx.is_some() {
                continue;
            }
            director.before_variable_changed(0, task_idx);
            director.working_solution_mut().tasks[task_idx].resource_idx =
                Some(task_idx % n_resources);
            director.after_variable_changed(0, task_idx);
        }
    }

    let mut current_score = director.get_score();
    let mut best_score = current_score;
    update_job(&job, &director, current_score);

    info!(%current_score, "Construction complete");

    // Phase 2: Late Acceptance local search
    let mut late_scores = vec![current_score; LATE_ACCEPTANCE_SIZE];
    let mut step: u64 = 0;
    let mut rng = rand::thread_rng();
    let mut last_improvement_time = solve_start;
    let mut last_improvement_step: u64 = 0;

    loop {
        let elapsed = solve_start.elapsed();
        if config.should_terminate(
            elapsed,
            step,
            last_improvement_time.elapsed(),
            step - last_improvement_step,
        ) {
            break;
        }
        if stop_rx.try_recv().is_ok() {
            info!("Solving stopped by user");
            break;
        }

        let n_tasks = director.working_solution().tasks.len();
        let n_resources = director.working_solution().resources.len();
        if n_tasks == 0 || n_resources == 0 {
            break;
        }

        let task_idx = rng.gen_range(0..n_tasks);
        let current_resource = director.working_solution().tasks[task_idx].resource_idx;
        let new_resource = rng.gen_range(0..n_resources);
        if current_resource == Some(new_resource) {
            continue;
        }

        let old = apply_move(&mut director, task_idx, Some(new_resource));
        let new_score = director.get_score();

        let late_idx = (step as usize) % LATE_ACCEPTANCE_SIZE;
        let late_score = late_scores[late_idx];

        if new_score >= current_score || new_score >= late_score {
            current_score = new_score;
            late_scores[late_idx] = new_score;
            if new_score > best_score {
                best_score = new_score;
                last_improvement_time = Instant::now();
                last_improvement_step = step;
                update_job(&job, &director, current_score);
            }
        } else {
            undo_move(&mut director, task_idx, old);
        }
        step += 1;
    }

    info!(%current_score, steps = step, "Solving ended");
    finish_job(&job, &director, current_score);
}

fn apply_move(
    director: &mut TypedScoreDirector<Plan, impl ConstraintSet<Plan, HardSoftScore>>,
    task_idx: usize,
    new_resource: Option<usize>,
) -> Option<usize> {
    let old = director.working_solution().tasks[task_idx].resource_idx;
    director.before_variable_changed(0, task_idx);
    director.working_solution_mut().tasks[task_idx].resource_idx = new_resource;
    director.after_variable_changed(0, task_idx);
    old
}

fn undo_move(
    director: &mut TypedScoreDirector<Plan, impl ConstraintSet<Plan, HardSoftScore>>,
    task_idx: usize,
    old: Option<usize>,
) {
    director.before_variable_changed(0, task_idx);
    director.working_solution_mut().tasks[task_idx].resource_idx = old;
    director.after_variable_changed(0, task_idx);
}

fn update_job(
    job: &Arc<RwLock<SolveJob>>,
    director: &TypedScoreDirector<Plan, impl ConstraintSet<Plan, HardSoftScore>>,
    score: HardSoftScore,
) {
    let mut g = job.write();
    g.plan = director.clone_working_solution();
    g.plan.score = Some(score);
}

fn finish_job(
    job: &Arc<RwLock<SolveJob>>,
    director: &TypedScoreDirector<Plan, impl ConstraintSet<Plan, HardSoftScore>>,
    score: HardSoftScore,
) {
    use super::service::SolverStatus;
    let mut g = job.write();
    g.plan = director.clone_working_solution();
    g.plan.score = Some(score);
    g.status = SolverStatus::NotSolving;
}
