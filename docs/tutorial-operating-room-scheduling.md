# Tutorial: Operating Room Scheduling

**Build a constraint solver that schedules surgeries into operating rooms — in under 5 minutes.**

Hospitals lose millions annually from underutilized ORs and last-minute schedule chaos. Most still use whiteboards. SolverForge gives you a native Rust constraint solver that assigns surgeries to rooms and time slots — respecting equipment, surgeon availability, and patient priority — scaffolded from the command line.

---

## What We're Building

A solver that assigns **surgeries** to **operating rooms** and **time slots**, respecting:

- **Hard**: No two surgeries overlap in the same room
- **Hard**: Room has required equipment (e.g., robotic arm, cardiac bypass)
- **Hard**: Surgeon is available during the assigned slot
- **Soft**: High-priority (emergency/urgent) cases scheduled earlier
- **Soft**: Minimize gaps between surgeries (maximize OR utilization)

This is a **resource-constrained scheduling** problem — the bread and butter of constraint solvers.

---

## Step 1: Scaffold

```bash
solverforge new or-scheduling --basic
cd or-scheduling
```

```
▸ Creating basic project or-scheduling

  Done! Your project is ready.

  Next steps:
    cd or-scheduling
    $ solverforge server
    Open http://localhost:7860 in your browser
```

You have a compiling project with solver, web API, and a starter constraint out of the box.

---

## Step 2: Build the Domain with the CLI

### Create the solution

```bash
solverforge generate solution or_schedule
```

```
▸ Created src/domain/or_schedule.rs
▸ Updated src/domain/mod.rs
```

### Create the planning entity — surgery with two planning variables

```bash
solverforge generate entity surgery --planning-variable room_idx
```

```
▸ Created src/domain/surgery.rs
▸ Updated src/domain/mod.rs
▸ Updated src/domain/or_schedule.rs   ← auto-wired with #[planning_entity_collection]
```

Now add the second planning variable:

```bash
solverforge generate variable slot_idx --entity Surgery
```

```
▸ Updated src/domain/surgery.rs
```

Open `src/domain/surgery.rs` — both planning variables are annotated and ready. Add the domain fields:

```rust
use serde::{Deserialize, Serialize};
use solverforge::prelude::*;

/// A surgery that needs to be scheduled into an operating room and time slot.
#[planning_entity]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Surgery {
    #[planning_id]
    pub id: String,
    #[planning_variable(allows_unassigned = true)]
    pub room_idx: Option<usize>,
    #[planning_variable(allows_unassigned = true)]
    pub slot_idx: Option<usize>,
    
    // Domain fields
    pub patient_name: String,
    pub procedure: String,
    pub duration_minutes: u32,
    pub required_equipment: Vec<String>,
    pub surgeon_id: String,
    pub priority: u32, // 1=Emergency, 2=Urgent, 3=Elective
}

impl Surgery {
    pub fn new(
        id: impl Into<String>,
        patient_name: String,
        procedure: String,
        duration_minutes: u32,
        required_equipment: Vec<String>,
        surgeon_id: String,
        priority: u32,
    ) -> Self {
        Self {
            id: id.into(),
            room_idx: None,
            slot_idx: None,
            patient_name,
            procedure,
            duration_minutes,
            required_equipment,
            surgeon_id,
            priority,
        }
    }
}
```

### Create the problem facts

```bash
solverforge generate fact operating_room
solverforge generate fact time_slot
```

```
▸ Created src/domain/operating_room.rs
▸ Updated src/domain/mod.rs
▸ Updated src/domain/or_schedule.rs   ← auto-wired with #[problem_fact_collection]

▸ Created src/domain/time_slot.rs
▸ Updated src/domain/mod.rs
▸ Updated src/domain/or_schedule.rs   ← auto-wired with #[problem_fact_collection]
```

Update `src/domain/operating_room.rs`:

```rust
use serde::{Deserialize, Serialize};
use solverforge::prelude::*;

/// An operating room with specific equipment capabilities.
#[problem_fact]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OperatingRoom {
    pub id: String,
    pub name: String,
    pub equipment: Vec<String>, // Available equipment in this room
}

impl OperatingRoom {
    pub fn new(id: impl Into<String>, name: String, equipment: Vec<String>) -> Self {
        Self {
            id: id.into(),
            name,
            equipment,
        }
    }
}
```

Update `src/domain/time_slot.rs`:

