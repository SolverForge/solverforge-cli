# CLI workflow

The full command surface for modeling. Run every `generate`/`destroy` command
from **inside the project directory** (the one containing `solverforge.app.toml`
and `src/`).

## Scaffold

```bash
solverforge new <name>                         # web shell (default)
solverforge new <name> --shell api             # headless HTTP API
solverforge new <name> --shell cli             # terminal app
solverforge new <name> --shell mcp             # MCP server
solverforge new <name> --skip-git --skip-readme
```

`--shell` accepts `web`, `api`, `cli`, and `mcp`.

## Order of operations

1. `generate solution` (only if renaming; do it first).
2. `generate fact` / `generate entity`.
3. `generate variable` (scalar, countable scalar, or list).
4. `generate constraint` (one per rule).
5. `generate data`.
6. `check` + `cargo check` + a real solve (web/API, or a CLI solve entry point
   you add and test).

Model mutations resync `solverforge.app.toml` and, for web shells,
`static/generated/ui-model.json`. Domain-shape mutations (facts, entities,
variables, solution/score) also regenerate `src/data/data_seed.rs`; constraint-
only changes do not. Never edit projections to model; use the commands.

## Solution and score

```bash
solverforge generate solution <name> --score HardSoftScore
solverforge generate score <Type>
```

- `generate solution` only replaces the neutral scaffold automatically. Once
  facts/entities exist it errors; `destroy solution` empties collections and does
  not re-wire them (see `gotchas.md`).
- `generate score` changes the score type on the existing solution and rewrites
  the API/DTF references.

Score types: `HardSoftScore` (default), `HardSoftDecimalScore`,
`HardMediumSoftScore`, `SoftScore`, `BendableScore<N, M>`.

## Facts and entities

```bash
solverforge generate fact <name> [--field <name:Type>]... [--force] [--pretend]
solverforge generate entity <name> [--planning-variable <field>] [--field <name:Type>]... [--force] [--pretend]
```

- Names are `snake_case`; structs are PascalCased, collections naively
  pluralized.
- `--field` is repeatable. `id` (and `name` for facts) are added automatically.
- `--planning-variable` on `generate entity` creates an initial
  `Option<usize>` planning variable with `allows_unassigned = true`; prefer the
  explicit `generate variable` command so you can name the source.
- `--force` overwrites an existing file, `--pretend` previews without writing.

## Variables

```bash
# scalar over a fact collection
solverforge generate variable <field> --entity <Entity> --kind scalar \
  --range <fact_plural> [--allows-unassigned]

# scalar over a half-open integer range
solverforge generate variable <field> --entity <Entity> --kind scalar \
  --countable-range <from..to>

# ordered list over a fact collection
solverforge generate variable <field> --entity <Entity> --kind list \
  --elements <fact_plural>

# stock CVRP route profile
solverforge generate variable <field> --entity <Entity> --kind list \
  --elements <fact_plural> --domain cvrp
```

Scalar fields are written as `pub <field>: Option<usize>` with:

```rust
#[planning_variable(value_range_provider = "<fact_plural>", allows_unassigned = true)]
```

or, for countable ranges:

```rust
#[planning_variable(countable_range = "<from..to>", allows_unassigned = false)]
```

List fields are written as `pub <field>: Vec<usize>` with
`#[planning_list_variable(element_collection = "<fact_plural>", ...)]`.

List metadata flags (all name user-owned Rust items, except `--domain`):
`--distance-meter`, `--intra-distance-meter`, `--route-hooks`, `--savings-hooks`,
`--savings-metric-class-fn`, `--element-owner-fn`,
`--construction-element-order-key`, `--precedence-duration-fn`,
`--precedence-successors-fn`, `--solution-trait`. The stock `cvrp` profile owns
its meters/hooks/metric/trait, so those cannot be combined with `--domain cvrp`.

Scalar hook metadata flags: `--candidate-values`, `--nearby-value-candidates`,
`--nearby-entity-candidates`, `--nearby-value-distance-meter`,
`--nearby-entity-distance-meter`, `--construction-entity-order-key`,
`--construction-value-order-key`. These only write metadata; you still implement
the hook functions in Rust.

## Constraints

```bash
solverforge generate constraint <id> --unary   [--hard|--soft]
solverforge generate constraint <id> --pair     [--hard|--soft]
solverforge generate constraint <id> --join     [--hard|--soft]
solverforge generate constraint <id> --balance
solverforge generate constraint <id> --reward
solverforge generate constraint <id> --runs     [--hard|--soft]
solverforge generate constraint <id> --presence [--hard|--soft]
solverforge generate constraint <id> --collect-vec [--hard|--soft]
solverforge generate constraint <id> --group-complement [--hard|--soft]
solverforge generate constraint <id> --projected-group [--hard|--soft]
```

Always pass exactly one pattern flag. Without one, the CLI starts an interactive
wizard that fails under a non-interactive agent. Pattern prerequisites are
validated: unary/pair/balance/reward need an entity; join/group-complement/
projected-group also need a fact; pair/join/balance/etc. need a scalar variable
on the first entity.

## Data

```bash
solverforge generate data [--size small|standard|large] [--mode sample|stub]
```

Regenerates `src/data/data_seed.rs` and updates the demo-size metadata. `sample`
produces index-based values; `stub` produces empty/`None`/`vec![]` shapes. This
file is compiler-owned; later domain-shape mutations regenerate it.

## Inspect, configure, validate

```bash
solverforge info                 # solution, entities, facts, constraints
solverforge check                # structure, managed blocks, model resources, solver.toml
solverforge routes               # web/api only (not cli or mcp)
solverforge connect              # mcp only: print MCP client configs
solverforge connect --write vscode   # mcp only: merge .vscode/mcp.json
solverforge config show
solverforge config set termination.seconds_spent_limit 60
solverforge config set candidate_trace.max_entries 100000
solverforge test                 # delegates to `cargo test`
solverforge completions <shell>
```

`check` never compiles or runs the model. Pair it with `cargo check` and a real
solve for web/API. The generated CLI only serializes demo data; extend it before
claiming that a terminal solve works. For mcp, verify the transport boots and
drive the tools with a real MCP client for a full claim.

## Run

```bash
# web / api
solverforge server                # release
solverforge server --debug        # faster first compile
solverforge server --port 8080

# mcp
cargo run --release               # stdio MCP server (default transport)
cargo run --release -- --http     # stateless Streamable HTTP at /mcp
cargo run --release -- --http --host 127.0.0.1
solverforge server                # equivalent; selects --http for mcp shells
solverforge connect               # register with Claude Code/Desktop, Cursor, VS Code

# cli
cargo run -- demo-data
cargo run -- demo-data --size large
```

`solverforge server` is rejected for cli-shell projects with a hint to use
`cargo run -- demo-data` (a serialization preview, not a solve), and boots the
HTTP MCP transport for mcp-shell projects.

## Destroy

```bash
solverforge destroy --yes constraint <id>
solverforge destroy --yes variable <field> --entity <Entity>
solverforge destroy --yes entity <name>
solverforge destroy --yes fact <name>
solverforge destroy --yes solution
solverforge destroy --yes scalar-group <name>
solverforge destroy --yes conflict-repair <id>
```

Destroy re-renders the affected managed blocks and projections. It blocks when a
dependent scalar group or conflict repair still references the resource; destroy
the dependents first. `--yes` skips the confirmation prompt (required in
non-interactive shells).
