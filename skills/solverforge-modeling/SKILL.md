---
name: solverforge-modeling
description: Model a planning or optimization problem end-to-end with the `solverforge` CLI and produce a runnable SolverForge app. Use when a user describes a scheduling, assignment, routing, rostering, sequencing, packing, loading, knapsack, or resource-allocation problem and wants it turned into working code, or when they want to scaffold, extend, or fix a SolverForge project or an existing hand-written SolverForge model (facts, entities, scalar/list planning variables, constraints, score type, demo data, output shell). Covers the CLI scaffold workflow, the existing-model path when the generator commands do not apply, output-shell choice (web / API / MCP / CLI), constraint authoring against the SolverForge stream API, and behavioral verification on shells that expose solving. Do not use for editing solverforge-cli itself.
---

# Modeling planning problems with solverforge-cli

`solverforge` scaffolds a neutral app shell, then grows it into a model through
managed commands. Your job: turn a user's problem statement into a **compiling,
solvable** SolverForge app, not a skeleton. For web/API, a model is finished only
after a real solve has completed without panicking and produced a score. The
generated CLI shell exposes demo-data only; add and test a solve command or state
that behavioral solve verification remains unavailable.

Read this file, then open the reference that matches the step you are on. Keep
`references/constraint-patterns.md` open while writing constraints: it contains
verified implementations for the common unary/reward/pair/join patterns and the
required source/collector rules for the grouped patterns.

## Prerequisites

- `solverforge` on `PATH` (`cargo install solverforge-cli`).
- Rust `1.95+`.
- Confirm the live contract before relying on version numbers:
  `solverforge --version`, `solverforge new --help`,
  `solverforge generate variable --help`.

This skill describes CLI `3.3.1` (scaffold runtime target `solverforge 0.19.7`,
UI `solverforge-ui 0.9.0`, maps `solverforge-maps 2.1.4`, MCP `rmcp 3.5.0`).
Re-derive specifics from the CLI if the version differs.

## First: which project are you in?

Decide this before anything else; it changes which parts of this skill apply.

- **Scaffolded project** — produced by `solverforge new`, so the CLI owns the
  model through `src/domain/mod.rs`, `src/constraints/mod.rs`, managed blocks,
  `solverforge.app.toml`, and `solver.toml`. Follow Steps 0–8 below.
