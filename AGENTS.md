# Repository Guidelines

## Project Structure & Module Organization
`src/main.rs` defines the CLI entrypoint and Clap command tree. Command implementations live in `src/commands/`; larger generators use submodules such as `src/commands/generate_constraint/` and `src/commands/generate_domain/`. Shared support code sits in files like `src/app_spec.rs`, `src/error.rs`, `src/managed_block.rs`, `src/output.rs`, `src/rc.rs`, `src/scaffold_target.rs`, and `src/template.rs`. Integration tests live in `tests/`, and scaffold/template assets live in `templates/`.

## Current Product Direction
`solverforge-cli` is the default entry point for new SolverForge applications. Treat the CLI as its own versioned product, distinct from the runtime crates and UI assets that generated projects target.

Current scaffold policy:
- current CLI package version is `2.0.1`
- minimum supported Rust version is `1.95`, matching the current SolverForge runtime crates
- generated projects currently target `solverforge 0.9.1`, `solverforge-ui 0.6.3`, and `solverforge-maps 2.1.3` as their crate dependency versions
- `solverforge new <name>` is the only public scaffold path and produces a neutral shell
- users shape the app afterward through facts, entities, solution/score metadata, variables, constraints, and generated data
- generated docs and CLI version output must distinguish CLI version from scaffold runtime/UI target
- `scalar` and `list` are the only planning variable kinds accepted by the public CLI and app-spec projection
- `standard` is a demo size label only; do not reintroduce it as a variable kind or scaffold family
- `templates/scalar/generic` is the embedded neutral scaffold used by `solverforge new`; `templates/list/generic` is not a public `new` selector
- generated `Cargo.toml` files must carry `rust-version = "1.95"` plus the current explicit web, serialization, and utility dependency baselines from the template files

When changing templates or scaffold behavior, follow the current repo reality over older starter-template assumptions. Do not add legacy aliases, compatibility shims, migration fallbacks, or automatic rewrites for unmanaged pre-refactor file shapes unless that is explicitly requested.

## Build, Test, and Development Commands
- `cargo build`: compile the `solverforge` binary.
- `cargo run -- --help`: run the CLI locally and inspect commands.
- `cargo test`: run Rust unit tests, scaffold contract tests, and generated-app runtime pipeline tests.
- `cargo test --test runtime_pipeline_test -- --nocapture --test-threads=1`: run the phase-marked runtime pipeline suite directly.
- `make test-runtime`: run only the generated-app runtime pipeline tests.
- `make install-e2e`: install Playwright Chromium locally.
- `make test-e2e`: run Playwright browser tests against ephemeral generated apps.
- `make test-full`: run the Rust suite, runtime pipeline suite, and Playwright suite in order.
- `cargo fmt --all`: apply Rust formatting.
- `cargo clippy --all-targets -- -D warnings`: enforce lint-clean code.
- `pre-commit run --all-files`: run the repository hooks, including YAML checks, `gitleaks`, `fmt`, and `clippy`.

## Coding Style & Naming Conventions
Use standard Rust style with 4-space indentation and `rustfmt` output as the source of truth. Prefer `snake_case` for modules, files, functions, and test names; use `PascalCase` for types and enums. Keep CLI flags, generated file names, and module names descriptive and consistent with existing commands such as `generate_constraint` and `sf_config`.

## Testing Guidelines
Add narrow unit tests close to the code in sibling test files or `#[cfg(test)]` child modules when validating parsing, rewriting, or template helpers. Put end-to-end CLI behavior in `tests/`, following existing names like `*_test.rs` and `test_*` functions. When changing scaffolding or generated code, cover both file creation and key output content assertions.

Prefer a small number of readable, high-signal end-to-end phases over a large number of narrow tests. The main generated-app suites are:

- `tests/scaffold_test.rs`: scaffold contract and generated `cargo check`
- `tests/runtime_pipeline_test.rs`: phase-marked generated-app runtime pipelines
- `tests/e2e/*.spec.js`: Playwright browser pipelines against ephemeral generated apps

Both runtime and browser suites must:

- scaffold fresh temp apps through the real CLI
- mutate them through the real CLI
- boot generated servers on random ports
- preserve failure artifacts under `target/test-artifacts/`
- keep phase boundaries explicit in code and logs

Current end-to-end scenario policy:

