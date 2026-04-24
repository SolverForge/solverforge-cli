# solverforge-cli

Default entry point for new SolverForge projects.

Use this CLI to scaffold, grow, and validate SolverForge applications. The CLI
is its own versioned product: `solverforge --version` reports the CLI package
version and the scaffold dependency targets separately.

New projects currently target these crate versions:

- `solverforge 0.9.0`
- `solverforge-ui 0.6.0`
- `solverforge-maps 2.1.3`

```bash
cargo install solverforge-cli
solverforge new my-scheduler
```

## Current Contract

Public scaffold path:

- `solverforge new <name>`

That command creates a neutral app shell. Users shape the app afterward through
facts, entities, variables, constraints, generated data, and
`solverforge.app.toml`. There are no public scaffold-family flags.

Planning variable kinds are canonical:

- `scalar` for single-value assignment variables backed by `--range <facts>`
- `list` for sequence variables backed by `--elements <facts>`

`standard` is only a demo dataset size label in `solverforge.app.toml`; it is
not a variable kind.

Basic domain flow:

```bash
solverforge new my-scheduler
cd my-scheduler
solverforge generate fact resource --field category:String --field load:i32
solverforge generate entity task --field label:String --field priority:i32
solverforge generate variable resource_idx --entity Task --kind scalar --range resources --allows-unassigned
solverforge generate data --size large
solverforge server
```

Generated projects use managed block markers as the canonical CLI edit points.
Domain exports, solution collections, entity variables, constraint modules, and
constraint calls must retain their `@solverforge:begin ...` /
`@solverforge:end ...` regions for later `generate` and `destroy` commands.
Project-local `.solverforge/templates/entity.rs.tmpl` and
`.solverforge/templates/solution.rs.tmpl` overrides are supported only when
they emit those same canonical managed blocks.

`solverforge generate data` owns the generated data pipeline. It keeps
`src/data/mod.rs` as the stable import wrapper, rewrites
`src/data/data_seed.rs` with deterministic sample builders, and persists
dataset size defaults in `solverforge.app.toml`. Generated values are
structurally useful rather than domain-specific fake business data.

The generated frontend is intentionally thin. It composes shipped
`solverforge-ui 0.6.0` primitives such as `SF.createBackend(...)`,
`SF.createSolver(...)`, and `SF.rail.createTimeline(...)` instead of vendoring
app-specific UI frameworks. Domain-specific examples belong in quickstarts, not
in the built-in scaffold catalog.

## Command Surface

Core commands:

- `solverforge new <name>` creates the neutral scaffold.
- `solverforge generate fact|entity|variable|constraint|solution|score|data`
  mutates the current project through the canonical generated surfaces.
- `solverforge destroy fact|entity|variable|constraint|solution` removes
  generated resources and rewrites the app spec/UI projection.
- `solverforge check`, `solverforge info`, and `solverforge routes` inspect the
  generated project.
- `solverforge config show|set` reads and writes `solver.toml`.
- `solverforge server` runs the generated app through Cargo.
- `solverforge test` delegates to `cargo test`.
- `solverforge completions <shell>` emits shell completions.

Persistent `.solverforgerc` preferences are intentionally narrow: `port`,
`no_color`, and `quiet`.

## Validation Flow

End-to-end validation is split into explicit phases so the real production
pipeline stays readable:

- `cargo test`
  Rust unit tests, scaffold contract tests, and generated-app runtime pipeline tests
- `make test-runtime`
  phase-marked runtime pipeline against ephemeral generated apps only
- `make test-e2e`
  Playwright browser tests against ephemeral generated apps only
- `make install-e2e`
  install Playwright Chromium locally before the first browser run
- `make test-full`
  full pipeline: Rust tests, runtime pipeline, then Playwright

The runtime and browser suites both scaffold fresh temp apps, mutate them
through the real CLI, boot the generated servers on random ports, and clean up
automatically. Failure artifacts are written under `target/test-artifacts/`.
By default, end-to-end validation keeps generated temp-app `Cargo.toml` files on
the published crate targets. To test a prerelease local ecosystem, set
`SF_USE_LOCAL_PATCHES=1`; the harness writes a temporary `.cargo/config.toml`
with explicit `[patch.crates-io]` entries for the sibling `solverforge-rs`,
`solverforge-ui`, and `solverforge-maps` checkouts. Generated manifests are not
rewritten.

Current scenario coverage:

- neutral shell: scaffold, boot, and verify the empty production shell
- mixed app: scaffold mixed shape, seed non-empty mixed demo data, and verify
  generated runtime/browser surface
- scalar-only app: seed non-empty data and run it through the real generated
  solver, including typed SSE, status, analysis, checkpointed Pause/Resume,
  user-facing Stop as runtime cancel, terminal-only Delete, and status/snapshot
  reconnect bootstrap

The mixed generated app is intentionally not the seeded solve scenario today.
The current SolverForge runtime does not yet support solving that mixed
scalar-plus-list combination end to end, so the test suite keeps that boundary
visible.

For solver and domain extension guidance after scaffolding, see the runtime docs
in [solverforge-rs](https://github.com/solverforge/solverforge-rs):
[Extend the solver](https://github.com/solverforge/solverforge-rs/blob/main/docs/extend-solver.md)
and [Extend the domain](https://github.com/solverforge/solverforge-rs/blob/main/docs/extend-domain.md).

## License

Licensed under the Apache License, Version 2.0. See [LICENSE](LICENSE).
