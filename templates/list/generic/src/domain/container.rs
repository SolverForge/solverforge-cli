use serde::{Deserialize, Serialize};
use solverforge::prelude::*;

/// A container that owns an ordered sequence of items.
///
/// Rename this to something domain-specific (Worker, Machine, Bin, Lane, …)
/// and add whatever fields describe a capacity or processing unit.
#[planning_entity]
#[derive(Serialize, Deserialize)]
pub struct Container {
    #[planning_id]
    pub id: usize,
    pub name: String,
    /// Ordered sequence of item indices assigned to this container.
    ///
    /// This field declares the list variable the solver may reorder.
    /// `solver.toml` still controls search strategy; the list starter only
    /// changes the sample app shape and example domain.
    #[planning_list_variable(element_collection = "item_facts")]
    pub items: Vec<usize>,
}

impl Container {
    pub fn new(id: usize, name: impl Into<String>) -> Self {
        Self {
            id,
            name: name.into(),
            items: Vec::new(),
        }
    }
}
