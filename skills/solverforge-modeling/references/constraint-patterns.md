# Constraint patterns

Every `solverforge generate constraint <id> --<pattern> [--hard|--soft]` writes
`src/constraints/<id>.rs` and wires it into `src/constraints/mod.rs` (managed
blocks) and, for web shells, `static/sf-config.json`. Do not wire constraints by
hand.

The generated file is a **stub**: a real `ConstraintFactory` call and placeholder
functions that `panic!`. Its imports, entity type, scalar variable, and fact type
are selected from the **first** entity, first scalar variable, and first fact in
the model. In any multi-collection model, replace those generated choices with
the collections the constraint actually reads. Then replace the placeholders
and fix the stream source.

## Two mandatory edits

1. **Real logic.** Replace every `panic!(...)` placeholder body.
2. **Generated accessor as the source.** Replace `fn entity_items` / `fn
   fact_items` with the `#[planning_solution]`-generated accessors.

### Why the source matters

`#[planning_solution]` generates `Plan::tasks()`, `Plan::resources()`, etc. — one
public accessor per collection. Each carries a `ChangeSource`
(`Descriptor(i)` for planning entities, `Static` for facts/list elements). The
skeleton's hand-written `fn entity_items(solution: &Plan) -> &[Task]` has
`ChangeSource::Unknown`.

- Unary and reward streams work with `Unknown`.
- Joins, self-joins, `group_by`, complement, and projection **panic at solve
  time** with `source Unknown cannot localize entity indexes`.

So use `Plan::<collection>()` everywhere and delete the extractor functions:

```rust
.for_each(Plan::tasks())          // planning entity collection `tasks: Vec<Task>`
.for_each(Plan::resources())      // fact collection `resources: Vec<Resource>`
```

The generated trait `PlanConstraintStreams` exists but is not re-exported by the
scaffold's `src/domain/mod.rs`, so prefer the associated accessor
(`Plan::tasks()`) over `.tasks()` and skip the trait import.

### The `.named(...)` must equal the module id

The model contract requires `constraint module '<id>'` to declare
`.named("<id>")`. If you rename a constraint, rename both the file and the
`.named` string, or `solverforge check` fails. Keep the CLI-assigned name.

## Hard constraints are penalties, not filters

A hard constraint **penalizes** an invalid candidate; it does not remove that
candidate from search. Construction can start from a hard-violating plan, and
local search is allowed to return one if it cannot improve the hard score. A
converged solve with a nonzero hard score means invalid candidates are
reachable — it does not mean the constraint is wrong.

Do not "fix" a returned plan with a post-solve sanitizer or by re-checking rules
in app code. Use one of:

1. **Keep invalid values out of the model's value range.** Restrict what the
   variable can take so a violating assignment is never generated. On a
   scaffolded project, use the scalar/list candidate metadata
   (`--candidate-values`, list metadata) or `generate scalar-group`; on an
   existing model, use a `value_range_provider`/candidate provider. A provider
   may compute availability, but the rule itself must still be stated as a
   constraint over domain objects (for example, availability on a fact read by a
   join), not as a precomputed penalty matrix.
2. **Gate grouped/repair moves on hard improvement.** For scalar groups and
   conflict repair, `require_hard_improvement = true` makes each emitted
   compound move carry a hard-improvement gate. The CLI writes this into the
   `grouped_scalar_move_selector` and compound conflict-repair phases it
   generates; set it manually for hand-written `solver.toml`.
3. **Model "must be assigned" explicitly.** Use `.unassigned()` (hard) when
   assignment is mandatory, or a separate medium constraint when it is a
   preference. Do not duplicate the assignment penalty inside overlap rules.

Never precompute the rule's **verdict per planning entity** — a
`Lesson.feasible: Vec<bool>` or per-slot `allowed: Vec<bool>` scored by a
constraint. It hides the rule from score analysis and is the classic way an
invalid plan is returned with a clean-looking constraint module.

