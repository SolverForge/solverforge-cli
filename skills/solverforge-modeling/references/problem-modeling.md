# Problem modeling

Convert the user's problem into SolverForge's four building blocks, confirm it
back, then build it.

## The intake template

After the user answers the Step 0 questions, write this back to them before
scaffolding:

```
Solution:   <Name> (<ScoreType>)
Facts:      <Plural> — <why immutable>
Entities:   <Plural> — <what gets a value assigned>
Variables:  <Entity>.<field> [scalar|list] over <source>
Hard:       <id> — <what must hold>
Soft:       <id> — <what should be optimized>
Output:     <web|api|cli>
```

Do not scaffold until the user agrees with this restatement. It prevents the
common failure of building the wrong model and only discovering it at solve time.

## The four blocks

| Block | Question | SolverForge |
| --- | --- | --- |
| Problem fact | What is fixed input that the solver must not change? | `generate fact` → `#[problem_fact]`, `Vec<T>` collection |
| Planning entity | What am I deciding *about*? Each instance gets assigned. | `generate entity` → `#[planning_entity]`, `Vec<T>` collection |
| Planning variable | What is the decision itself? | `generate variable` → `scalar` or `list` on an entity |
| Constraint | What makes a plan valid or good? | `generate constraint` → one module, hard or soft |
| Score | How are hard/soft violations ranked? | `generate solution --score` / `generate score` |

### Facts vs entities

- If the solver may change it, it is an entity.
- If it is an input the solver reads but must not mutate, it is a fact.
- Counts (capacity, demand, priority, duration) belong on facts/entities as
  fields; they are **not** planning variables.
- A "resource", "employee", "vehicle", "machine", "room" is usually a fact; the
  thing being scheduled/assigned/routed is usually an entity.

### Scalar vs list

- **Scalar** — one decision value per entity. Two sources:
  - `--range <fact_plural>`: the value is the **index** into that fact
    collection. Add `--allows-unassigned` if an entity may go unassigned.
  - `--countable-range <from..to>`: the value is an integer in `[from, to)`.
    Use for time slots, positions, quantities.
- **List** — an ordered sequence owned by the entity (routes, tours, line
  sequences, precedence-ordered jobs). Uses `--elements <fact_plural>`. Use the
  `--domain cvrp` profile for vehicle-routing, or the explicit list-metadata
  flags for custom sequences.

Never model an ordered sequence as a scalar predecessor field. That is the most
common modeling error and it is explicitly not supported.

## Model the rule, don't precompute it

The most damaging shortcut is to decide legality in Rust ahead of the solver.
Do not build per-entity feasibility or cost matrices (`task.feasible:
Vec<bool>`, per-slot `allowed: Vec<bool>`, minute-offset tables) and then score
a penalty against that flag. It compiles, it scores, and it hides the rule from
score analysis while letting the solver return plans the matrix was supposed to
forbid.

- State each rule once, over the domain objects, inside a constraint
  (`constraint-patterns.md`). The constraint reads the entity's assigned value
  and the fact that owns the relevant data.
