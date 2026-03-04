use crate::domain::{EmployeeSchedule, Shift};
use solverforge::prelude::*;
use solverforge::IncrementalConstraint;

/// SOFT: TODO — describe what this constraint optimizes.
pub fn constraint() -> impl IncrementalConstraint<EmployeeSchedule, HardSoftDecimalScore> {
    ConstraintFactory::<EmployeeSchedule, HardSoftDecimalScore>::new()
        .for_each(|s: &EmployeeSchedule| s.shifts.as_slice())
        .filter(|e: &Shift| todo!("add your condition"))
        .reward(HardSoftDecimalScore::ONE_SOFT)
        .as_constraint("Longer Is Better")
}
