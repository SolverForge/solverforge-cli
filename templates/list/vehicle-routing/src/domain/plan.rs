use solverforge::prelude::*;

use super::Vehicle;

/// The root planning solution: a fleet of vehicles and a score.
#[planning_solution]
pub struct VrpPlan {
    #[planning_entity_collection]
    pub vehicles: Vec<Vehicle>,
    #[planning_score]
    pub score: Option<HardSoftScore>,
}
