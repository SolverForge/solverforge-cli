use crate::domain::{OrSchedule, Surgery};
use solverforge::prelude::*;

/// Penalize if two surgeries overlap in the same room.
pub fn no_overlap(schedule: &OrSchedule) -> impl ConstraintOp {
    constraint_factory(schedule)
        .for_each_unique_pair(
            |s: &OrSchedule| s.surgeries.as_slice(),
            joiner::equal(|s: &Surgery| s.room_idx),
        )
        .filter(|a: &Surgery, b: &Surgery| {
            // Both must be assigned to same room AND same time slot
            a.room_idx.is_some() && a.slot_idx.is_some() && a.slot_idx == b.slot_idx
        })
        .penalize(HardSoftScore::ONE_HARD)
}
