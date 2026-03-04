use serde::{Deserialize, Serialize};
use solverforge::prelude::*;

use super::{Resource, Task};

/// The root planning solution: resources + tasks + score.
///
/// Rename this to something domain-specific (Schedule, Roster, Timetable, …).
#[planning_solution]
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
        Self { resources, tasks, score: None }
    }
}
