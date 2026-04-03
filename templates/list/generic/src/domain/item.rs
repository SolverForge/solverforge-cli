use serde::{Deserialize, Serialize};
use solverforge::prelude::*;

/// An item that gets placed into a container.
///
/// Rename this to something domain-specific (Job, Package, Task, Order, …)
/// and add whatever fields describe a unit of work.
#[problem_fact]
#[derive(Serialize, Deserialize)]
pub struct Item {
    #[planning_id]
    pub id: String,
    pub name: String,
}

impl Item {
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
        }
    }

    pub fn finalize(&mut self) {}
}