Factual input data owned by a fact is different and is expected: a teacher's
availability calendar, a slot's start/end, or a prepared travel-time matrix is
read by the constraint and scored there (see the availability recipe below and
`routing-and-maps.md`). The test is whether the data is an observation about the
world or a copy of the rule's decision.

## Weight vocabulary

```rust
use solverforge::prelude::*;

<HardSoftScore as Score>::one_hard()   // one hard violation
<HardSoftScore as Score>::one_soft()   // one soft violation
<HardSoftScore as Score>::zero()       // no violation
```

- Hard streams take a weight function wrapped by `hard_weight(fn)`, where `fn`
  returns a `Score`.
- Soft `penalize`/`reward` take the weight function directly.
- The static constants `HardSoftScore::ONE_HARD` / `ONE_SOFT` work too (used by
  the `unassigned()` example below).

## Verified patterns

`unary`, `reward`, `pair`, and `join` below were verified end to end (compiled
and solved without panic, producing a score). The grouped patterns use the same
wiring rule; treat their collectors as stubs and verify each with a real solve.

### unary — penalize or reward matching entities

When: one entity's own state is illegal or desirable (unassigned, over a
threshold, on a bad day). No join, no grouping.

```rust
use crate::domain::{Plan, Task};
use solverforge::prelude::*;
use solverforge::IncrementalConstraint;

/// HARD: penalize every task that carries a negative priority.
pub fn constraint() -> impl IncrementalConstraint<Plan, HardSoftScore> {
    ConstraintFactory::<Plan, HardSoftScore>::new()
        .for_each(Plan::tasks())
        .penalize(hard_weight(unary_weight))
        .named("required_positive")
}

fn unary_weight(task: &Task) -> HardSoftScore {
    if task.priority < 0 { <HardSoftScore as Score>::one_hard() }
    else { <HardSoftScore as Score>::zero() }
}
```

For soft: `.reward(unary_weight)` (or `.penalize(unary_weight)`) with a
`one_soft()` body and no `hard_weight`.

### unassigned — hard "every entity must be assigned"

When: assignment is mandatory. The stream is empty when all are assigned.

```rust
ConstraintFactory::<Plan, HardSoftScore>::new()
    .for_each(Plan::tasks())
    .unassigned()
    .penalize(HardSoftScore::ONE_HARD)
    .named("required_assignment")
```

The variable must allow unassigned values (`--allows-unassigned`).

### pair — penalize conflicting pairs of the same entity type

When: two entities with the same assigned value conflict (double-booking,
overlap, same resource twice). Uses a self-join keyed by the planning variable.

```rust
use crate::domain::{Plan, Task};
use solverforge::prelude::*;
use solverforge::stream::joiner::equal;
use solverforge::IncrementalConstraint;

/// HARD: two distinct high-priority tasks must not share a resource.
pub fn constraint() -> impl IncrementalConstraint<Plan, HardSoftScore> {
    ConstraintFactory::<Plan, HardSoftScore>::new()
        .for_each(Plan::tasks())
        .join(equal(|task: &Task| task.resource_idx))
        .penalize(hard_weight(pair_weight))
        .named("no_double_book")
}

fn pair_weight(left: &Task, right: &Task) -> HardSoftScore {
    if left.priority >= 3 && right.priority >= 3 {
        <HardSoftScore as Score>::one_hard()
    } else {
        <HardSoftScore as Score>::zero()
    }
}
```

A self-join visits each unordered pair **exactly once** and never pairs an
entity with itself, so one match is one violation. Do not add a self-pair guard
and do not halve the weight.

### join — penalize an entity against a joined fact

When: the entity's assigned value must satisfy a property of the fact it points
at (capacity, skill, distance, compatibility). Join key on the entity side is
the scalar variable (an index); the fact side needs a numeric field to join on.