```rust
use serde::{Deserialize, Serialize};
use solverforge::prelude::*;

/// A time slot when surgeries can be scheduled.
#[problem_fact]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeSlot {
    pub id: String,
    pub start: String, // e.g., "08:00"
    pub end: String,   // e.g., "10:00"
    pub surgeon_ids_available: Vec<String>, // Which surgeons are available
}

impl TimeSlot {
    pub fn new(
        id: impl Into<String>,
        start: String,
        end: String,
        surgeon_ids_available: Vec<String>,
    ) -> Self {
        Self {
            id: id.into(),
            start,
            end,
            surgeon_ids_available,
        }
    }
}
```

**5 CLI commands to build the entire domain — solution, entity, two variables, two facts — all auto-wired.**

---

## Step 3: Add Constraints with the Wizard

### No overlapping surgeries (hard)

```bash
solverforge generate constraint no_overlap
```

The wizard prompts you:

```
? Constraint type
    Penalize matching entities        (e.g. unassigned, invalid state)
  ❯ Penalize conflicting pairs        (e.g. overlapping shifts, double-booking)
    Penalize entity-fact mismatch     (e.g. missing skill, wrong location)
    Balance assignments               (e.g. fair workload distribution)
    Reward matching entities          (e.g. desired day, preferred shift)

? Hard or soft
  ❯ Hard  (must be satisfied — correctness)
    Soft  (should be optimized — quality)
```

```
▸ Created src/constraints/no_overlap.rs
▸ Updated src/constraints/mod.rs
```

The generated skeleton uses `for_each_unique_pair` with a joiner. Complete the conflict condition in `src/constraints/no_overlap.rs`:

```rust
use solverforge::prelude::*;
use crate::domain::{Surgery, OrSchedule};

/// Penalize if two surgeries overlap in the same room.
pub fn no_overlap(schedule: &OrSchedule) -> impl ConstraintOp {
    constraint_factory(schedule)
        .for_each_unique_pair(
            |s: &OrSchedule| s.surgeries.as_slice(),
            joiner::equal(|s: &Surgery| s.room_idx),
        )
        .filter(|a: &Surgery, b: &Surgery| {
            // Both must be assigned to same room AND same time slot
            a.room_idx.is_some() && 
            a.slot_idx.is_some() && 
            a.slot_idx == b.slot_idx
        })
        .penalize(HardSoftScore::ONE_HARD)
}
```

### Equipment match (hard) — skip the wizard with flags

```bash
solverforge generate constraint equipment_match --join --hard
```

```
▸ Created src/constraints/equipment_match.rs
▸ Updated src/constraints/mod.rs
```

Complete the equipment match constraint in `src/constraints/equipment_match.rs`:

```rust
use solverforge::prelude::*;
use crate::domain::{Surgery, OperatingRoom, OrSchedule};

/// Penalize if surgery's required equipment is not available in the assigned room.
pub fn equipment_match(schedule: &OrSchedule) -> impl ConstraintOp {
    constraint_factory(schedule)
        .for_each(|s: &OrSchedule| s.surgeries.iter())
        .join(
            |s: &Surgery| s.room_idx,
            |s: &OrSchedule| s.operating_rooms.iter().enumerate(),
            |_room_idx: &usize, (idx, _room): &(usize, &OperatingRoom)| *idx,
        )
        .filter(|surgery: &Surgery, (_idx, room): &(usize, &OperatingRoom)| {
            // Check if room lacks any required equipment
            surgery.required_equipment.iter().any(|req_eq| {
                !room.equipment.contains(req_eq)
            })
        })
        .penalize(HardSoftScore::ONE_HARD)
}
```

### Surgeon availability (hard)

```bash
solverforge generate constraint surgeon_available --join --hard
```

```
▸ Created src/constraints/surgeon_available.rs
▸ Updated src/constraints/mod.rs
```

Complete the surgeon availability constraint in `src/constraints/surgeon_available.rs`:

```rust
use solverforge::prelude::*;
use crate::domain::{Surgery, TimeSlot, OrSchedule};

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
```

### Priority scheduling (soft)

```bash
solverforge generate constraint priority_first --reward
```

```
▸ Created src/constraints/priority_first.rs
▸ Updated src/constraints/mod.rs
```

Complete the priority scheduling constraint in `src/constraints/priority_first.rs`:

```rust
use solverforge::prelude::*;
use crate::domain::{Surgery, OrSchedule};

/// Reward scheduling high-priority surgeries in earlier time slots.
pub fn priority_first(schedule: &OrSchedule) -> impl ConstraintOp {
    constraint_factory(schedule)
        .for_each(|s: &OrSchedule| s.surgeries.iter())
        .filter(|s: &Surgery| s.slot_idx.is_some())
        .reward_long(|s: &Surgery| {
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
        }, HardSoftScore::ONE_SOFT)
}
```

