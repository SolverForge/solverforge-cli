use serde::{Deserialize, Serialize};
use solverforge::prelude::*;

/// A time slot when surgeries can be scheduled.
#[problem_fact]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeSlot {
    pub id: String,
    pub start: String,                      // e.g., "08:00"
    pub end: String,                        // e.g., "10:00"
    pub surgeon_ids_available: Vec<String>, // Which surgeons are available
}

impl TimeSlot {
    pub fn new(
        id: impl Into<String>,
        start: String,
        end: String,
        surgeon_ids_available: Vec<String>,
    ) -> Self {
        Self {
            id: id.into(),
            start,
            end,
            surgeon_ids_available,
        }
    }
}