- Keep derived data on the object that owns it as **input** (a fact's
  availability calendar, a slot's start/end), not as a per-entity verdict.
- Never add a greedy initializer, construction heuristic, or post-solve
  sanitizer in application code. Candidates are restricted through the
  variable's candidate/value-range metadata; search policy lives in
  `solver.toml`.
- Explain results with `analyze()` / `evaluate_detailed`, not with a second copy
  of the rule written as reporting predicates.

## Modeling time and windows

- **Duration belongs to the entity; the window belongs to a fact.** If a task
  has a duration and a timeslot has a start/end, add a hard constraint that the
  duration fits the window, or size the slot facts to the entity. Do not
  precompute "end minute" offsets onto the entity.
- **"Now" is a horizon boundary, not a penalty.** If the plan must not be
  scheduled in the past, build the slot/visit facts so periods before `now` are
  never generated. A past slot left in the candidate range will be chosen unless
  a constraint catches it, and a hard penalty can lose to a local optimum.
- **Existing busy intervals are facts, not entities.** Model a pre-existing
  booking/absence as fact data that a conflict constraint joins against, so
  overlap is computed from the same stream logic as new assignments.
- **Overlaps use project-then-self-join**, not dense-slot equality. See the
  recipe and worked `uc-lessons` links in `constraint-patterns.md`.
- **Unassigned is a separate concern.** Use a hard `.unassigned()` when
  assignment is mandatory, or a medium penalty when "schedule as much as
  possible" is a preference. Do not duplicate the assignment penalty inside
  overlap/conflict rules.

## Score type and hardness

- Default `HardSoftScore`: hard = validity, soft = quality. Almost always this.
- `HardMediumSoftScore` when the user has a middle tier.
- `HardSoftDecimalScore` for fractional weights.
- `SoftScore` for pure optimization; it has no hard level, so every generated
  constraint must be `--soft` (the CLI rejects hard constraints on `SoftScore`).
- `BendableScore<N, M>` for custom counts of hard/soft levels.
- Soft weights let you express priority: a violation that costs 10 is ten times
  as bad as one that costs 1. Use explicit `one_hard()` / `one_soft()` only when
  all violations are equal.

## Field types

`--field name:Type` accepts any Rust type the struct can hold. Practical menu:

| Type | Use for | Generated sample |
| --- | --- | --- |
| `String` | labels, ids, categories | `"<stem>-<field>-<idx>"` |
| `i32`, `i64` | counts, weights, priorities | `(idx % 7) - 2`, `(idx % 11) - 5` |
| `f32`, `f64` | coordinates, durations, loads | `(idx % 9) * 1.25`, `(idx % 13) * 1.25` |
| `bool` | flags | `idx % 2 == 0` |
| `usize` | external indexes, slots | `idx + 1` |
| `Option<T>` | optional data | `None` |
| `Vec<T>` | inline lists | `vec![]` |

These sample formulas are fixed by the CLI. If a constraint depends on field
values having a particular relationship (e.g. demand exceeding capacity), the
defaults may make the constraint trivially satisfied or trivially violated. Pick
field types/names with the sample formulas in mind, or load real data (see
`gotchas.md`).

## Worked examples

### Scheduling / assignment

```
Solution:   Schedule (HardSoftScore)
Facts:      employees — fixed workforce
Entities:   shifts — each shift needs an employee
Variables:  Shift.employee_idx scalar over employees, --allows-unassigned
Hard:       no_overlap — one employee cannot cover two shifts at the same time
Soft:       prefer_senior — reward assigning a shift's preferred employee
Output:     web
```

### Vehicle routing

```
Solution:   Plan (HardSoftScore)
Facts:      visits, vehicles
Entities:   routes — each route is one vehicle's ordered tour
Variables:  Route.stops list over visits, --domain cvrp
Hard:       capacity — vehicle capacity must cover its route demand
Soft:       distance — minimize total travel
Output:     api
```

List variables model ordered sequences in general — routes, ordered
assignments, job sequencing, precedence lists. Most never touch geography; use
`references/routing-and-maps.md` only when the ordering cost is real road
travel.

### Time-slot placement

```
Solution:   Plan (HardSoftScore)
Facts:      rooms
Entities:   meetings
Variables:  Meeting.room_idx scalar over rooms --allows-unassigned
            Meeting.start_slot scalar --countable-range 0..24
Hard:       room_no_overlap — no two meetings share a room and slot
Soft:       prefer_morning — reward early slots
Output:     web
```

This dense countable-range form is fine only when every slot is legal and
interchangeable. When slots have real times, per-teacher/room availability, or a
"now" horizon, model slots as **facts** that own `day`/`start`/`end` and let the
hard constraints read them, generating only legal periods. See the
`uc-lessons`-derived time/window/overlap recipe in `constraint-patterns.md`.

## Constraint inventory checklist

For each constraint, capture: exact `snake_case` id, hard/soft, the entities and
facts it reads, the trigger condition, and the weight. A constraint with no
condition is not a constraint. Feed these straight into `generate constraint`
plus a rewritten body (see `constraint-patterns.md`).
