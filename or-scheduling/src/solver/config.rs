use std::time::Duration;

const DEFAULT_TIME_LIMIT_SECS: u64 = 30;

/// Termination criteria for the solver.
#[derive(Debug, Clone, Default)]
pub struct SolverConfig {
    pub time_limit: Option<Duration>,
    pub unimproved_time_limit: Option<Duration>,
    pub step_limit: Option<u64>,
    pub unimproved_step_limit: Option<u64>,
}

impl SolverConfig {
    pub fn default_config() -> Self {
        Self {
            time_limit: Some(Duration::from_secs(DEFAULT_TIME_LIMIT_SECS)),
            ..Default::default()
        }
    }

    pub fn should_terminate(
        &self,
        elapsed: Duration,
        steps: u64,
        time_since_improvement: Duration,
        steps_since_improvement: u64,
    ) -> bool {
        if let Some(limit) = self.time_limit {
            if elapsed >= limit {
                return true;
            }
        }
        if let Some(limit) = self.unimproved_time_limit {
            if time_since_improvement >= limit {
                return true;
            }
        }
        if let Some(limit) = self.step_limit {
            if steps >= limit {
                return true;
            }
        }
        if let Some(limit) = self.unimproved_step_limit {
            if steps_since_improvement >= limit {
                return true;
            }
        }
        false
    }
}
