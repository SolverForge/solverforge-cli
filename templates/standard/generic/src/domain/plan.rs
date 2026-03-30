use serde::{Deserialize, Serialize};
use solverforge::prelude::*;

use super::{Resource, Task};

/// The root planning solution: resources + tasks + score.
///
/// Rename this to something domain-specific (Schedule, Roster, Timetable, …).
/// The solvable fields live on entities; this root type just groups facts,
/// entities, and score for the sample starter.
#[planning_solution(constraints = "crate::constraints::create_constraints")]
#[derive(Serialize, Deserialize)]
pub struct Plan {
    #[problem_fact_collection]
    pub resources: Vec<Resource>,
    #[planning_entity_collection]
    pub tasks: Vec<Task>,
    #[planning_score]
    pub score: Option<HardSoftScore>,
}

impl Plan {
    pub fn new(resources: Vec<Resource>, tasks: Vec<Task>) -> Self {
        Self {
            resources,
            tasks,
            score: None,
        }
    }
}