### Utilization (soft)

```bash
solverforge generate constraint maximize_utilization --balance
```

```
▸ Created src/constraints/maximize_utilization.rs
▸ Updated src/constraints/mod.rs
```

Complete the utilization constraint in `src/constraints/maximize_utilization.rs`:

```rust
use solverforge::prelude::*;
use crate::domain::{Surgery, OrSchedule};

/// Penalize gaps between surgeries in the same room to maximize OR utilization.
pub fn maximize_utilization(schedule: &OrSchedule) -> impl ConstraintOp {
    constraint_factory(schedule)
        .for_each_unique_pair(
            |s: &OrSchedule| s.surgeries.as_slice(),
            joiner::equal(|s: &Surgery| s.room_idx),
        )
        .filter(|a: &Surgery, b: &Surgery| {
            // Both assigned to same room, check for gaps
            a.room_idx.is_some() && 
            a.slot_idx.is_some() && 
            b.slot_idx.is_some()
        })
        .penalize_long(|a: &Surgery, b: &Surgery| {
            // Penalize gaps between consecutive surgeries
            let gap = (a.slot_idx.unwrap() as i64 - b.slot_idx.unwrap() as i64).abs() - 1;
            gap.max(0) // Only penalize if gap > 0
        }, HardSoftScore::ONE_SOFT)
}
```

---

## Step 4: Add Demo Data

Create `src/data/mod.rs` with sample data:

```rust
use crate::domain::{Surgery, OperatingRoom, TimeSlot, OrSchedule};

pub fn demo_schedule() -> OrSchedule {
    let operating_rooms = vec![
        OperatingRoom::new("or1", "OR-1 Cardiac".into(), 
            vec!["cardiac_bypass".into(), "general".into()]),
        OperatingRoom::new("or2", "OR-2 Neuro".into(), 
            vec!["neuro_monitor".into(), "general".into()]),
        OperatingRoom::new("or3", "OR-3 Ortho".into(), 
            vec!["robotic_arm".into(), "general".into()]),
        OperatingRoom::new("or4", "OR-4 General".into(), 
            vec!["general".into()]),
        OperatingRoom::new("or5", "OR-5 General".into(), 
            vec!["general".into()]),
        OperatingRoom::new("or6", "OR-6 Hybrid".into(), 
            vec!["cardiac_bypass".into(), "robotic_arm".into(), "general".into()]),
    ];

    let time_slots = vec![
        TimeSlot::new("slot1", "08:00".into(), "09:00".into(), 
            vec!["surgeon1".into(), "surgeon2".into(), "surgeon3".into()]),
        TimeSlot::new("slot2", "09:00".into(), "10:00".into(), 
            vec!["surgeon1".into(), "surgeon2".into(), "surgeon4".into()]),
        TimeSlot::new("slot3", "10:00".into(), "11:00".into(), 
            vec!["surgeon2".into(), "surgeon3".into(), "surgeon4".into()]),
        TimeSlot::new("slot4", "11:00".into(), "12:00".into(), 
            vec!["surgeon1".into(), "surgeon3".into(), "surgeon4".into()]),
        TimeSlot::new("slot5", "13:00".into(), "14:00".into(), 
            vec!["surgeon1".into(), "surgeon2".into(), "surgeon3".into(), "surgeon4".into()]),
        TimeSlot::new("slot6", "14:00".into(), "15:00".into(), 
            vec!["surgeon2".into(), "surgeon3".into(), "surgeon4".into()]),
        TimeSlot::new("slot7", "15:00".into(), "16:00".into(), 
            vec!["surgeon1".into(), "surgeon3".into(), "surgeon4".into()]),
        TimeSlot::new("slot8", "16:00".into(), "17:00".into(), 
            vec!["surgeon1".into(), "surgeon2".into()]),
    ];

    let surgeries = vec![
        // Emergency surgeries (priority 1)
        Surgery::new("s1", "John Smith".into(), "Emergency Cardiac".into(), 
            120, vec!["cardiac_bypass".into()], "surgeon1".into(), 1),
        Surgery::new("s2", "Jane Doe".into(), "Emergency Trauma".into(), 
            90, vec!["general".into()], "surgeon2".into(), 1),
        
        // Urgent surgeries (priority 2)
        Surgery::new("s3", "Bob Wilson".into(), "Urgent Appendectomy".into(), 
            60, vec!["general".into()], "surgeon3".into(), 2),
        Surgery::new("s4", "Alice Brown".into(), "Urgent Fracture".into(), 
            90, vec!["robotic_arm".into()], "surgeon4".into(), 2),
        Surgery::new("s5", "Charlie Davis".into(), "Urgent Neuro".into(), 
            180, vec!["neuro_monitor".into()], "surgeon2".into(), 2),
        
        // Elective surgeries (priority 3)
        Surgery::new("s6", "Diana Miller".into(), "Knee Replacement".into(), 
            120, vec!["robotic_arm".into()], "surgeon4".into(), 3),
        Surgery::new("s7", "Edward Jones".into(), "Hip Replacement".into(), 
            120, vec!["robotic_arm".into()], "surgeon4".into(), 3),
        Surgery::new("s8", "Fiona Garcia".into(), "Cataract".into(), 
            30, vec!["general".into()], "surgeon1".into(), 3),
        Surgery::new("s9", "George Martinez".into(), "Hernia Repair".into(), 
            60, vec!["general".into()], "surgeon3".into(), 3),
        Surgery::new("s10", "Helen Rodriguez".into(), "Gallbladder".into(), 
            90, vec!["general".into()], "surgeon2".into(), 3),
        
        // Add more surgeries to create a realistic scheduling problem
        Surgery::new("s11", "Ian Lee".into(), "Cardiac Bypass".into(), 
            240, vec!["cardiac_bypass".into()], "surgeon1".into(), 2),
        Surgery::new("s12", "Julia White".into(), "Brain Surgery".into(), 
            300, vec!["neuro_monitor".into()], "surgeon2".into(), 2),
        Surgery::new("s13", "Kevin Harris".into(), "Spine Surgery".into(), 
            180, vec!["robotic_arm".into()], "surgeon4".into(), 3),
        Surgery::new("s14", "Laura Clark".into(), "Thyroid".into(), 
            90, vec!["general".into()], "surgeon3".into(), 3),
        Surgery::new("s15", "Michael Lewis".into(), "Prostate".into(), 
            120, vec!["robotic_arm".into()], "surgeon4".into(), 3),
    ];

    OrSchedule::new(surgeries, operating_rooms, time_slots)
}
```

