//! Assembles the project's constraint set.

use crate::domain::Plan;
use solverforge::prelude::*;

/// Builds the constraint set the solver scores every solution against.
pub fn create_constraints() -> impl ConstraintSet<Plan, HardSoftScore> {
    // @solverforge:begin constraint-calls
    (
        super::balanced_load::constraint(),
        super::sequence_cohesion::constraint(),
    )
    // @solverforge:end constraint-calls
}
