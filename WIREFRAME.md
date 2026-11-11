# solverforge-cli Wireframe

## Product Surface

`solverforge-cli` owns one public scaffold entry point:

- `solverforge new <name>`

The generated project is a neutral shell. The default shell is `web`; users can
choose `--shell api` for an HTTP API without frontend assets, `--shell cli` for
a Clap command-line app without Axum, or `--shell mcp` for an MCP server that
any MCP-capable agent harness can drive. Users add facts, entities, variables,
solution/score metadata, constraints, solver config, and generated data after
scaffolding. Shell choice is a delivery surface, not a model family. The
current shell set is exactly `web`, `api`, `cli`, and `mcp`; Tauri is deferred
and has no public scaffold selector in this release line.

Current CLI package version: `4.0.3`.

Required Rust version: `1.95` or later.

Current generated projects target:

- `solverforge 0.26.1`
- `solverforge-ui 0.9.1` for the web shell
- `solverforge-maps 2.3.3` for the web shell
- `rmcp 3.9.0` for the MCP shell

The CLI version is separate from those targets and must remain visible in
version output and generated README content.

## Agent Skill Surface

`skills/` bundles two portable, harness-agnostic Agent Skills: `solverforge-modeling/`
(turn a planning problem into an app with this CLI) and `solverforge-ui/` (extend
a generated web shell with the shipped components). They are in-repo product
artifacts, not generated code: a `SKILL.md` entry point with `references/` and any
`scripts/`, discovered by opencode, Claude Code, Codex, and the cross-harness
Agent Skills standard.

`scripts/install-skill` is agent-centric: the user names the harnesses they use
and the installer resolves each harness's own skills directory. It never installs
into an unrequested directory, never assumes `~/.agents`, and refuses duplicate
discovery. The default is one direct copy per selected harness; `--layout covering`
computes a duplicate-free shared placement for overlapping selections, and
`--link` symlinks instead of copying. The `{opencode, claude, codex}` combination
has no duplicate-free placement and is reported rather than silently duplicated;
`--layout per-harness --force` installs all three copies while accepting the
duplicate discovery. Ownership is recorded by a `.solverforge-skill` marker
written into each installed copy (and a sidecar receipt for symlinks whose
recorded target must still match the link), so the installer refuses to
overwrite or remove an entry it did not install. `make install-skill
ARGS='--agent <harness>'` forwards arguments to the script.

The skills bundle `scripts/solve-smoke-test.sh`, a per-shell development aid: it
builds a generated app and drives it (web/API require a clean `COMPLETED` solve
with published scores; MCP proves transport health and a panic-free boot only —
tool calls require a real MCP client, as driven by `runtime_mcp_pipeline_test`;
CLI validates `demo-data` serialization). It complements, and does not replace,
the repository's generated-app suites.

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

The MCP shell keeps the domain, constraints, solver, data, and shared DTO
surface, but omits REST route files, SSE files, frontend assets,
`solverforge-ui`, and `solverforge-maps`. It adds `src/mcp/`, an MCP server
built on `rmcp 3.9.0` that projects the retained solver job lifecycle onto
twelve annotated tools with typed schemas. `solve` returns an MCP task handle
with the retained `jobId` in result metadata to task-capable clients (MCP
2026-07-28) and an immediate job summary to other clients. Each task has no
TTL, so expiry cannot orphan an active solve; task records remain for the
server process lifetime. Failed or cancelled solves terminate with the
corresponding MCP task state. stdio is the default transport;
`--http` serves stateless Streamable HTTP at `/mcp` on the loopback interface
unless `--host` selects a concrete IP. Wildcard binds are rejected so rmcp Host
validation remains active. One solver service and one task store are shared
across every stateless HTTP negotiation, and the transport has no authentication
or per-caller isolation: a non-loopback `--host` exposes the full job lifecycle
(start, inspect, cancel, delete) to anyone who can reach the port, and Host
validation only rejects DNS-rebinding headers. Documented as a known limitation;
keep the bind on loopback unless the network is trusted. The MCP shell enables
only the `serde` feature of `solverforge`: `console` writes the runtime banner
to stdout, which is the stdio MCP transport channel, and `verbose-logging`
enables `console`, so both stay off; diagnostics go to stderr. The web, API,
and CLI shells enable `serde`, `console`, and `verbose-logging`. Every shell
declares `schemars` as an optional dependency behind a `schema` feature, and
only the MCP shell enables it.

