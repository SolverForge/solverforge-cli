/* Constraint definitions.

Each constraint is a separate function returning an `IncrementalConstraint`.
Assemble them into a tuple — the solver scores the solution against all of them.

Add as many constraints as your problem needs. The tuple supports up to 12 elements;
use nested tuples for more. */

pub use self::assemble::create_constraints;

// @solverforge:begin constraint-modules
mod balanced_load;
mod sequence_cohesion;
// @solverforge:end constraint-modules

mod assemble {
    use super::*;
    use crate::domain::Plan;
    use solverforge::prelude::*;

    pub fn create_constraints() -> impl ConstraintSet<Plan, HardSoftScore> {
        // @solverforge:begin constraint-calls
        (balanced_load::constraint(), sequence_cohesion::constraint())
        // @solverforge:end constraint-calls
    }
}
