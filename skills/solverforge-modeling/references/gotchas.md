# Gotchas

Hard-won behaviors that break otherwise-correct models.

## Managed blocks are the CLI's edit surface

Generated files carry `// @solverforge:begin <label>` / `// @solverforge:end
<label>` regions. `generate`/`destroy` rewrite inside those regions. Do not
delete, duplicate, or move the markers, and do not hand-wire collections or
constraint calls. If you need a custom entity/solution template, it must emit
the same canonical blocks (`.solverforge/templates/entity.rs.tmpl`,
`.solverforge/templates/solution.rs.tmpl`).

## Rename the solution before adding anything

`solverforge generate solution <name>` replaces the neutral scaffold **only
while it is still neutral**. After facts/entities/variables exist it errors.
`destroy solution` then `generate solution` does not re-wire the existing
collections into the new solution, so entities/facts silently fall out of the
solution and `info` shows none. Correct order: scaffold → rename solution →
add facts/entities/variables.

## `src/data/data_seed.rs` is compiler-owned and aggressively regenerated

Domain-shape commands such as `generate fact/entity/variable` re-render
`data_seed.rs` from the current structs and demo sizes; constraint-only changes
do not. Hand edits are lost on the next domain-shape change.
- Generated values are index-based samples, not domain data.
- `usize` samples are `idx + 1` while scalar variable indices are `idx` (0-based).
  Joins that compare a scalar index to a `usize` slot are off by one unless you
  account for it (see the `checked_sub(1)` in `constraint-patterns.md`).
- For real data, add a loader module or use the stable `src/data/mod.rs` wrapper.
  Do not fight the generator.

## Web UI projection is compiler-owned

`static/generated/ui-model.json` and the domain entries in
`static/sf-config.json` are projections of the model. Mutate the model with the
CLI; do not edit the projection. `static/sf-config.json` is the user-facing
web-shell config seam.

## Generated constraints are stubs that compile

`solverforge generate constraint` writes real structure plus `panic!`
placeholders. `check` and `cargo check` both pass on an untouched stub. A stub
that is not on any hot path may even let a solve complete with a wrong score.
Replace every placeholder and verify a real solve through web/API or a solve
entry point/integration test you add.

## Hard constraints are penalties, not filters

A hard constraint **penalizes** an invalid candidate; it does not remove it from
search. A solve can terminate with a nonzero hard score and return a plan that
violates a rule if construction started invalid and local search could not
repair it. Do not add a post-solve sanitizer or a re-checked guard. Keep invalid
values out of the variable's candidate/value range, or set
`require_hard_improvement = true` on the grouped/repair selector (the CLI writes
this for `generate scalar-group` and conflict repair). See
`constraint-patterns.md`.

## Model the rule, don't precompute it

Do not encode a domain rule as a per-entity feasibility verdict scored by a
penalty, and do not add greedy initializers or construction heuristics in app
code. State the rule over domain objects in a constraint; restrict candidates
through value-range/candidate metadata; put search policy in `solver.toml`.
Explain results with `analyze()`/`evaluate_detailed`, not a duplicated predicate.
Factual input data owned by a fact — an availability calendar, a slot's
start/end, a prepared travel-time matrix — is allowed and is what constraints
read. See `problem-modeling.md`.

## The localizing-source trap

The single most common runtime failure. A hand-written
`fn entity_items(solution: &Plan) -> &[T]` has `ChangeSource::Unknown`; any
enabled constraint over it panics on the first solver-applied move — any
pattern, unary and reward included — with
`source Unknown cannot localize entity indexes`. The generated skeleton already
streams from the `Plan::<collection>()` accessors; keep it that way and never
swap in a hand-written extractor. See `constraint-patterns.md`.

## Nearby selectors require nearby hooks

`nearby_change_move_selector` / `nearby_swap_move_selector` in `solver.toml`
compile only when the targeted scalar variable declares nearby hooks
(`nearby_value_candidates` / `nearby_entity_candidates`; the matching distance
meters are optional). A solver policy copied from an app that declares them —
the hospital example does — fails on a hook-less model with
`<Entity>.<variable> does not provide required nearby scalar value source`.
Either drop the nearby selector blocks (the default policy then emits ordinary
change/swap moves) or regenerate the variable with
`solverforge generate variable ... --nearby-value-candidates <fn>
--nearby-entity-candidates <fn>` and implement the hook functions.

## Never omit the constraint pattern flag

With no `--unary/--pair/--join/...`, the CLI launches an interactive wizard and
fails in a non-interactive agent: `prompt error: IO error: not a terminal`.
Always pass exactly one pattern flag. `--hard`/`--soft` alone do not select a
pattern.

## MCP shells have no REST surface

An mcp-shell project exposes solver tools over MCP, not Axum routes. There is no
`/health`, `/jobs`, or SSE endpoint, and `solverforge routes` is rejected. The
binary's default transport is stdio (stdout is the protocol channel, so the
runtime `console` banner is disabled); use `--http` or `solverforge server` for
stateless Streamable HTTP at `/mcp`.

The HTTP transport is unauthenticated and has no per-caller isolation: every
connection shares one solver and task store, so `--host` on a non-loopback
address lets anyone who can reach the port start solves and inspect, cancel, or
delete jobs. Host validation only rejects DNS-rebinding headers. Leave `--host`
on loopback unless the network is trusted; stdio has no such exposure.

## Naive pluralization

Collections are naively pluralized: `visit` → `visits`, `route` → `routes`, but
`category` → `categories`, `box` → `boxes`, and irregulars like `person` are
wrong. The plural is what `--range`/`--elements` and the data generator key on.
Prefer regular nouns.

## Unassigned semantics

Scalar variables are `Option<usize>` regardless of `--allows-unassigned`. Use
`--allows-unassigned` only when `None` is a legal planning state or an advanced
runtime resource explicitly requires it. Scalar values are reusable unless a
constraint enforces exclusivity, so having more entities than candidate values
does not itself require unassigned values. Pair an allowed `None` state with an
`unassigned()` hard constraint when assignment is ultimately mandatory.

## Destroy dependencies block

`solverforge destroy` refuses to remove an entity/variable/constraint that a
scalar group or conflict repair still references. Destroy dependents first
(`--yes scalar-group <name>` / `--yes conflict-repair <id>`).

## Score-type constraints

- `SoftScore` has no hard level: only `--soft` constraints are allowed.
- `generate score <Type>` must name a supported type, or the CLI rejects it.

## `solverforge.app.toml` is a projection, not an input

It is serialized from the parsed domain after every mutation. Edit the model with
commands. It does contain user-facing metadata (demo sizes, shell, runtime
targets) that the CLI manages; leave managed fields alone.

## Persistent prefs

`.solverforgerc` (project root, then `~/.solverforgerc`) recognizes only `port`,
`no_color`, `quiet`. CLI flags override it.
