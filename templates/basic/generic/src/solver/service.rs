use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::oneshot;

use super::config::SolverConfig;
use super::engine::solve_blocking;
use crate::domain::Plan;

/// Status of a solving job.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SolverStatus {
    NotSolving,
    Solving,
}

/// A solving job with its current state.
pub struct SolveJob {
    pub id: String,
    pub status: SolverStatus,
    pub plan: Plan,
    pub config: SolverConfig,
    stop_signal: Option<oneshot::Sender<()>>,
}

impl SolveJob {
    pub fn new(id: String, plan: Plan) -> Self {
        Self {
            id,
            status: SolverStatus::NotSolving,
            plan,
            config: SolverConfig::default_config(),
            stop_signal: None,
        }
    }
}

/// Manages solving jobs.
pub struct SolverService {
    jobs: RwLock<HashMap<String, Arc<RwLock<SolveJob>>>>,
}

impl SolverService {
    pub fn new() -> Self {
        Self { jobs: RwLock::new(HashMap::new()) }
    }

    pub fn create_job(&self, id: String, plan: Plan) -> Arc<RwLock<SolveJob>> {
        let job = Arc::new(RwLock::new(SolveJob::new(id.clone(), plan)));
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
        let (tx, rx) = oneshot::channel();
        let config = job.read().config.clone();
        {
            let mut g = job.write();
            g.status = SolverStatus::Solving;
            g.stop_signal = Some(tx);
        }
        let job_clone = job.clone();
        tokio::task::spawn_blocking(move || {
            solve_blocking(job_clone, rx, config);
        });
    }

    pub fn stop_solving(&self, id: &str) -> bool {
        if let Some(job) = self.get_job(id) {
            let mut g = job.write();
            if let Some(tx) = g.stop_signal.take() {
                let _ = tx.send(());
                g.status = SolverStatus::NotSolving;
                return true;
            }
        }
        false
    }
}

impl Default for SolverService {
    fn default() -> Self {
        Self::new()
    }
}
