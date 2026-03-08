use crate::domain::{OrSchedule, Surgery, TimeSlot};
use solverforge::prelude::*;

/// Penalize if surgeon is not available during the assigned time slot.
pub fn surgeon_available(schedule: &OrSchedule) -> impl ConstraintOp {
    constraint_factory(schedule)
        .for_each(|s: &OrSchedule| s.surgeries.iter())
        .join(
            |s: &Surgery| s.slot_idx,
            |s: &OrSchedule| s.time_slots.iter().enumerate(),
            |_slot_idx: &Option<usize>, (idx, _slot): &(usize, &TimeSlot)| Some(*idx),
        )
        .filter(|surgery: &Surgery, (_idx, slot): &(usize, &TimeSlot)| {
            // Surgeon must be available in this time slot
            !slot.surgeon_ids_available.contains(&surgery.surgeon_id)
        })
        .penalize(HardSoftScore::ONE_HARD)
}
