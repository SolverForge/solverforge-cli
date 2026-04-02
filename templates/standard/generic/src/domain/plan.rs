use serde::{Deserialize, Serialize};
use solverforge::prelude::*;

/// The root planning solution.
///
/// Fresh projects start as a neutral shell. Add fact collections, planning
/// entity collections, and variable fields through the CLI as your domain
/// takes shape.
#[planning_solution(constraints = "crate::constraints::create_constraints")]
#[derive(Serialize, Deserialize)]
pub struct Plan {
    #[planning_score]
    pub score: Option<HardSoftScore>,
}

impl Plan {
    pub fn new() -> Self {
        Self { score: None }
    }
}
