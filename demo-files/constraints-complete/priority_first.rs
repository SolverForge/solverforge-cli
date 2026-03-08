use crate::domain::{OrSchedule, Surgery};
use solverforge::prelude::*;

/// Reward scheduling high-priority surgeries in earlier time slots.
pub fn priority_first(schedule: &OrSchedule) -> impl ConstraintOp {
    constraint_factory(schedule)
        .for_each(|s: &OrSchedule| s.surgeries.iter())
        .filter(|s: &Surgery| s.slot_idx.is_some())
        .reward_long(
            |s: &Surgery| {
                let slot = s.slot_idx.unwrap_or(999) as i64;
                let priority_weight = match s.priority {
                    1 => 100, // Emergency
                    2 => 50,  // Urgent
                    3 => 10,  // Elective
                    _ => 1,
                };
                // Reward = priority weight * (100 - slot number)
                // Earlier slots get higher rewards
                priority_weight * (100 - slot).max(0)
            },
            HardSoftScore::ONE_SOFT,
        )
}