- **Existing model** — a hand-written app that already depends on `solverforge`
  directly (the
  [`solverforge-usecases`](https://github.com/SolverForge/solverforge-usecases)
  apps, `solverforge-calendar`, or any library/test that
  declares `#[planning_solution]` itself). The CLI generator commands
  (`generate`, `destroy`, `check`, `generate data`) and managed blocks do **not**
  exist here; model changes are ordinary Rust edits. Read
  `references/existing-model.md` first, then apply the modeling, constraint, and
  verification rules in this skill directly to the source.

Do not run `solverforge new` on an app that already has a model, and do not
hand-wire managed blocks into one.

## Worked references and docs — read these before guessing

When a rule is hard to express, **find a worked app before reading framework
internals**. These are the primary references (fetch from GitHub if there is no
local checkout):

- [`solverforge-usecases/uc-lessons`](https://github.com/SolverForge/solverforge-usecases/tree/main/uc-lessons)
  — time/fact-based scheduling: timeslot facts own their time data, availability
  lives on the fact that owns it, overlaps are projection + self-join, and
  assignment is a separate constraint. This is the closest reference for any
  "assign an entity to a window/slot" problem.
- [`solverforge-usecases/uc-hospital`](https://github.com/SolverForge/solverforge-usecases/tree/main/uc-hospital)
  — preference/availability/coverage scheduling with `solver.toml` policy and
  per-constraint score tests.

Both show the canonical `solver.toml` policy (construction heuristic, acceptor,
forager) and the per-constraint test idiom `(constraint(),).evaluate_all(&plan)`.
The published apps are `solverforge-lessons` and `solverforge-hospital`.

For API details, read the crate docs first — constraint streams
([`solverforge::stream`](https://docs.rs/solverforge/latest/solverforge/stream/index.html)),
score analysis
([`ScoreAnalysis`](https://docs.rs/solverforge/latest/solverforge/struct.ScoreAnalysis.html),
`evaluate_detailed`, `analyze_snapshot`), and solver configuration (`solver.toml`
/ `SolverConfig`) — then the `solverforge` source. Read `solverforge-macros` /
`solverforge-solver` internals only as a last resort; the macros are wiring, not
modeling guidance.

## Step 0 — Intake: ask the user before you build

Do not guess the model. Ask the user, in one message, for:

1. **The problem, in their words.** One paragraph is enough: what is being
   decided, what makes a plan good, and what makes a plan invalid. Restate it
   back as `facts / entities / variables / constraints` before scaffolding.
2. **The output shell** — the "type of output" they want:
   - `web` — end-to-end: JSON/SSE API **plus** the browser UI (default).
   - `api` — headless HTTP API only, no frontend assets.
   - `cli` — a terminal command-line scaffold, no Axum server or frontend. The
     generated command exposes demo data, not solving, unless you extend it.
   - `mcp` — an MCP server that exposes the retained solve lifecycle as
     schema-typed tools to any MCP-capable agent harness. stdio by default,
     stateless Streamable HTTP at `/mcp` with `--http`. Use it when the consumer
     is an agent/LLM rather than a browser or a human at a terminal.
3. **The constraints**, split into:
   - **Hard** constraints (must hold for a valid plan), and
   - **Soft** constraints (should be optimized).
   Ask explicitly; users usually name only a subset. For each, ask what
   triggers a violation and what data it reads.
4. **Score type** — default `HardSoftScore`. Only change it if the user needs a
   medium level (`HardMediumSoftScore`), decimals (`HardSoftDecimalScore`),
   soft-only (`SoftScore`), or a custom shape (`BendableScore<N, M>`).

Use a structured question with concrete choices for the shell question. Then use
`references/problem-modeling.md` to convert the answer into a concrete model and
echo that model back to the user for confirmation.

## Step 1 — Scaffold

```bash
solverforge new <name> --shell web   # or api / cli / mcp
cd <name>
```

- `<name>` starts with a letter; letters, digits, `-`, `_` only.
- `--skip-git` skips `git init`; `--skip-readme` skips the README.
- The scaffold is the same neutral model for every shell; the shell only changes
  which adapters (Axum routes, static UI, Clap, or MCP tools) are generated.

## Step 2 — Establish the solution identity first

If the user does not want the default `Plan` solution name, rename it **now**,
before adding any facts or entities:

```bash
solverforge generate solution schedule --score HardSoftScore
```

This is the one chance to replace the neutral scaffold automatically. After
facts/entities exist, `generate solution` refuses; you would have to
`destroy solution` (which empties the solution's collections and does **not**
re-wire them). See `references/gotchas.md`.

## Step 3 — Add facts and entities

Facts are immutable inputs; entities are the things the solver assigns.

```bash
solverforge generate fact resource --field capacity:i32 --field slot:usize
solverforge generate entity task --field demand:i32 --field ready_at:i64
```

- Names are `snake_case`; the CLI PascalCases the struct (`task` → `Task`) and
  naively pluralizes the collection (`task` → `tasks`).
- `--field name:Type` is repeatable. Every fact/entity gets `id: String` (and
  facts get `name: String`) automatically; do not declare them yourself.
- Watch irregular plurals: the collection name is used verbatim by variables
  (`--range`, `--elements`, data generation), so prefer regular nouns.

## Step 4 — Add planning variables

Scalar = one value per entity. List = an ordered sequence owned by an entity.

```bash
# scalar over a fact collection (value is an index into that collection)
solverforge generate variable resource_idx --entity Task --kind scalar \
  --range resources --allows-unassigned

# scalar over an integer range (value is the integer itself)
solverforge generate variable start_slot --entity Task --kind scalar --countable-range 0..24

# ordered list over a fact collection
solverforge generate variable stops --entity Route --kind list --elements visits
```

- Use `--allows-unassigned` only when `None` is a legal planning state or a
  specific runtime resource (such as an assignment scalar group) requires it.
  Scalar values are reusable unless a constraint enforces exclusivity, so entity
  count alone is not a reason. The field is `Option<usize>` either way.
- A scalar over a fact collection stores the **index** into that collection, not
  the fact itself.
- Ordered sequences and routes are **list** variables. Never model sequence
  topology as a scalar predecessor field.
- `--domain cvrp` selects the stock CVRP list profile. It owns its distance
  meters, route/savings hooks, metric class, and solution trait; those cannot be
  overridden alongside it, and the solution must satisfy the runtime's CVRP
  contract. For custom sequences, pass the specific list metadata flags instead
  (see `references/cli-workflow.md`).

## Step 5 — Author constraints (this is where models fail)

Generate one module per constraint with an **explicit pattern flag**, generate a
real implementation, and register nothing by hand.

```bash
solverforge generate constraint capacity --join --hard
solverforge generate constraint prefer_early --unary --soft
```

Always pass a pattern flag. Without one the CLI opens an interactive wizard that
fails in a non-interactive agent shell (`prompt error: IO error: not a
terminal`).

Patterns: `--unary --pair --join --balance --reward --runs --presence
--collect-vec --group-complement --projected-group`. `--hard` is the default;
`--soft` opts into a soft constraint. `--balance` and `--reward` imply soft.

### The localizing-source rule (read this even if you read nothing else)

The generated skeleton is a **compiling stub full of `panic!` placeholders**.
Replace every placeholder function body with real domain logic before enabling
the constraint. The stream sources are already correct: the skeleton streams
from the `#[planning_solution]`-generated collection accessors, never from a
hand-written extractor.

`#[planning_solution]` generates a public associated accessor for each
collection: `Plan::tasks()` for `#[planning_entity_collection] pub tasks:
Vec<Task>`, and `Plan::resources()` for `#[problem_fact_collection]`. These
carry the change-source metadata the incremental engine needs. A hand-written
`fn entity_items(solution: &Plan) -> &[Task]` has `ChangeSource::Unknown`:

- `initialize` (full evaluation) tolerates `Unknown`, but the first
  solver-applied move calls `on_insert`/`on_retract`, and **every** pattern —
  unary and reward included — panics there with:
  `constraint <name> received descriptor <n>, but source Unknown cannot localize entity indexes`.

So wire **every** stream through the generated accessor — never introduce a
hand-written extractor:

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

Self-joins (entities sharing a key) use `joiner::equal(|e: &Task| key)`.
Entity↔fact joins use `joiner::equal_bi(entity_key, fact_key)`. Fact rows carry
no implicit index, so when you need to join to "the Nth fact" add an explicit
numeric field (`slot`/`index`) to the fact and read it in `fact_join_key`.

`references/constraint-patterns.md` has the full catalog with when-to-use and
verified implementations, plus the `hard_weight` / `one_hard` / `one_soft` /
`zero` weight vocabulary and the score-type caveats.

## Step 6 — Generate demo data

```bash
solverforge generate data --size standard
```

`src/data/data_seed.rs` is **compiler-owned**: domain-shape commands such as
`generate fact/entity/variable` re-render it from the current structs and
`--size` counts. Constraint-only changes do not. Do not hand-edit it; later
domain-shape changes overwrite it. Generated values
are structurally useful index-based samples, not realistic domain data. If the
user needs real data, load it from a new module or the stable `src/data/mod.rs`
wrapper rather than editing the seed.

## Step 7 — Verify (mandatory, in this order)

```bash
solverforge check          # structure, managed blocks, model resources, solver.toml
cargo check                # the code must actually compile
```

`check` passing does **not** prove the model works: it never runs the solver and
never sees placeholder `panic!`s. Run a real solve through web/API or through a
CLI solve entry point/integration test you add.

Also add one per-constraint test per rule with exact score-delta assertions
(`(constraint(),).evaluate_all(&plan)`); it is the only fast gate that catches a
rule modeled as a precomputed flag. See gate 4 in `references/verification.md`.
An existing hand-written model has no `check` step at all — start at
`cargo test` (`references/existing-model.md`).

For `web`/`api`:

```bash
solverforge server --debug
# Or locate this loaded skill directory and run:
<skill-dir>/scripts/solve-smoke-test.sh .
```

Then drive the documented API (details in `references/verification.md`):

1. `GET /health`
2. `GET /demo-data` → `defaultId`
3. `GET /demo-data/<ID>` → the plan JSON
4. `POST /jobs` with that plan → `{ "id": ... }`
5. Poll `GET /jobs/<id>/status` until `lifecycleState` settles.

A passing result means: the server booted, the job reached `COMPLETED`, current
and best scores are non-null, and the log contains **no** constraint panic.
`SOLVING` after the timeout, `CANCELLED`, `FAILED`, or a panic is failure — fix
the model (often the localizing-source rule) and re-verify.

For `cli`, there is no generated solve command. Run `cargo run -- demo-data` to
verify compilation and serialization only. To claim a working optimizer, add a
solve subcommand backed by `src/solver/service.rs` (or a library integration
test that drives it) and verify a terminal score; otherwise report this
limitation explicitly.

For `mcp`, the generated binary is itself the MCP server, and the agent's own
harness is the client. Boot it, register it, and verify by calling a tool:

```bash
cargo run --release -- --http    # Streamable HTTP at http://127.0.0.1:7860/mcp
solverforge server               # equivalent; selects --http for mcp shells
# The HTTP transport is unauthenticated and shares one job store across all
# callers: keep it on loopback or put an authenticating proxy in front.

# Register the server in the harness you are running in, then reload so the
# harness rescans its MCP config. `connect` writes an in-project config:
solverforge connect --write opencode   # opencode.json
solverforge connect --write claude     # .mcp.json
solverforge connect --write cursor     # .cursor/mcp.json
solverforge connect --write vscode     # .vscode/mcp.json
```

The server exposes `list_demo_data`, `get_demo_data`, `solve`, `get_status`,
`get_best_solution`, `analyze_solution`, `get_telemetry`, `get_candidate_trace`,
`pause`, `resume`, `cancel`, and `delete`. `solve` is task-backed for
task-capable clients (retained `jobId` in result metadata) and returns an
immediate summary to others.

Verify through the harness's own MCP client, not a separate binary: after
reloading, call `list_demo_data`, then `solve` and poll `get_status` until the
job reaches a terminal state, and read `get_best_solution` for a scored
snapshot. The CLI's own pipeline tests drive the generated server with `rmcp`
over stdio and Streamable HTTP, which is the protocol-level reference if a
harness integration misbehaves.

## Step 8 — Report

Tell the user: the model you built, the shell, the score type, the constraints
(hard/soft) and what each does, the exact verification command you ran, the
observed score, and any remaining limitations. Record repository changes as
small conventional commits if the user asks for commits.

## Hard rules

- Never precompute a rule's **verdict per planning entity** (a
  `Vec<bool>`/`Vec<i32>` "this entity may use this value" flag, or a keyed
  verdict table) and feed it to a penalty constraint. State the rule over domain
  objects inside the constraint. A candidate/value-range provider may compute
  which values are *available*, but it must not become the scoring input.
- Factual input data owned by a fact is not a verdict: an availability calendar,
  a slot's start/end, or a prepared travel-time matrix is read by the constraint
  and scored there. The test is whether the data observes the world or repeats
  the rule's decision.
- Never add a construction heuristic, greedy initializer, or post-solve
  sanitizer/repair pass in application code. Restrict candidates through the
  variable's candidate/value-range metadata and set search policy in
  `solver.toml`.
- Hard constraints are penalties, not filters: a returned solution can still
  violate one. Keep invalid candidates out of the model's value range, or gate
  grouped/repair moves with `require_hard_improvement`; never guard the result
  after solving. See `references/constraint-patterns.md`.
- Explain a plan with framework analysis (`analyze()`, `analyze_snapshot`,
  `evaluate_detailed`), not by re-deriving constraint predicates in application
  code.
- Never hand-wire managed blocks, `src/constraints/mod.rs`, or the solution's
  collections. Use the CLI commands. (Scaffolded projects only; an existing
  hand-written model is edited as ordinary Rust — see
  `references/existing-model.md`.)
- MCP-shell projects expose MCP tools, not Axum routes: do not expect `/health`,
  `/jobs`, or `solverforge routes` there.
- Always pass an explicit constraint pattern flag.
- Rename/replace the solution before adding facts and entities.
- `src/data/data_seed.rs` and web-shell `static/generated/ui-model.json` are
  compiler-owned; `solverforge.app.toml`, `solver.toml`, and the managed blocks
  are the CLI's surfaces.
- Ordered sequences are list variables; do not use scalar predecessor fields.
- Finish web/API only after a real solve completes. For MCP, register the server
  in the running harness and verify by calling `solve`/`get_status`/
  `get_best_solution` through that harness's own client.
  For CLI, either implement and verify a solve entry point or explicitly report
  that the generated shell only proves data serialization.

## Reference index

| File | Use it for |
| --- | --- |
| `references/problem-modeling.md` | Converting a problem statement into facts, entities, variables, and hard/soft constraints; intake template; why not to precompute rules or model time incorrectly. |
| `references/existing-model.md` | Non-scaffolded apps: locating and editing an existing `#[planning_solution]` model when the generator commands do not apply. |
| `references/cli-workflow.md` | Exact command surface, flags, ordering, and shell-specific run steps. |
| `references/constraint-patterns.md` | Every constraint pattern and its source rules; verified code for unary/reward/pair/join; why hard constraints are penalties; the time/window/overlap recipe. |
| `references/output-shells.md` | `web` vs `api` vs `cli` vs `mcp`, what each generates, and how to run each. |
| `references/verification.md` | `check` vs compile vs real solve vs per-constraint tests; the API smoke flow and helper script; explaining a plan from framework analysis. |
| `references/gotchas.md` | Managed blocks, compiler-owned files, ordering traps, score-type limits, hard-constraint and precompute traps. |
| `references/advanced-resources.md` | Countable ranges, scalar hooks, list metadata, scalar groups, conflict repair, candidate traces. |
| `references/routing-and-maps.md` | When and how to use `solverforge-maps` for road-network travel times, matrices, and route geometry. |
