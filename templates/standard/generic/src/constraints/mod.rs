/* Constraint definitions.

Add constraint modules with `solverforge generate constraint ...`.
The neutral shell starts with an empty constraint set. */

use crate::domain::Plan;
use solverforge::prelude::*;

pub fn create_constraints() -> impl ConstraintSet<Plan, HardSoftScore> {
    ()
}
