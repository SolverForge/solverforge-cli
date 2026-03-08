use serde::{Deserialize, Serialize};
use solverforge::prelude::*;

/// An operating room with specific equipment capabilities.
#[problem_fact]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OperatingRoom {
    pub id: String,
    pub name: String,
    pub equipment: Vec<String>, // Available equipment in this room
}

impl OperatingRoom {
    pub fn new(id: impl Into<String>, name: String, equipment: Vec<String>) -> Self {
        Self {
            id: id.into(),
            name,
            equipment,
        }
    }
}