- neutral shell is covered as a bootable empty app
- mixed is covered as generated shape plus runtime/browser surface
- seeded solver execution is covered in the scalar-only scenario

Do not claim mixed seeded solving is supported until the underlying runtime actually supports that combination.

For scaffold changes, prefer assertions that check the generated contract directly:
- dependency wiring for `solverforge`, `solverforge-ui`, and `solverforge-maps`
- CLI version vs runtime target messaging
- generated README version/runtime source disclosure
- typed solver SSE payload shape and typed frontend hooks
- `solverforge generate data` ownership boundaries:
  `src/data/mod.rs` is the stable wrapper and `src/data/data_seed.rs` is compiler-owned generated sample data
- managed block ownership boundaries:
  domain exports, solution collections, entity variables, constraint modules, and constraint calls require their `@solverforge:begin ...` / `@solverforge:end ...` markers
- `solverforge.app.toml` projection:
  facts, entities, variables, constraints, demo sizes, solution/score metadata, runtime target metadata, and `static/generated/ui-model.json`
- scaffolded `cargo check` against the published crate targets by default; prerelease sibling-checkout validation must be explicit via `SF_USE_LOCAL_PATCHES=1`, which writes a temporary `.cargo/config.toml` patch file without rewriting generated `Cargo.toml`

## Commit & Pull Request Guidelines
Follow the commit style already used in history: `fix: ...`, `refactor: ...`, `style: ...`, `chore: ...`. Keep commits scoped to one behavior change. PRs should explain user-visible CLI impact, list verification commands run, and link the relevant issue. Include concrete examples when flags, generated files, or template output change.

## Security & Configuration Tips
Do not commit generated projects, secrets, or credentials. The pre-commit config runs `gitleaks`; keep it enabled. When tests or generated scaffolds touch published SolverForge crate versions, verify `Cargo.toml` template changes and corresponding integration tests together.

## Scaffold Contract Notes
Generated apps should behave like production references, not toy demos.

Keep these rules aligned across the single neutral scaffold and all generated
domain shapes that users create afterward:
- backend services track best solution separately from live status telemetry
- status endpoints return `currentScore`, `bestScore`, solver status, and latest snapshot revision
- SSE payloads carry typed lifecycle metadata including `eventType`, `eventSequence`, `lifecycleState`, and `snapshotRevision`
- retained lifecycle events include `progress`, `best_solution`, `pause_requested`, `paused`, `resumed`, `completed`, `cancelled`, and `failed`
- frontends use lifecycle-aware hooks including `onProgress(meta)`, `onSolution(snapshot, meta)`, `onPaused(snapshot, meta)`, `onResumed(meta)`, `onCancelled(snapshot, meta)`, `onComplete(snapshot, meta)`, and `onFailure(message, meta, snapshot, analysis)`
- snapshot-bound analysis and retained `/jobs/{id}/snapshot` flows stay aligned with the shared UI backend contract
- reconnect bootstrap is derived from `SolverManager` status plus the latest retained snapshot revision; do not cache or replay last SSE text locally
- Pause resumes from the runtime-retained checkpoint, Stop maps to `/jobs/{id}/cancel`, and Delete is terminal cleanup only
- progress-only events update status, not the rendered board
- generated demo data flows expose `/demo-data` as the catalog and `/demo-data/{id}` for selected data; frontends derive the default ID from the catalog and must not hard-code `/demo-data/STANDARD`
- generated UI composition should use shipped `solverforge-ui` primitives before adding template-owned JavaScript
- `src/data/data_seed.rs` and `static/generated/ui-model.json` are compiler-owned; user-facing seams are the `solverforge generate data` command, stable `src/data/mod.rs` wrapper, and `static/sf-config.json`
- generated domain and constraint mutation is canonical-only: current managed block shapes are required, not inferred from old layouts

Do not reintroduce:
- raw top-level score-only progress payloads
- starter-specific solve/render lifecycles
- docs that blur CLI version with runtime/UI target
- `standard` as a planning variable kind
- hidden scaffold/template aliases
- fallback migration code for legacy generated files

Test harness notes:

- ephemeral generated apps should be the default validation path; do not rely on manually scaffolded directories for routine testing
- runtime/browser failures should leave actionable logs and artifacts under `target/test-artifacts/`
- phase output should remain human-readable and ordered, even when tests grow
