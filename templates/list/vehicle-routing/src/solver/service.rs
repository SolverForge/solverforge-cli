use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::oneshot;

use crate::domain::{ProblemData, Vehicle, VrpPlan};

use super::engine::{compute_cost, solve};

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SolverStatus {
    NotSolving,
    Solving,
}

/// A solving job: holds the `Box<ProblemData>` that backs all vehicle pointers.
pub struct SolveJob {
    pub id: String,
    pub status: SolverStatus,
    /// The latest best plan (updated periodically during solving).
    pub routes: Vec<Vec<usize>>,
    pub cost: i64,
    pub score: Option<String>,
    /// Owned problem data — must outlive all vehicle raw pointers.
    pub problem: Box<ProblemData>,
    pub n_vehicles: usize,
    pub time_limit_secs: u64,
    stop_signal: Option<oneshot::Sender<()>>,
}

impl SolveJob {
    pub fn new(
        id: String,
        problem: Box<ProblemData>,
        n_vehicles: usize,
        time_limit_secs: u64,
    ) -> Self {
        Self {
            id,
            status: SolverStatus::NotSolving,
            routes: Vec::new(),
            cost: 0,
            score: None,
            problem,
            n_vehicles,
            time_limit_secs,
            stop_signal: None,
        }
    }
}

pub struct SolverService {
    jobs: RwLock<HashMap<String, Arc<RwLock<SolveJob>>>>,
}

impl SolverService {
    pub fn new() -> Self {
        Self { jobs: RwLock::new(HashMap::new()) }
    }

    pub fn create_job(&self, id: String, job: SolveJob) -> Arc<RwLock<SolveJob>> {
        let job = Arc::new(RwLock::new(job));
        self.jobs.write().insert(id, job.clone());
        job
    }

    pub fn get_job(&self, id: &str) -> Option<Arc<RwLock<SolveJob>>> {
        self.jobs.read().get(id).cloned()
    }

    pub fn list_jobs(&self) -> Vec<String> {
        self.jobs.read().keys().cloned().collect()
    }

    pub fn remove_job(&self, id: &str) -> Option<Arc<RwLock<SolveJob>>> {
        self.jobs.write().remove(id)
    }

    pub fn start_solving(&self, job: Arc<RwLock<SolveJob>>) {
        let (tx, _rx) = oneshot::channel::<()>();
        let time_limit_secs = job.read().time_limit_secs;
        let n_vehicles = job.read().n_vehicles;
        {
            let mut g = job.write();
            g.status = SolverStatus::Solving;
            g.stop_signal = Some(tx);
        }

        let job_clone = job.clone();
        tokio::task::spawn_blocking(move || {
            // Build a fresh VrpPlan borrowing the stable Box pointer.
            let data_ptr: *const ProblemData = &*job_clone.read().problem;
            let vehicles: Vec<Vehicle> = (0..n_vehicles)
                .map(|id| Vehicle { id, visits: Vec::new(), data: data_ptr })
                .collect();
            let plan = VrpPlan { vehicles, score: None };

            let solved = solve(plan, time_limit_secs);

            let routes: Vec<Vec<usize>> = solved
                .vehicles
                .iter()
                .filter(|v| !v.visits.is_empty())
                .map(|v| v.visits.clone())
                .collect();
            let cost = compute_cost(&solved);
            let score = solved.score.map(|s| s.to_string());

            let mut g = job_clone.write();
            g.routes = routes;
            g.cost = cost;
            g.score = score;
            g.status = SolverStatus::NotSolving;
        });
    }

    pub fn stop_solving(&self, id: &str) {
        if let Some(job) = self.get_job(id) {
            let mut g = job.write();
            if let Some(tx) = g.stop_signal.take() {
                let _ = tx.send(());
                g.status = SolverStatus::NotSolving;
            }
        }
    }
}

impl Default for SolverService {
    fn default() -> Self {
        Self::new()
    }
}
