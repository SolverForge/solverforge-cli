use serde::{Deserialize, Serialize};
use solverforge::prelude::*;

/// A surgery that needs to be scheduled into an operating room and time slot.
#[planning_entity]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Surgery {
    #[planning_id]
    pub id: String,
    #[planning_variable(allows_unassigned = true)]
    pub room_idx: Option<usize>,
    #[planning_variable(allows_unassigned = true)]
    pub slot_idx: Option<usize>,

    // Domain fields
    pub patient_name: String,
    pub procedure: String,
    pub duration_minutes: u32,
    pub required_equipment: Vec<String>,
    pub surgeon_id: String,
    pub priority: u32, // 1=Emergency, 2=Urgent, 3=Elective
}

impl Surgery {
    pub fn new(
        id: impl Into<String>,
        patient_name: String,
        procedure: String,
        duration_minutes: u32,
        required_equipment: Vec<String>,
        surgeon_id: String,
        priority: u32,
    ) -> Self {
        Self {
            id: id.into(),
            room_idx: None,
            slot_idx: None,
            patient_name,
            procedure,
            duration_minutes,
            required_equipment,
            surgeon_id,
            priority,
        }
    }
}