## Step 5: Run It

```bash
solverforge server
```

Open `http://localhost:7860`. Hit **Solve**. 15 surgeries across 6 ORs and 8 time slots — scheduled in seconds with zero conflicts.

---

## Recap: What the CLI Did For You

| Step | CLI Command | Auto-generated |
|------|------------|----------------|
| Solution | `generate solution or_schedule` | Solution struct, score, mod.rs |
| Entity | `generate entity surgery --planning-variable room_idx` | Entity struct, variable, **wired into solution** |
| Variable | `generate variable slot_idx --entity Surgery` | Second planning variable injected |
| Fact 1 | `generate fact operating_room` | Fact struct, **wired into solution** |
| Fact 2 | `generate fact time_slot` | Fact struct, **wired into solution** |
| Constraint 1 | `generate constraint no_overlap` (wizard) | Pair skeleton, **registered in mod.rs** |
| Constraint 2 | `generate constraint equipment_match --join --hard` | Join skeleton, registered |
| Constraint 3 | `generate constraint surgeon_available --join --hard` | Join skeleton, registered |
| Constraint 4 | `generate constraint priority_first --reward` | Reward skeleton, registered |
| Constraint 5 | `generate constraint maximize_utilization --balance` | Balance skeleton, registered |

**10 commands. Zero manual file creation. Zero manual wiring. You only add domain fields and fill in constraint TODOs.**

---

## The Business Case

A single underutilized OR costs a hospital **$1,000–2,000 per hour** in lost revenue. A single scheduling conflict means a cancelled surgery, a wasted anesthesia team, and a patient who doesn't get care.

This solver eliminates both problems. It runs as a standalone binary — no JVM, no cloud dependency, no per-seat license. Deploy it on a $5/month server or embed it in your hospital's existing systems.

**Total time from zero to working solver: ~5 minutes.**

---

## Next Steps

- Add **surgeon preference**: `solverforge generate constraint preferred_room --reward`
- Add **cleanup buffer**: `solverforge generate constraint room_buffer --pair --hard`
- Upgrade score type: `solverforge generate score HardMediumSoftScore`
- Connect to your hospital's EHR/scheduling system via the REST API
