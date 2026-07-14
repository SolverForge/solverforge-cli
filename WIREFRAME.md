# solverforge-cli Wireframe

## Product Surface

`solverforge-cli` owns one public scaffold entry point:

- `solverforge new <name>`

The generated project is a neutral shell. The default shell is `web`; users can
choose `--shell api` for an HTTP API without frontend assets or `--shell cli`
for a Clap command-line app without Axum. Users add facts, entities, variables,
solution/score metadata, constraints, solver config, and generated data after
scaffolding. Shell choice is a delivery surface, not a model family. The
current shell set is exactly `web`, `api`, and `cli`; Tauri is deferred and has
no public scaffold selector in this release line.

Current CLI package version: `2.2.3`.

Required Rust version: `1.95` or later.

Current generated projects target:

- `solverforge 0.19.0`
- `solverforge-ui 0.7.0` for the web shell
- `solverforge-maps 2.1.4` for the web shell

The CLI version is separate from those targets and must remain visible in
version output and generated README content.

## Canonical Modeling Terms

Planning variable kinds are:

- `scalar`: one assigned value from either a fact collection, configured with
  `--range <FACT_COLLECTION>`, or a non-negative half-open integer domain,
  configured with `--countable-range <FROM..TO>`
- `list`: an ordered sequence of values from a fact collection, configured with
  `--elements <FACT_COLLECTION>`

Scalar variable generation accepts optional hook-name flags for current
SolverForge scalar metadata:

- `--candidate-values`
- `--nearby-value-candidates`
- `--nearby-entity-candidates`
- `--nearby-value-distance-meter`
- `--nearby-entity-distance-meter`
- `--construction-entity-order-key`
- `--construction-value-order-key`

These flags are metadata-only. The CLI emits the planning-variable attribute,
syncs the values through `solverforge.app.toml`, writes them to
`static/generated/ui-model.json` for web-shell projects, and leaves the Rust hook
function bodies under domain ownership.

Countable ranges are validated before mutation, emitted as
`countable_range = "from..to"`, synchronized through the app spec, and projected
as structured `from`/`to` bounds for numeric-lane rendering in the web shell.
Fact ranges and countable ranges are mutually exclusive.

List variable generation accepts the complete current list metadata surface:

- `--domain cvrp`
- `--distance-meter` and `--intra-distance-meter`
- `--route-hooks`, `--savings-hooks`, and `--savings-metric-class-fn`
- `--element-owner-fn`
- `--construction-element-order-key`
- `--precedence-duration-fn` and `--precedence-successors-fn`
- `--solution-trait`

The values are emitted into `#[planning_list_variable(...)]`, synchronized to
the app spec, and projected to the web UI model. The `cvrp` profile supplies
its stock distance meters, route/savings hooks, metric class, and solution
trait as one coherent runtime profile; explicit overrides of those fields are
rejected when the profile is selected.

Scalar groups and conflict repairs are exact-ID resources. Scalar groups are
named by the user-provided group ID; conflict repairs are named by the
snake_case constraint ID they repair. Assignment-backed and candidate-backed
scalar groups participate in grouped construction and grouped local search
unless `--skip-solver-config` is used.

`standard` is not a planning variable kind. It is only the default demo data
size name alongside `small` and `large`.

## Generated Project Shape

The web shell generates:

- `Cargo.toml` with Rust `1.95`, `solverforge`, `solverforge-ui`, and
  `solverforge-maps` dependencies, plus explicit current web, serialization,
  and utility dependency baselines. Generated projects do not depend on
  SolverForge runtime subcrates directly.
- `solver.toml` as the search strategy and termination configuration layer
- `solverforge.app.toml` as the scaffolded app/domain contract
- `src/domain/` with a neutral `Plan` solution and managed domain exports
- `src/constraints/` with an empty managed constraint set
- `src/api/` exposing the retained `/jobs` REST/SSE contract expected by
  `solverforge-ui`, plus qualified-run and detailed telemetry routes
- `src/solver/` tracking best solution, complete compact status telemetry,
  lifecycle events, snapshot-bound analysis, and separately retained bounded
  candidate-pull detail
