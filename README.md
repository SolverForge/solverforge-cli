# solverforge-cli

Default entry point for new SolverForge projects.

Use this CLI to scaffold, grow, and manage SolverForge applications. Generated
projects currently target released `solverforge 0.7.1` and `solverforge-ui 0.4.0`
crate dependencies, and that target policy is part of the product surface rather
than an invisible implementation detail.

This package has its own version. That version is not the same thing as the
SolverForge runtime version or source that newly scaffolded projects target.
The CLI should say that runtime/UI target explicitly.

```bash
cargo install solverforge-cli
solverforge new my-scheduler
```

The CLI lives outside `solverforge-rs` and generates problem-type projects that
depend on separate SolverForge runtime and UI packages. Use `solverforge --version`
to see the CLI version and the runtime/UI targets currently baked into new projects.

Built-in starter path:

- `solverforge new <name>`

That scaffold is a neutral app shell. Users choose standard variables, list
variables, or any mixed combination later through domain generation and the app
spec, instead of picking a starter family up front.

Basic domain flow:

```bash
solverforge new my-scheduler
cd my-scheduler
solverforge generate fact resource --field category:String --field load:i32
solverforge generate entity task --field label:String --field priority:i32
solverforge generate variable resource_idx --entity Task --kind standard --range resources --allows-unassigned
solverforge generate data --size large
solverforge server
```

`solverforge generate data` rewrites the compiler-owned sample builders in
`src/generated/data_seed.rs`. The stable wrapper in `src/data/mod.rs` delegates
to that generated seed file by default, so the command can keep regenerating
sample data without clobbering user-owned entrypoints. Dataset size defaults are
persisted in `solverforge.app.toml`. Generated values are intentionally generic
and deterministic: the CLI optimizes for structurally useful optimization
datasets, not domain-specific fake business data.

The generated frontend is intentionally thin and composes shipped `solverforge-ui` primitives instead of vendoring template-specific web assets. Domain-specific examples such as employee scheduling and vehicle routing belong in quickstarts, not in the CLI's built-in scaffold catalog.

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

Current scenario coverage:

- neutral shell: scaffold, boot, and verify the empty production shell
- mixed app: scaffold mixed shape, seed non-empty mixed demo data, and verify
  generated runtime/browser surface
- standard-only app: seed non-empty data and run it through the real generated
  solver, including typed SSE, status, analysis, and delete flow

The mixed generated app is intentionally not the seeded solve scenario today,
because the current SolverForge runtime does not yet support solving that mixed
standard-plus-list combination end to end. The test suite reflects the actual
runtime capability boundary instead of hiding it.

For solver and domain extension guidance after scaffolding, see the runtime docs
in [solverforge-rs](https://github.com/solverforge/solverforge-rs):
[Extend the solver](https://github.com/solverforge/solverforge-rs/blob/main/docs/extend-solver.md)
and [Extend the domain](https://github.com/solverforge/solverforge-rs/blob/main/docs/extend-domain.md).

## License

Licensed under the Apache License, Version 2.0. See [LICENSE](LICENSE).
