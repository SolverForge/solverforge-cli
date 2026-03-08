use crate::domain::{OperatingRoom, OrSchedule, Surgery};
use solverforge::prelude::*;

/// Penalize if surgery's required equipment is not available in the assigned room.
pub fn equipment_match(schedule: &OrSchedule) -> impl ConstraintOp {
    constraint_factory(schedule)
        .for_each(|s: &OrSchedule| s.surgeries.iter())
        .join(
            |s: &Surgery| s.room_idx,
            |s: &OrSchedule| s.operating_rooms.iter().enumerate(),
            |_room_idx: &Option<usize>, (idx, _room): &(usize, &OperatingRoom)| Some(*idx),
        )
        .filter(
            |surgery: &Surgery, (_idx, room): &(usize, &OperatingRoom)| {
                // Check if room lacks any required equipment
                surgery
                    .required_equipment
                    .iter()
                    .any(|req_eq| !room.equipment.contains(req_eq))
            },
        )
        .penalize(HardSoftScore::ONE_HARD)
}
