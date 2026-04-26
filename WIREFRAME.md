# solverforge-cli Wireframe

## Product Surface

`solverforge-cli` owns one public scaffold entry point:

- `solverforge new <name>`

The generated project is a neutral shell. Users add facts, entities, variables,
solution/score metadata, constraints, solver config, and generated data after
scaffolding. The CLI does not expose scaffold-family flags.

Current CLI package version: `2.0.1`.

Required Rust version: `1.95` or later.

Current generated projects target:

- `solverforge 0.9.1`
- `solverforge-ui 0.6.3`
- `solverforge-maps 2.1.3`

The CLI version is separate from those targets and must remain visible in
version output and generated README content.

## Canonical Modeling Terms

Planning variable kinds are:

- `scalar`: one assigned value from a fact collection, configured with
  `--range <FACT_COLLECTION>`
- `list`: an ordered sequence of values from a fact collection, configured with
  `--elements <FACT_COLLECTION>`

`standard` is not a planning variable kind. It is only the default demo data
size name alongside `small` and `large`.

## Generated Project Shape

The neutral scaffold generates:

- `Cargo.toml` with Rust `1.95`, `solverforge`, `solverforge-ui`, and
  `solverforge-maps` dependencies, plus explicit current web, serialization,
  and utility dependency baselines
- `solver.toml` as the search strategy and termination configuration layer
- `solverforge.app.toml` as the scaffolded app/domain contract
- `src/domain/` with a neutral `Plan` solution and managed domain exports
- `src/constraints/` with an empty managed constraint set
- `src/api/` exposing the retained `/jobs` REST/SSE contract expected by
  `solverforge-ui`
- `src/solver/` tracking best solution, status telemetry, lifecycle events, and
  snapshot-bound analysis
- `src/data/mod.rs` as the stable data import wrapper
- `src/data/data_seed.rs` as compiler-owned generated sample data
- `static/index.html` loading `/sf/sf.css` and `/sf/sf.js`
- `static/app.js` containing app composition and projection-specific rendering
- `static/generated/ui-model.json` as the compiler-owned UI model projection
- `static/sf-config.json` as the preserved customization seam

`templates/scalar/generic` is the embedded template used by `solverforge new`.
`templates/list/generic` is not a public scaffold selector.

## Managed Blocks

Generated mutation is canonical-only. The CLI rewrites explicit managed block
regions and does not infer old unmanaged shapes.

Required managed surfaces:

- `src/domain/mod.rs`: `solverforge::planning_model!` manifest with
  `root = "src/domain"` and the `domain-exports` block
- solution files: `solution-imports`, `solution-collections`,
  `solution-constructor-params`, and `solution-constructor-init`
- entity files: `entity-variables` and `entity-variable-init`
- `src/constraints/mod.rs`: `constraint-modules` and `constraint-calls`

Commands that add or remove generated resources should fail clearly when those
current markers are missing or duplicated.
Project-local entity and solution override templates are only valid when they
emit the same managed block set; free-form overrides are intentionally rejected.

## App Spec Projection

`solverforge.app.toml` is the project model used to regenerate frontend and data
projections. It tracks:

- app metadata, including fixed `starter = "neutral-shell"` metadata and CLI
  version
- runtime target metadata, `runtime_source`, and `ui_source`
- demo data sizes
- solution name and score type
- fact collections
- planning entity collections
- `scalar` and `list` variable declarations
- constraint modules

`static/generated/ui-model.json` is derived from the app spec and current domain
parsing. Unknown variable kinds are errors, not aliases.

## Frontend Rule

For any generated frontend feature, ask first: does `solverforge-ui` already
provide it?

If yes, the scaffold should use the shipped `solverforge-ui` surface instead of
re-implementing it. Generated apps should rely on:

- `solverforge_ui::routes()` for `/sf/*` assets
- `SF.createBackend()` and `SF.createSolver()` for retained solver lifecycle
- `SF.createHeader()`, `SF.createStatusBar()`, `SF.createModal()`,
  `SF.createApiGuide()`, and `SF.createFooter()`
- `SF.createTable()` for tabular projections
- `SF.rail.createTimeline(...)` for variable-driven timelines

The scaffold may own thin composition code for domain projections that are not
available as turnkey `solverforge-ui` views.

## Runtime Contract

Generated apps should behave like production references:

- status endpoints expose `currentScore`, `bestScore`, solver status, and latest
  snapshot revision
- SSE messages carry typed lifecycle metadata including `eventType`,
  `eventSequence`, `lifecycleState`, and `snapshotRevision`
- retained lifecycle events include `progress`, `best_solution`,
  `pause_requested`, `paused`, `resumed`, `completed`, `cancelled`, and `failed`
- progress-only events update status, not the rendered board
- best-solution snapshots remain separate from live progress telemetry
- `/jobs/{id}/snapshot` stays aligned with snapshot-bound analysis
- reconnect bootstrap comes from current `SolverManager` status plus the latest
  retained snapshot, not cached last SSE event text
- Pause resumes from a retained checkpoint, Stop calls runtime cancel, and
  Delete is available only for terminal retained jobs before the next Solve

## Data Generation

`solverforge generate data` owns generated sample data:

- preserves `src/data/mod.rs` as the stable wrapper
- rewrites `src/data/data_seed.rs`
- persists the selected demo size in `solverforge.app.toml`
- supports `sample` mode by default and `stub` mode for shape-only data
- serves the generated data catalog from `/demo-data` and selected demo data
  from `/demo-data/{id}`

Generated data should be deterministic and structurally useful for optimization
testing. It should not pretend to be domain-specific business data.

## Configuration

`solver.toml` owns solver behavior, including phases and termination settings.
`solverforge config show|set` edits that file through dotted TOML paths such as
`termination.seconds_spent_limit`.

`.solverforgerc` is loaded from the project root first, then from
`~/.solverforgerc`. It is intentionally narrow and only carries local CLI
preferences:

- `port`
- `no_color`
- `quiet`

## Validation Shape

Validation should use ephemeral generated apps rather than checked-in sample
projects:

- scaffold contract tests verify dependency targets, generated README metadata,
  managed block markers, generated app specs, and `cargo check`
- runtime pipeline tests run phase-marked generated-app scenarios
- Playwright tests boot generated apps on random ports and verify browser-visible
  lifecycle behavior

Current scenario policy:

- neutral shell: bootable empty app
- mixed app: generated scalar-plus-list shape and browser/runtime surface
- scalar-only app: seeded solve flow through typed SSE, status, analysis, and
  delete flow

Do not claim mixed seeded solving until the runtime supports that combination.

## Non-Goals

Do not reintroduce:

- public scaffold-family flags
- hidden `standard` variable-kind aliases
- hidden console/scaffold aliases
- compatibility migrations for unmanaged legacy generated files
- raw score-only SSE payloads
- separate starter-specific solve/render lifecycles
