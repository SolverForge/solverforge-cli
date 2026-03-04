//! Constraint definitions for vehicle routing.
//!
//! Two constraints mirror the CVRP standard:
//!   - HARD  vehicleCapacity — penalise demand overload per vehicle
//!   - SOFT  totalDistance   — minimise sum of route distances

use crate::domain::{ProblemData, Vehicle, VrpPlan};
use solverforge::prelude::*;

pub fn create_constraints() -> impl ConstraintSet<VrpPlan, HardSoftScore> {
    let capacity = ConstraintFactory::<VrpPlan, HardSoftScore>::new()
        .for_each(|p: &VrpPlan| p.vehicles.as_slice())
        .filter(|v: &Vehicle| !v.visits.is_empty())
        .penalize_hard_with(|v: &Vehicle| {
            let data = unsafe { &*(v.data as *const ProblemData) };
            let overload = (data.route_demand(&v.visits) - data.capacity).max(0);
            HardSoftScore::of(overload, 0)
        })
        .as_constraint("vehicleCapacity");

    let distance = ConstraintFactory::<VrpPlan, HardSoftScore>::new()
        .for_each(|p: &VrpPlan| p.vehicles.as_slice())
        .filter(|v: &Vehicle| !v.visits.is_empty())
        .penalize_with(|v: &Vehicle| {
            let data = unsafe { &*(v.data as *const ProblemData) };
            HardSoftScore::of(0, data.route_distance(&v.visits))
        })
        .as_constraint("totalDistance");

    (capacity, distance)
}