`solverforge connect` prints MCP client configuration derived from the project
(app name, resolved binary profile, and server port) and writes only in-project
configuration: `--write vscode` merges the project entry into
`.vscode/mcp.json` while preserving other servers and refusing to rewrite a
file that is not valid JSON. Global client configuration files are printed with
their exact path rather than modified.

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
- `src/constraints/mod.rs`: the `constraint-modules` block, `mod assemble;`, and
  `pub use assemble::create_constraints;`, with no code
- `src/constraints/assemble.rs`: `create_constraints` and the
  `constraint-calls` block, which calls each module as
  `super::<id>::constraint()`
- `solver.toml`: one generated model-resource region marked with
  `# @solverforge:begin solver-config` and
  `# @solverforge:end solver-config`; each generated phase has an exact-ID
  `# @solverforge:owner <kind> <exact-id> <role>` marker

Commands that add or remove generated resources should fail clearly when those
current markers are missing or duplicated.

Every `mod.rs` and `lib.rs` under `src/` holds only module declarations, `use`
items, and (in `src/domain/mod.rs`) the `planning_model!` manifest.
`solverforge check` rejects any function, type, impl, constant, or inline module
body in those files.

`constraint-modules` holds `mod` lines in rustfmt's sorted order and matches
them to calls by name; `constraint-calls` keeps insertion order because it is
the constraint set's evaluation order. `generate`, `destroy`, and `new` pipe
each Rust file they create or change through the toolchain's `rustfmt`
(honouring `$RUSTFMT`, the manifest edition, and `rustfmt.toml`), so generated
projects pass `cargo fmt --check` and `cargo fmt` leaves the managed blocks
valid. rustfmt is optional: a missing rustfmt or an unparseable file leaves
the file as generated, and a rustfmt result that would move a standalone
`// @solverforge:` marker line is discarded.

`destroy fact` and `destroy entity` refuse before writing anything while other
source under `src/` still names the type or streams its collection (comments
and string literals are ignored), and name the dependent files.
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
spec and current domain parsing. API, CLI, and MCP shells do not create
frontend projection files. Unknown variable kinds are errors, not aliases.

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
  aggregate and phase/selector/move/applied-move breakdown; the six
  `conflictRepair*` counters are run-level only, because SolverForge `0.26.1`
  `SelectorTelemetry` does not carry them
- constraint analysis carries `name`, `weightMultiplier` (the bound constraint
  weight), `score`, and `matchCount` per constraint
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
testing. It should not pretend to be domain-specific business data. Field
types without a `Default` get a concrete value (`chrono::Weekday::Mon`,
`chrono::Utc`).

## Domain Fields

`generate fact|entity --field name:Type` rejects a field that repeats a
generated field (`id` and `name` on facts, `id` and the `--planning-variable`
field on entities) or another `--field`, before any file is written. The
generators import `std::collections` and chrono field types, add
`chrono = { version = "0.4.45", features = ["serde"] }` to the app's
dependencies when a field first needs it, and import any other bare type
with `use super::<Type>`. A type that is not defined under `src/domain` is a
warning, not an error, so the user can define it afterward.

## Score Types

`generate score` and `generate solution --score` accept `HardSoftScore`,
`HardSoftDecimalScore`, `HardMediumSoftScore`, and `SoftScore`.
`BendableScore` is rejected: `#[planning_solution]` requires a
`ParseableScore`, and SolverForge `0.26.1` does not implement it for
`BendableScore`. Score rewrites change only the bare type name inside `use`
items.

## Constraint Skeletons

`generate constraint` writes a stream that compiles and placeholder
functions that panic with the function name and the
`src/constraints/<name>.rs` path. The common shapes are:

- `--unary`: `for_each` + `filter(unary_condition)` + `penalize`, for both
  `--hard` and `--soft`
- `--reward`: `for_each` + `filter(reward_condition)` + `reward`
- `--pair`: `for_each` + `filter` (variable assigned) + `join(joiner::equal(key))`
  self-join, which yields each unordered pair once, + `filter(pair_condition)`
  + `penalize`; SolverForge `0.26.1` has no `for_each_unique_pair`

