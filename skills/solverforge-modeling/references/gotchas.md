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

## The localizing-source trap

The single most common runtime failure. Hand-written
`fn entity_items(solution: &Plan) -> &[T]` has `ChangeSource::Unknown`; any join,
self-join, `group_by`, complement, or projection over it panics with
`source Unknown cannot localize entity indexes`. Always use the generated
`Plan::<collection>()` accessors. See `constraint-patterns.md`.

## Never omit the constraint pattern flag

With no `--unary/--pair/--join/...`, the CLI launches an interactive wizard and
fails in a non-interactive agent: `prompt error: IO error: not a terminal`.
Always pass exactly one pattern flag. `--hard`/`--soft` alone do not select a
pattern.

## `--shell mcp` does not exist on this line

`possible values: web, api, cli`. MCP is on a separate branch. Do not use or
promise it.

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
