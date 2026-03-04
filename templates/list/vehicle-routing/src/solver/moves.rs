//! List-variable accessor functions required by move selectors.
//!
//! SolverForge list-variable moves take plain fn pointers rather than trait
//! objects, keeping the hot path fully monomorphized and zero-allocation.

use crate::domain::VrpPlan;

// --- ListChangeMove / ListRuinMove: remove and insert single elements ---

pub fn list_len(plan: &VrpPlan, entity_idx: usize) -> usize {
    plan.vehicles.get(entity_idx).map_or(0, |v| v.visits.len())
}

pub fn list_remove(plan: &mut VrpPlan, entity_idx: usize, pos: usize) -> Option<usize> {
    plan.vehicles.get_mut(entity_idx).map(|v| v.visits.remove(pos))
}

pub fn list_insert(plan: &mut VrpPlan, entity_idx: usize, pos: usize, val: usize) {
    if let Some(v) = plan.vehicles.get_mut(entity_idx) {
        v.visits.insert(pos, val);
    }
}

// --- ListSwapMove: get and set a single element ---

pub fn list_get(plan: &VrpPlan, entity_idx: usize, pos: usize) -> Option<usize> {
    plan.vehicles.get(entity_idx).and_then(|v| v.visits.get(pos).copied())
}

pub fn list_set(plan: &mut VrpPlan, entity_idx: usize, pos: usize, val: usize) {
    if let Some(v) = plan.vehicles.get_mut(entity_idx) {
        if let Some(elem) = v.visits.get_mut(pos) {
            *elem = val;
        }
    }
}

// --- ListReverseMove: reverse a segment in-place (intra-route 2-opt) ---

pub fn list_reverse(plan: &mut VrpPlan, entity_idx: usize, start: usize, end: usize) {
    if let Some(v) = plan.vehicles.get_mut(entity_idx) {
        v.visits[start..end].reverse();
    }
}

// --- SubListChangeMove: drain and re-insert a contiguous segment (Or-opt) ---

pub fn sublist_remove(plan: &mut VrpPlan, entity_idx: usize, start: usize, end: usize) -> Vec<usize> {
    plan.vehicles
        .get_mut(entity_idx)
        .map(|v| v.visits.drain(start..end).collect())
        .unwrap_or_default()
}

pub fn sublist_insert(plan: &mut VrpPlan, entity_idx: usize, pos: usize, items: Vec<usize>) {
    if let Some(v) = plan.vehicles.get_mut(entity_idx) {
        for (i, item) in items.into_iter().enumerate() {
            v.visits.insert(pos + i, item);
        }
    }
}

// --- ListRuinMove: non-Option remove (element is always present) ---

pub fn ruin_remove(plan: &mut VrpPlan, entity_idx: usize, pos: usize) -> usize {
    plan.vehicles.get_mut(entity_idx).map(|v| v.visits.remove(pos)).unwrap_or(0)
}

pub fn ruin_insert(plan: &mut VrpPlan, entity_idx: usize, pos: usize, val: usize) {
    if let Some(v) = plan.vehicles.get_mut(entity_idx) {
        v.visits.insert(pos, val);
    }
}
