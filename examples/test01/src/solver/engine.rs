use parking_lot::RwLock;
use rand::Rng;
use solverforge::prelude::*;
use solverforge::TypedScoreDirector;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::oneshot;
use tracing::{debug, info};

use super::config::SolverConfig;
use super::service::SolveJob;
use crate::console::{self, PhaseTimer};
use crate::constraints::create_constraints;
use crate::domain::EmployeeSchedule;

const LATE_ACCEPTANCE_SIZE: usize = 400;

pub fn solve_blocking(
    job: Arc<RwLock<SolveJob>>,
    mut stop_rx: oneshot::Receiver<()>,
    config: SolverConfig,
) {
    let initial_schedule = job.read().schedule.clone();
    let job_id = job.read().id.clone();
    let solve_start = Instant::now();

    console::print_config(
        initial_schedule.shifts.len(),
        initial_schedule.employees.len(),
    );

    info!(
        job_id = %job_id,
        shifts = initial_schedule.shifts.len(),
        employees = initial_schedule.employees.len(),
        "Starting solver"
    );

    let constraints = create_constraints();
    let mut director = TypedScoreDirector::new(initial_schedule.clone(), constraints);

    // Phase 1: Construction heuristic
    let mut ch_timer = PhaseTimer::start("ConstructionHeuristic", 0);
    let mut current_score = construction_heuristic(&mut director, &mut ch_timer);
    ch_timer.finish();

    console::print_solving_started(
        solve_start.elapsed().as_millis() as u64,
        &current_score.to_string(),
        initial_schedule.shifts.len(),
        initial_schedule.shifts.len(),
        initial_schedule.employees.len(),
    );

    update_job(&job, &director, current_score);

    let n_employees = director.working_solution().employees.len();
    if n_employees == 0 {
        console::print_solving_ended(
            solve_start.elapsed(),
            0,
            1,
            &current_score.to_string(),
            current_score.is_feasible(),
        );
        finish_job(&job, &director, current_score);
        return;
    }

    // Phase 2: Late Acceptance local search
    let mut ls_timer = PhaseTimer::start("LateAcceptance", 1);
    let mut late_scores = vec![current_score; LATE_ACCEPTANCE_SIZE];
    let mut step: u64 = 0;
    let mut rng = rand::thread_rng();
    let mut best_score = current_score;
    let mut last_improvement_time = solve_start;
    let mut last_improvement_step: u64 = 0;

    loop {
        let elapsed = solve_start.elapsed();
        let time_since_improvement = last_improvement_time.elapsed();
        let steps_since_improvement = step - last_improvement_step;

        if config.should_terminate(
            elapsed,
            step,
            time_since_improvement,
            steps_since_improvement,
        ) {
            break;
        }
        if stop_rx.try_recv().is_ok() {
            info!("Solving stopped by user");
            break;
        }

        if let Some((shift_idx, new_emp)) = generate_move(&director, &mut rng) {
            ls_timer.record_move();

            let old_emp = apply_move(&mut director, shift_idx, new_emp);
            let new_score = director.get_score();

            let late_idx = (step as usize) % LATE_ACCEPTANCE_SIZE;
            let late_score = late_scores[late_idx];

            if new_score >= current_score || new_score >= late_score {
                ls_timer.record_accepted(&current_score.to_string());
                current_score = new_score;
                late_scores[late_idx] = new_score;

                if new_score > best_score {
                    best_score = new_score;
                    last_improvement_time = Instant::now();
                    last_improvement_step = step;
                }
                if ls_timer.steps_accepted().is_multiple_of(1000) {
                    update_job(&job, &director, current_score);
                    debug!(step, score = %current_score, "Progress update");
                }
                if ls_timer.moves_evaluated().is_multiple_of(10000) {
                    console::print_step_progress(
                        ls_timer.steps_accepted(),
                        ls_timer.elapsed(),
                        ls_timer.moves_evaluated(),
                        &current_score.to_string(),
                    );
                }
            } else {
                undo_move(&mut director, shift_idx, old_emp);
            }
            step += 1;
        }
    }

    ls_timer.finish();

    console::print_solving_ended(
        solve_start.elapsed(),
        step,
        2,
        &current_score.to_string(),
        current_score.is_feasible(),
    );

    finish_job(&job, &director, current_score);
}