```rust
use crate::domain::{Plan, Resource, Task};
use solverforge::prelude::*;
use solverforge::stream::joiner::equal_bi;
use solverforge::IncrementalConstraint;

/// HARD: a task's demand must not exceed its assigned resource capacity.
pub fn constraint() -> impl IncrementalConstraint<Plan, HardSoftScore> {
    ConstraintFactory::<Plan, HardSoftScore>::new()
        .for_each(Plan::tasks())
        .join((
            Plan::resources(),
            equal_bi(entity_join_key, fact_join_key),
        ))
        .penalize(hard_weight(join_weight))
        .named("capacity")
}

fn entity_join_key(task: &Task) -> Option<usize> { task.resource_idx }
fn fact_join_key(resource: &Resource) -> Option<usize> { resource.slot.checked_sub(1) }

fn join_weight(task: &Task, resource: &Resource) -> HardSoftScore {
    if task.demand > resource.capacity {
        <HardSoftScore as Score>::one_hard()
    } else {
        <HardSoftScore as Score>::zero()
    }
}
```

Facts have no implicit index. When you need "the fact at index i", add an
explicit `slot:usize` (or similar) field to the fact so both sides can produce a
comparable key. For enum/string compatibility, key both sides to the same
`String`/id.

### reward — soft incentive for matching entities

When: a desirable property should be maximized. `--reward` implies soft.

```rust
ConstraintFactory::<Plan, HardSoftScore>::new()
    .for_each(Plan::tasks())
    .reward(reward_weight)
    .named("prefer_high_priority")

fn reward_weight(task: &Task) -> HardSoftScore {
    if task.priority >= 0 { <HardSoftScore as Score>::one_soft() }
    else { <HardSoftScore as Score>::zero() }
}
```

### balance — spread a metric across groups

When: fairness / load balancing (workload per employee, load per machine). The
collector is macro-generated; replace the placeholder key/metric functions with
real accessors.

```rust
use solverforge::stream::collector::LoadBalance;

ConstraintFactory::<Plan, HardSoftScore>::new()
    .for_each(Plan::tasks())
    .group_by(scope_key, load_balance(group_key, metric))
    .penalize(balance_weight)
    .named("balanced_load")
```

`scope_key: fn(&Task) -> usize` groups the balance problem, `group_key` is the
bucket (usually the assigned value), `metric: fn(&Task) -> i64` is the weight
carried. The skeleton's `load_balance<Option<usize>>` type must match your key
type. `--balance` implies soft, so `balance_weight` must return a soft score
(`one_soft()` or a proportional soft weight); do not wrap it with `hard_weight`.

### runs / presence / collect-vec / group-complement / projected-group

These are grouped/collector skeletons over an index-based planning variable:

- `--runs`: consecutive runs of a value (`consecutive_runs`).
- `--presence`: presence of indexed values (`indexed_presence`).
- `--collect-vec`: collect values into a vector (`collect_vec`).
- `--group-complement`: group entities then complement against a fact collection
  (`count()` + `.complement(...)`).
- `--projected-group`: join to facts, project a row, then group (`project` +
  `group_by`).

Each generated file compiles but every collector closure is a `panic!`
placeholder. Implement each closure with real indexing logic and use
`Plan::<collection>()` for all sources. Verify with a real solve: collector
semantics (ordering, index ranges, empty groups) are easy to get subtly wrong.

## Time, windows, and overlaps