- `src/data/mod.rs` as the stable data import wrapper
- `src/data/data_seed.rs` as compiler-owned generated sample data
- `static/index.html` loading `/sf/sf.css` and `/sf/sf.js`
- `static/app.js` containing app composition and projection-specific rendering
- `static/generated/ui-model.json` as the compiler-owned UI model projection
- `static/sf-config.json` as the preserved customization seam

The API shell uses the same domain, constraints, solver, data, DTO, route, and
SSE surfaces, but omits `static/`, `solverforge-ui`, `solverforge-maps`, and
static file serving.

The CLI shell keeps the domain, constraints, solver, data, and shared DTO
surface, but omits Axum routes, SSE route files, frontend assets,
`solverforge-ui`, and `solverforge-maps`. Its binary is a Clap command-line
entry point.

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
- `solver.toml`: one generated model-resource region marked with
  `# @solverforge:begin solver-config` and
  `# @solverforge:end solver-config`; each generated phase has an exact-ID
  `# @solverforge:owner <kind> <exact-id> <role>` marker

Commands that add or remove generated resources should fail clearly when those
current markers are missing or duplicated.
Project-local entity and solution override templates are only valid when they
emit the same managed block set; free-form overrides are intentionally rejected.

## App Spec Projection

`solverforge.app.toml` is the project model used to regenerate frontend and data
projections. It tracks:

- app metadata, including fixed `starter = "neutral-shell"` metadata, selected
  `shell`, and CLI version
- runtime target metadata and `runtime_source`
- web-shell `ui_source`; API and CLI specs must keep `ui_source` absent,
  including after follow-up domain mutations
- demo data sizes
- solution name and score type
- fact collections
- planning entity collections
- `scalar` and `list` variable declarations
- scalar fact/countable value-source metadata, scalar hooks, and list
  domain/hook metadata
- constraint modules
- scalar groups and conflict repairs by exact ID

For the web shell, `static/generated/ui-model.json` is derived from the app
spec and current domain parsing. API and CLI shells do not create frontend
projection files. Unknown variable kinds are errors, not aliases.

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
- status, snapshot, and SSE payloads expose every compact SolverForge telemetry
  aggregate and phase/selector/move/applied-move breakdown
- `GET /jobs/{id}/telemetry` exposes the atomically paired retained status and
  opt-in bounded candidate-pull trace; candidate pulls never ride normal SSE,
  status, or snapshot traffic
- `POST /jobs/qualified` accepts externally attested schema, instance,
  initial-state, core-tree, and build SHA-256 digests and starts the same
  retained lifecycle with qualified trace provenance
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
`solverforge config show|set` edits non-phase settings in that file through
dotted TOML paths such as `termination.seconds_spent_limit`. Ordered `phases`
are the solver execution graph and are not edited through `config set`.
Model-resource entries in `solver.toml` are graph references, not flat text
matches. Validation must inspect construction phases, top-level
`move_selector`, `neighborhoods`, nested selector children, and partition child
phases. Destroy re-renders the CLI-managed solver config region from
`solverforge.app.toml`; nested or user-authored refs block the destroy before
any project file is written. `solverforge-cli` must not link to
`solverforge-config` for this; the graph scan is CLI-private TOML-structure
validation.

Bounded candidate diagnostics are enabled through
`candidate_trace.max_entries`, including
`solverforge config set candidate_trace.max_entries <N>`. The CLI rejects a
missing, zero, non-integer, or out-of-range capacity. The neutral template
documents this setting but leaves it disabled by default because trace detail
can be large.

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
- mixed app: seeded scalar-plus-list solve through the runtime and browser,
  including required scalar assignment, complete list placement, cancel, and
  terminal cleanup
- scalar-only app: seeded solve flow through typed SSE, status, analysis, and
  delete flow, including full aggregate telemetry, bounded candidate detail,
  and qualified trace provenance

SolverForge `0.19.0` uses list variables for ordered sequences and routes. Both
generated-app end-to-end suites start and observe the real mixed scalar/list
path.

## Non-Goals

Do not reintroduce:

- public scaffold-family flags
- public Tauri shell selection before a real Tauri scaffold exists
- hidden `standard` variable-kind aliases
- scalar predecessor topology; ordered sequences belong to list variables
- hidden console/scaffold aliases
- compatibility migrations for unmanaged legacy generated files
- raw score-only SSE payloads
- separate starter-specific solve/render lifecycles