fn construction_heuristic(
    director: &mut TypedScoreDirector<
        EmployeeSchedule,
        impl ConstraintSet<EmployeeSchedule, HardSoftDecimalScore>,
    >,
    timer: &mut PhaseTimer,
) -> HardSoftDecimalScore {
    let _ = director.calculate_score();
    let n_shifts = director.working_solution().shifts.len();
    let n_employees = director.working_solution().employees.len();

    if n_employees == 0 || n_shifts == 0 {
        return director.get_score();
    }

    let mut emp_idx = 0;
    for shift_idx in 0..n_shifts {
        if director.working_solution().shifts[shift_idx]
            .employee_idx
            .is_some()
        {
            continue;
        }
        timer.record_move();
        director.before_variable_changed(0, shift_idx);
        director.working_solution_mut().shifts[shift_idx].employee_idx = Some(emp_idx);
        director.after_variable_changed(0, shift_idx);
        let score = director.get_score();
        timer.record_accepted(&score.to_string());
        emp_idx = (emp_idx + 1) % n_employees;
    }

    director.get_score()
}

fn generate_move<R: Rng>(
    director: &TypedScoreDirector<
        EmployeeSchedule,
        impl ConstraintSet<EmployeeSchedule, HardSoftDecimalScore>,
    >,
    rng: &mut R,
) -> Option<(usize, Option<usize>)> {
    let sol = director.working_solution();
    let n_shifts = sol.shifts.len();
    let n_employees = sol.employees.len();
    if n_shifts == 0 || n_employees == 0 {
        return None;
    }

    let shift_idx = rng.gen_range(0..n_shifts);
    let current = sol.shifts[shift_idx].employee_idx;
    let new_emp = rng.gen_range(0..n_employees);
    if current == Some(new_emp) {
        return None;
    }
    Some((shift_idx, Some(new_emp)))
}

fn apply_move(
    director: &mut TypedScoreDirector<
        EmployeeSchedule,
        impl ConstraintSet<EmployeeSchedule, HardSoftDecimalScore>,
    >,
    shift_idx: usize,
    new_emp: Option<usize>,
) -> Option<usize> {
    let old = director.working_solution().shifts[shift_idx].employee_idx;
    director.before_variable_changed(0, shift_idx);
    director.working_solution_mut().shifts[shift_idx].employee_idx = new_emp;
    director.after_variable_changed(0, shift_idx);
    old
}

fn undo_move(
    director: &mut TypedScoreDirector<
        EmployeeSchedule,
        impl ConstraintSet<EmployeeSchedule, HardSoftDecimalScore>,
    >,
    shift_idx: usize,
    old_emp: Option<usize>,
) {
    director.before_variable_changed(0, shift_idx);
    director.working_solution_mut().shifts[shift_idx].employee_idx = old_emp;
    director.after_variable_changed(0, shift_idx);
}

fn update_job(
    job: &Arc<RwLock<SolveJob>>,
    director: &TypedScoreDirector<
        EmployeeSchedule,
        impl ConstraintSet<EmployeeSchedule, HardSoftDecimalScore>,
    >,
    score: HardSoftDecimalScore,
) {
    let mut g = job.write();
    g.schedule = director.clone_working_solution();
    g.schedule.score = Some(score);
}

fn finish_job(
    job: &Arc<RwLock<SolveJob>>,
    director: &TypedScoreDirector<
        EmployeeSchedule,
        impl ConstraintSet<EmployeeSchedule, HardSoftDecimalScore>,
    >,
    score: HardSoftDecimalScore,
) {
    use super::service::SolverStatus;
    let mut g = job.write();
    g.schedule = director.clone_working_solution();
    g.schedule.score = Some(score);
    g.status = SolverStatus::NotSolving;
}