Derived from
[`uc-lessons`](https://github.com/SolverForge/solverforge-usecases/tree/main/uc-lessons),
the canonical reference for slot/window scheduling. Read that app before
inventing time arithmetic; the worked pieces are
[`src/domain/timeslot.rs`](https://github.com/SolverForge/solverforge-usecases/blob/main/uc-lessons/src/domain/timeslot.rs),
[`src/constraints/teacher_availability.rs`](https://github.com/SolverForge/solverforge-usecases/blob/main/uc-lessons/src/constraints/teacher_availability.rs),
[`src/constraints/no_teacher_conflict.rs`](https://github.com/SolverForge/solverforge-usecases/blob/main/uc-lessons/src/constraints/no_teacher_conflict.rs),
and
[`src/data/data_seed/timeslots.rs`](https://github.com/SolverForge/solverforge-usecases/blob/main/uc-lessons/src/data/data_seed/timeslots.rs).

- **Facts own the time data.** A `Timeslot`/`Slot` fact holds `day`, `start`,
  `end`, and a dense `index` join key. The entity's `#[planning_variable]` stores
  the slot index. Never store precomputed per-slot durations, minute offsets, or
  "allowed" booleans on the entity.
- **Generate only legal periods.** Build the slot fact list so it already fits
  the window: skip lunch breaks, stop at the day end, and — when the plan must
  not be scheduled in the past — start the horizon at "now". An illegal period
  that is never a candidate cannot be chosen. A past slot left in range will be
  chosen unless a constraint penalizes it, and a penalty can lose to a local
  optimum.
- **Fit duration to the window.** Either make slots the size the entity needs, or
  add a hard join constraint that penalizes an assignment whose duration does
  not fit the slot (`slot.end - slot.start < entity.duration`). State it, do not
  precompute it.
- **Availability lives on the fact that owns it**, as data indexed by slot
  (`Teacher.availability: Vec<bool>`, `Group.availability: Vec<bool>`). The rule
  is a join + filter, not a per-entity matrix:

  ```rust
  ConstraintFactory::<Plan, HardMediumSoftScore>::new()
      .for_each(Plan::lessons())
      .join((
          ConstraintFactory::<Plan, HardMediumSoftScore>::new().for_each(Plan::teachers()),
          equal_bi(|lesson: &Lesson| lesson.teacher_idx,
                   |teacher: &Teacher| Some(teacher.index)),
      ))
      .filter(|lesson: &Lesson, teacher: &Teacher| {
          lesson.timeslot_idx.is_some_and(|slot| {
              !teacher.availability.get(slot).copied().unwrap_or(false)
          })
      })
      .penalize(hard_weight(|_: &Lesson, _: &Teacher| HardMediumSoftScore::of_hard(1)))
      .named("Teacher Availability")
  ```

- **Overlaps: project, then self-join.** Join the entity to its slot fact, project
  only the fields needed to detect a collision, then self-join on the shared
  resource (teacher, room, group) and filter on real interval overlap. This
  handles slots and free intervals alike and scores each pair once:

  ```rust
  .join((slot_stream, equal_bi(|l: &Lesson| l.timeslot_idx,
                               |t: &Timeslot| Some(t.index))))
  .project(|lesson: &Lesson, slot: &Timeslot| AssignedSlot {
      lesson_index: lesson.index,
      teacher_idx: lesson.teacher_idx.unwrap_or(usize::MAX),
      day: slot.day_of_week, start: slot.start_time, end: slot.end_time,
  })
  .join(equal(|row: &AssignedSlot| row.teacher_idx))
  .filter(|a: &AssignedSlot, b: &AssignedSlot| {
      a.lesson_index < b.lesson_index
          && a.day == b.day
          && a.start < b.end
          && b.start < a.end
  })
  .penalize(hard_weight(|_: &AssignedSlot, _: &AssignedSlot| HardMediumSoftScore::of_hard(1)))
  .named("No Teacher Conflict")
  ```

  The self-join already visits each unordered pair exactly once, so the
  `a.lesson_index < b.lesson_index` term is only an explicit orientation guard;
  the symmetric strict `<`/`>` comparisons do the work and treat a lesson ending
  at 10:00 as compatible with one starting at 10:00.

- **Unassigned is its own constraint.** In `uc-lessons`, "every lesson gets a
  timeslot" is a **medium** constraint (a preference to schedule as much as
  possible), while availability and conflicts are hard. Use hard `.unassigned()`
  only when assignment is mandatory. Do not fold the assignment penalty into the
  conflict rules.

## Score-type caveats

- `SoftScore` has no hard level. Every constraint must be `--soft`; the CLI
  rejects hard-generated constraints on a soft-only score.
- `HardMediumSoftScore` gives you a third level for soft-quality tiers.
- Keep `.named(id)` stable: it is the identity used by `check`, `destroy`, and
  any conflict-repair selector.
