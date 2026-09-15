# Advanced modeling resources

Opt-in surfaces for models that outgrow plain scalar/list variables. These
commands can modify entity fields, planning attributes, solution managed blocks,
metadata projections, and `solver.toml` refs. Rust paths supplied as flags name
functions or traits you still implement.

## Countable-range scalars

```bash
solverforge generate variable start_slot --entity Meeting --kind scalar \
  --countable-range 0..24
```

The value is an integer in `[from, to)` (validated `usize`, `from < to`). Written
as `#[planning_variable(countable_range = "0..24", allows_unassigned = false)]`
with a `pub start_slot: Option<usize>` field. Projected into
`solverforge.app.toml` and the web UI model as numeric value lanes. Use for time
slots, positions, and counts.

## Scalar hook metadata

```bash
solverforge generate variable resource_idx --entity Task --kind scalar --range resources \
  --candidate-values resource_candidates \
  --nearby-value-candidates nearby_resources \
  --nearby-entity-candidates nearby_tasks \
  --nearby-value-distance-meter resource_distance \
  --nearby-entity-distance-meter task_distance \
  --construction-entity-order-key task_priority \
  --construction-value-order-key resource_priority
```

Each value is a Rust path you own. The flags select model-owned candidate/nearby/
distance/construction-order behavior during construction and local search. They
only write `#[planning_variable(...)]` metadata and the projections; implement
the functions in the domain yourself.

## List metadata

Beyond `--domain cvrp`:

```bash
solverforge generate variable stops --entity Route --kind list --elements visits \
  --distance-meter crate::domain::route::route_distance \
  --route-hooks crate::domain::route \
  --element-owner-fn crate::domain::route::visit_owner \
  --construction-element-order-key crate::domain::route::stop_order \
  --precedence-duration-fn crate::domain::route::stop_duration \
  --precedence-successors-fn crate::domain::route::stop_successors
```

The full flag set is in `cli-workflow.md`. These name user-owned Rust
implementations for cross/intra distance, route and Clarke-Wright savings hooks,
savings metric class, element ownership, construction ordering, precedence, and
any additional solution trait. The stock `cvrp` profile owns its meters/hooks/
metric/trait and cannot be combined with them.

## Scalar groups (coupled construction / grouped moves)

When several scalar variables must be solved together (assignment with
capacities/incompatibilities, grouped moves with limits):

```bash
# assignment-backed
solverforge generate scalar-group required_assignment \
  --assignment Task.resource_idx --required-entity required_task

# candidate-backed, multi-target
solverforge generate scalar-group paired_assignment \
  --candidates paired_candidates \
  --target Task.primary_idx --target Task.secondary_idx
```

- Identified by exact scalar-group names and `Entity.field` targets.
- Assignment groups require the target scalar variable(s) to allow unassigned
  values; they accept `--capacity-key`, `--assignment-rule` (requires
  `--sequence-key`), `--position-key`, `--sequence-key`, `--entity-order`,
  `--value-order`.
- Candidate groups require `--candidates` and forbid assignment hooks.
- Limits: `--value-candidate-limit`, `--group-candidate-limit`,
  `--max-moves-per-step`, `--max-augmenting-depth`, `--max-rematch-size`.
- Unless `--skip-solver-config` is passed, the CLI writes grouped construction
  and grouped local-search refs into the CLI-managed `solver-config` region of
  `solver.toml`. `solverforge check` validates those refs across the config
  graph.
- The solution must declare `scalar_groups = "scalar_groups"` in
  `#[planning_solution(...)]`; the CLI adds this when the first group is created.
- Destroy blocks while targets still reference the group.

## Conflict repair selectors

For a specific constraint, add a repair provider that proposes moves addressing
its violations:

```bash
solverforge generate conflict-repair required_assignment \
  --provider repair_required_assignment \
  --selector compound
```

- `--selector` is `compound` (default) or `conflict`.
- Limits: `--max-matches-per-step`, `--max-repairs-per-match`,
  `--max-moves-per-step`; `--include-soft-matches` allows soft matches.
- The `constraint` argument is an exact existing constraint id; the CLI validates
  it. Unless `--skip-solver-config`, it writes a matching `solver.toml` phase.
- The solution must declare `conflict_repairs = "conflict_repairs"`.
- Destroy blocks while conflict repairs reference the constraint; destroy the
  repair first.

## Candidate-trace diagnostics (web / api)

Bounded candidate-pull tracing is opt-in because it can be large:

```bash
solverforge config set candidate_trace.max_entries 100000
```

Then fetch the retained detail from `GET /jobs/{id}/telemetry`. Ordinary status,
snapshot, and SSE payloads never include candidate pulls. `POST /jobs/qualified`
starts the same retained lifecycle with externally attested SHA-256 digests and
a non-empty producer. These are diagnostics, not modeling primitives.