Weight helpers return the unit penalty or reward. A solve that reaches a
placeholder fails: the job ends `FAILED` and the panic names the file and
function to edit. `solverforge check` and `cargo build` do not catch this.

## Configuration

`solver.toml` owns solver behavior, including phases and termination settings.
The neutral scaffold declares no `[[phases]]`, so a fresh app runs the
runtime's model-aware default pipeline, which also completes a model with no
planning variables; declaring any phase replaces that whole pipeline.
`generate scalar-group` refuses a user-authored scalar construction phase that
reaches the group's assignment-owned target (untargeted, or matching its
`entity_class`/`variable_name`), because the runtime rejects that config and
every solve would fail; the error names the phase path and how to scope it.
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

Parallel local-search evaluation is enabled through `move_thread_count`,
including `solverforge config set move_thread_count.count <N>`. The CLI accepts
exactly the runtime's forms (`"auto"`, `"none"`, `{ count = N }` with a
positive integer) because the generated app parses its embedded
`solver.toml` on the first solve, where a malformed value would fail the job
instead of the build. The neutral template documents the setting, the Rayon and
32-evaluator caps, and the shared per-process pool, and leaves the runtime
default `auto` (up to 4 evaluators, sequential or parallel per step by measured
cost) in place.

`.solverforgerc` is loaded from the project root first, then from
`~/.solverforgerc`. It is intentionally narrow and only carries local CLI
preferences:

- `port`
- `no_color`
- `quiet`

`solverforge server` and `solverforge connect` resolve the port as
`--port`, then the `PORT` environment variable, then the `.solverforgerc`
`port`, then `7860`. A non-numeric `PORT` is an error.

## Inspection And Tests

- `solverforge check` is structural: it validates managed blocks, the domain
  model, model resources, and `solver.toml`, and its output states that the
  project was not compiled
- `solverforge routes` lists every method of a chained router and `.route(`
  calls that span several lines, and ignores `//` comments
- `solverforge test` passes arguments before `--` to `cargo test` and
  arguments after `--` to the test binaries

## Validation Shape

Validation should use ephemeral generated apps rather than checked-in sample
projects:

- scaffold contract tests verify dependency targets, generated README metadata,
  managed block markers, generated app specs, and `cargo check`
- runtime pipeline tests run phase-marked generated-app scenarios
- Playwright tests boot generated apps on random ports and verify browser-visible
  lifecycle behavior

Current scenario policy:

- neutral shell: bootable empty app whose demo data solves to `COMPLETED`
  with the scaffold `solver.toml`
- mixed app: seeded scalar-plus-list solve through the runtime and browser,
  including required scalar assignment, complete list placement, cancel, and
  terminal cleanup; the runtime pipeline also proves fixed-work parallel
  evaluation retraces the sequential solution, logical counters, and consumed
  candidate pulls
- scalar-only app: seeded solve flow through typed SSE, status, snapshot-bound
  analysis, checkpointed pause/resume, cancel, terminal-only delete, and
  reconnect bootstrap, including full aggregate telemetry, bounded candidate
  detail, and qualified trace provenance
- MCP app: modeled mixed scalar-plus-list app driven through the official
  `rmcp` client over stdio and stateless Streamable HTTP, covering the
  annotated tool surface, task-backed solve to a terminal snapshot with scalar
  assignment and complete list placement, the retained lifecycle through a
  legacy client, and byte-level stdio stdout purity during a solve, all with
  parallel evaluation enabled

SolverForge `0.26.1` uses list variables for ordered sequences and routes. Both
generated-app end-to-end suites start and observe the real mixed scalar/list
path.

## Non-Goals

Do not reintroduce:

- public scaffold-family flags
- public Tauri shell selection before a real Tauri scaffold exists
- MCP or other shell aliases beyond the canonical `web`, `api`, `cli`, and `mcp` selectors
- hidden `standard` variable-kind aliases
- scalar predecessor topology; ordered sequences belong to list variables
- hidden console/scaffold aliases
- compatibility migrations for unmanaged legacy generated files
- raw score-only SSE payloads
- separate starter-specific solve/render lifecycles
