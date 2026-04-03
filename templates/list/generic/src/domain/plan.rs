use serde::{Deserialize, Serialize};
use solverforge::prelude::*;
use solverforge::CrossEntityDistanceMeter;

use super::{Container, Item};

/// The root planning solution: items + containers + score.
///
/// Rename this to something domain-specific (Route, Schedule, Assignment, …).
/// The list field on `Container` declares what is solvable; this root type
/// groups the sample data and score for the starter.
#[planning_solution(constraints = "crate::constraints::create_constraints")]
#[derive(Serialize, Deserialize)]
pub struct Plan {
    #[problem_fact_collection]
    pub item_facts: Vec<Item>,
    #[planning_entity_collection]
    pub containers: Vec<Container>,
    #[planning_score]
    pub score: Option<HardSoftScore>,
}

impl Plan {
    pub fn new(item_facts: Vec<Item>, containers: Vec<Container>) -> Self {
        Self {
            item_facts,
            containers,
            score: None,
        }
    }
}

/// Simple cross-entity meter for the generic list scaffold.
///
/// Uses item-position distance so nearby move selectors can rank positions
/// without relying on a domain-specific metric.
#[derive(Clone, Default)]
pub struct ItemIndexDistanceMeter;

impl CrossEntityDistanceMeter<Plan> for ItemIndexDistanceMeter {
    fn distance(
        &self,
        solution: &Plan,
        src_entity: usize,
        src_pos: usize,
        dst_entity: usize,
        dst_pos: usize,
    ) -> f64 {
        let src_item = solution
            .containers
            .get(src_entity)
            .and_then(|container| container.items.get(src_pos))
            .copied();
        let dst_item = solution
            .containers
            .get(dst_entity)
            .and_then(|container| container.items.get(dst_pos))
            .copied();

        match (src_item, dst_item) {
            (Some(src), Some(dst)) => src.abs_diff(dst) as f64,
            _ => f64::INFINITY,
        }
    }
}
