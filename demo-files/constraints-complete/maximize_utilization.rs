use crate::domain::{OrSchedule, Surgery};
use solverforge::prelude::*;

/// Penalize gaps between surgeries in the same room to maximize OR utilization.
pub fn maximize_utilization(schedule: &OrSchedule) -> impl ConstraintOp {
    constraint_factory(schedule)
        .for_each_unique_pair(
            |s: &OrSchedule| s.surgeries.as_slice(),
            joiner::equal(|s: &Surgery| s.room_idx),
        )
        .filter(|a: &Surgery, b: &Surgery| {
            // Both assigned to same room, check for gaps
            a.room_idx.is_some() && a.slot_idx.is_some() && b.slot_idx.is_some()
        })
        .penalize_long(
            |a: &Surgery, b: &Surgery| {
                // Penalize gaps between consecutive surgeries
                let gap = (a.slot_idx.unwrap() as i64 - b.slot_idx.unwrap() as i64).abs() - 1;
                gap.max(0) // Only penalize if gap > 0
            },
            HardSoftScore::ONE_SOFT,
        )
}
