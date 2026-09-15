# Existing hand-written models

Use this path when the app already declares its own SolverForge model in Rust
and was **not** produced by `solverforge new`. Examples: `solverforge-calendar`,
a domain library that declares `#[planning_solution]`, or an app embedding
`solverforge` directly. The CLI generator commands and their managed blocks do
not apply here; model changes are ordinary Rust source edits.

## Confirm it is an existing model, not a scaffold

You are on this path when the project has no `solverforge.app.toml` (and no
`.solverforge/` templates) but does have a struct annotated with
`#[planning_solution]`. Do not run `solverforge new` on it, and do not introduce
managed blocks. If `solverforge.app.toml` exists, it is a scaffolded project —
use the main workflow instead.

## Locate the model

| Piece | Where to look |
| --- | --- |
| Solution | the `#[planning_solution(...)]` struct (often `src/domain/plan.rs` or a `domain` module). Declares the fact/entity collections, score, and the constraint entry point. |
| Entities | `#[planning_entity]` structs carrying `#[planning_variable(...)]` fields. |
| Facts | `#[problem_fact]` structs; immutable inputs the solver reads. |
| Constraints | functions returning `impl IncrementalConstraint<Plan, Score>`, registered by the function named in `#[planning_solution(constraints = "...")]` (commonly `create_constraints`). |
| Solver policy | the file named by `#[planning_solution(solver_toml = "...")]`, or the conventional `solver.toml`. |
| Data / demo data | wherever the app builds or loads its facts and entities; there is no compiler-owned seed. |

## Wiring points you must preserve

- `#[planning_id] pub id: String` (or another stable id) on every fact and
  entity. Keep it stable; transport and snapshots key on it.
- A dense `#[serde(skip)] pub index: usize` is the usual solver-facing join key.
  Recompute it from collection position after construction and after decoding a
  transport payload, so the solver always sees normalized indexes. Many apps
  (and the
  [`uc-*` references](https://github.com/SolverForge/solverforge-usecases)) put
  this in a `rebuild_derived_fields`-style method called from `Plan::new`.
- `#[planning_variable(value_range_provider = "collection")]` names a
  `#[problem_fact_collection]` (or use `countable_range = "0..n"`). The field is
  `Option<usize>` and stores an **index**, not the fact. Filter stale
  out-of-range indexes to `None` during normalization.
- Use the `#[planning_solution]`-generated `Plan::collection()` accessors as
  constraint stream sources. The localizing-source rule is identical to
  scaffolded apps (see `constraint-patterns.md`).
- Ordered sequences are **list** variables. Never model sequence topology as a
  scalar predecessor field.
- Add a rule by writing a module and adding one entry to the constraint-assembly
  function. Do not add a second, parallel copy of a rule for reporting.

## Change discipline

- Make the smallest edit in the file that owns the rule. Model each domain rule
  once, as a constraint over domain objects.
- Do not precompute feasibility or cost into per-entity matrices; do not add
  greedy initializers, construction heuristics, or post-solve sanitizers in app
  code. Candidate restriction belongs in the variable's value-range/candidate
  metadata; search policy belongs in `solver.toml`. See the Hard Rules in
  `SKILL.md`.
- When the model is refactored, delete the old mechanism instead of layering a
  replacement beside it.
- Explain solved plans with framework analysis (`analyze()`,
  `analyze_snapshot`, `evaluate_detailed`), not with re-derived predicates.

## Verify (no `solverforge check` here)

There is no `check`, no `/jobs` smoke script, and no generated solve command.
Use this order:

1. `cargo check` / `cargo test` — the fast loop. Add one per-constraint test per
   rule with exact score-delta assertions (`verification.md`).
2. A behavioral solve — add a library integration test or a solve entry point
   that drives `SolverManager`/`SolverService` to a terminal score, and assert
   the observed score and a hard-feasible solution. A run that terminates with a
   nonzero hard score is not "done"; it means invalid candidates are reachable.
