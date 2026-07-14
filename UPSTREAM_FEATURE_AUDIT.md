# SolverForge Upstream Feature Audit

Audit date: 2026-07-14

This audit compares the current `solverforge-cli` scaffold surface against the
live SolverForge upstream checkout at `/srv/lab/dev/solverforge/solverforge`
and the crates.io release state. Generated projects now target the local
SolverForge `0.19.0` release candidate. The latest published crate remains
`0.18.0` until the coordinated runtime release, so prerelease generated-app
validation must use `SF_USE_LOCAL_PATCHES=1`.

The inclusion bar is starter-safe only: a feature is worth adding to the CLI
when it helps generated projects express a current SolverForge capability
without turning the neutral scaffold into a domain-specific demo.

## Source Evidence

- Published gate: the crates.io API reports `solverforge 0.18.0` as the latest
  published crate on 2026-07-13, with Rust `1.95` and the scaffolded `serde`,
  `console`, and `verbose-logging` feature set still available.
- Upstream local checkout: the workspace and inter-crate dependency baseline is
  `0.19.0`. The published-history tag remains `v0.18.0`; `0.19.0` is the
  coordinated source release candidate that makes list variables the sole
  sequence and route model.
- Previous upstream releases: `solverforge/CHANGELOG.md` lists `0.17.2`
  dynamic construction primitives and required-assignment streaming, `0.17.0`
  CVRP list-domain profile support, and `0.16.0` split route/savings hooks.
- Earlier upstream release: `solverforge/CHANGELOG.md` lists `0.15.2` directed
  projected self-join scoring work.
- Earlier upstream release: `solverforge/CHANGELOG.md` lists `0.15.1`
  features for the bridge crate, dynamic runtime slots, list precedence hooks,
  fixed-owner list handling, and mandatory list construction.
- Previous upstream release: `solverforge/CHANGELOG.md` lists `0.15.0`
  features for typed shared constraint sets, shared grouped-node state,
  assignment value-pattern neighborhoods, and required scalar assignment
  construction.
- Upstream release: `solverforge/CHANGELOG.md` lists `0.14.1` and `0.14.0`
  features for shared route metric classes, owner-aware route hooks,
  complemented direct cross-join groups, and filtered join preservation.
- Earlier upstream release: `solverforge/CHANGELOG.md` lists features for
  generalized grouped collectors and a scoring fix that preserves joined filter
  source indexes.
- Upstream release: `solverforge/CHANGELOG.md` lists `0.13.0` features for
  typed model-aware search defaults, streaming-first stock search, grouped
  assignment ownership/search tuning, bounded grouped scalar and conflict repair
  streams, explicit score weight wrappers, and collector additions.
- Upstream release: `solverforge/CHANGELOG.md` lists a `0.13.0` breaking
  scoring change: constraint streams removed `penalize_with`, `reward_with`,
  and the hard/soft shortcut variants in favor of `penalize(score)`,
  `reward(score)`, and typed dynamic scoring closures.
- Upstream release: `solverforge/CHANGELOG.md` lists `0.12.1` features for
  folding the former coverage behavior into scalar assignment groups and
  unifying assignment-backed grouped scalar construction.
- Upstream release: `solverforge/CHANGELOG.md` lists `0.12.0` features for
  declarative scalar planning contracts, scalar/grouped construction and repair
  configuration, model-owned grouped scalar declarations, consecutive-run
  scoring collectors, and the cleaned public constraint stream surface.
- Upstream domain docs: `docs/extend-domain.md` documents scalar candidate,
  nearby, distance-meter, and construction-order hooks on
  `#[planning_variable]`.
- Upstream solver docs: `docs/extend-solver.md` documents capability-routed
  construction, `group_name` routing into `ScalarGroup`, canonical selector
  defaults, grouped assignment construction, grouped scalar local search, and
  scalar candidate limits.
- Upstream config docs: `crates/solverforge-config/WIREFRAME.md` documents
  `group_name`, `construction_obligation`, grouped-scalar construction limits,
  and `grouped_scalar_move_selector`.
- Upstream macro docs: `crates/solverforge-macros/WIREFRAME.md` documents
  `scalar_groups = "path"` on `#[planning_solution]` and scalar hook arguments
  on `#[planning_variable]`.
- Upstream example: `examples/minimal-shift-scheduling` demonstrates
  `scalar_groups`, `ScalarGroup::assignment`, grouped scalar construction with
  `group_name`, `construction_obligation = "assign_when_candidate_exists"`, and
  `grouped_scalar_move_selector`.
- CLI coverage today: the scaffold targets `solverforge 0.19.0` and includes
  the retained `SolverManager` lifecycle, typed SSE, snapshots, analysis,
  pause/resume/cancel/delete, generated `solverforge.app.toml`, scalar/list
  variable generation, complete executable scalar/list metadata projection,
  countable scalar ranges, full compact telemetry, bounded candidate-detail
  retrieval, and qualified trace jobs.
- Mixed execution gate: fresh runtime and browser scenarios seed a required
  scalar variable plus a list variable, start a retained solve, verify scalar
  assignment and complete list placement, and exercise cancel/terminal cleanup.
- UI release gate: `cargo info solverforge-ui` confirms `solverforge-ui 0.7.0`
  is published; the web scaffold targets its framework-neutral asset release.
- Local checkout note: `/srv/lab/dev/solverforge/solverforge` remains the
  source gate used to inspect the current release notes and feature surface.

## Inclusion Matrix

| Upstream feature | Current CLI coverage | Starter-safe verdict | Proposed CLI/scaffold change | Required tests |
| --- | --- | --- | --- | --- |
| `scalar_groups = "path"` on `#[planning_solution]` | Implemented as an opt-in solution attribute and app-spec/UI projection surface through `solverforge generate scalar-group`. | Included, opt-in only. It is the current model-owned grouped scalar entry point, but it must not appear in the neutral default. | Keep the neutral scaffold unchanged. The command updates the planning solution attribute with `scalar_groups = "scalar_groups"` and owns a clear domain hook seam. | Unit tests for solution-attribute parsing/rewrite and rendered group declarations; scaffold coverage should remain neutral by default. |
| Assignment-backed `ScalarGroup::assignment` | Implemented as `solverforge generate scalar-group NAME --assignment Entity.field` with explicit hook flags and limits. | Included. This remains the replacement for the older coverage-group concept and is generic enough for opt-in scaffolding. | The command requires an existing nullable scalar variable target and generates only metadata/wiring plus hook stubs that panic until the user supplies domain logic. | Parser/generator tests; generated-app `cargo check` after user-owned hook bodies are present; negative tests that neutral scaffolds do not emit scalar groups. |
| Candidate-backed `ScalarGroup` | Implemented as `solverforge generate scalar-group NAME --candidates provider --target Entity.field [...]`. | Included, opt-in only. It is too domain-specific for defaults but valuable as a CLI-owned wiring surface. | The command validates existing scalar targets, wires the candidate provider path, emits an explicit provider stub for local function names, and generates grouped construction plus search config unless `--skip-solver-config` is passed. | Unit tests for target rendering and config refs; generated-app `cargo check` after provider implementation exists. |
| Grouped scalar construction through `group_name` | Implemented as generated `solver.toml` phases for assignment-backed and candidate-backed scalar groups unless `--skip-solver-config` is passed. | Included only after a scalar group exists. Do not change default `solver.toml`. | The command inserts a construction phase with `group_name`, limits, and `construction_obligation = "assign_when_candidate_exists"` for both assignment and candidate groups. | Config graph tests; generated app `solverforge check`; generated app `cargo check`; runtime smoke only if the command claims solve behavior. |
| `grouped_scalar_move_selector` | Implemented as a generated local-search move selector for scalar groups unless `--skip-solver-config` is passed. | Included only after a scalar group exists. Do not add to neutral local-search defaults. | The command inserts grouped local-search config with `group_name`, `max_moves_per_step`, optional `value_candidate_limit`, and `require_hard_improvement`. | Config generation tests and generated app `cargo check`; runtime pipeline if used in an end-to-end generated scenario. |
| `construction_obligation = "assign_when_candidate_exists"` | Implemented as part of generated assignment-backed and candidate-backed scalar-group construction phases. It is not exposed as a standalone config flag. | Include only as part of scalar-group config, not as a neutral scaffold default. | Keep this tied to `generate scalar-group`; do not add it to neutral `solver.toml` or generic `config set` presets. | TOML assertions and generated app compile checks. |
| Scalar `candidate_values` hook | Implemented in this worktree: `generate variable --kind scalar` accepts `--candidate-values`, renders it into `#[planning_variable(...)]`, parses handwritten attributes, persists it in `solverforge.app.toml`, and projects it into `static/generated/ui-model.json`. | Covered. This is a generic scalar modeling capability and remains starter-safe as metadata only. | No further scaffold change. Keep hook bodies domain-owned and keep tests proving generated apps compile when the user provides the hook. | Existing parser/generator/app-spec/scaffold tests. |
| Scalar nearby hooks and distance meters | Implemented in this worktree for `--nearby-value-candidates`, `--nearby-entity-candidates`, `--nearby-value-distance-meter`, and `--nearby-entity-distance-meter`. | Covered. Nearby selectors remain opt-in because the model must bound candidate discovery explicitly. | No further scaffold change. Do not alter `solver.toml` defaults when these flags are present. | Existing parser/projection/generation/scaffold tests; keep coverage that no nearby selector is emitted by default. |
| Scalar construction order hooks | Implemented in this worktree for `--construction-entity-order-key` and `--construction-value-order-key`. | Covered. Required by scalar-only order-sensitive construction heuristics, but metadata alone should not switch solver policy. | No further scaffold change. A future config preset can validate that required hooks exist before selecting order-sensitive construction. | Existing parser/projection/generated-attribute tests. |
| Countable scalar value ranges | Implemented as mutually exclusive `--countable-range FROM..TO`, with non-negative half-open validation, macro emission, domain parsing, app-spec/UI projection, numeric web rendering, and a fresh generated-app compile check. | Covered. This is a canonical scalar value source and does not require a synthetic fact collection. | Keep the stored macro/app-spec form as `from..to`; keep the UI projection structured as numeric `from`/`to` bounds. | Parser/validation unit tests, projection assertions, and generated-app `cargo check` against 0.19.0. |
| Sequence and route modeling | Implemented only through `--kind list --elements <collection>` and current list metadata. | Included. One list representation owns assignment and order and is the canonical sequence architecture. | Keep the public CLI, app spec, parser, generated data, UI projection, and runtime gates scalar/list-only. | List generator/parser/projection tests, mixed generated-app compile and runtime gates, and route-profile coverage. |
| Current construction heuristic catalog | CLI ships conservative scalar/list template defaults and generic `config set`. | Do not mirror every variant in scaffold defaults. | Document which upstream heuristics need opt-in model hooks; keep `first_fit` and `list_cheapest_insertion` templates stable until a command explicitly owns a configured preset. | Docs/audit assertions only unless a preset command is added. |
| Immutable runtime compilation and resolved selector policy | Generated planning macros and `solver.toml` enter the canonical 0.19.0 runtime compiler; the templates do not assemble phases directly. | Covered by the runtime dependency upgrade. This is runtime-owned architecture, not a new scaffold family or compatibility path. | Keep the generated model and config contracts unchanged and validate every shell plus scalar, list, and mixed runtime pipelines against 0.19.0. | Local-patched prerelease scaffold checks, seeded scalar/mixed generated solves, and browser lifecycle tests; repeat the registry-only gate after publication. |
| Qualified candidate execution traces | Implemented as an opt-in config setting, complete typed diagnostic DTO, `GET /jobs/{id}/telemetry`, and additive `POST /jobs/qualified` entry point carrying all required external digests and producer attestation. Candidate pulls remain absent from ordinary SSE/status/snapshot payloads. | Included but disabled by default. This preserves the runtime's compact control plane while exposing the complete diagnostic and qualification surface. | Keep the commented `[candidate_trace]` example, positive-capacity validation, separate detail route, and qualified provenance request aligned with the runtime types. | Unit config validation, scaffold source assertions, fresh generated-app compile checks, and a runtime pipeline that proves normal and qualified retained traces. |
| CVRP list profile and split route/savings hooks | Implemented through `generate variable --kind list`: `--domain cvrp` plus all generic distance, route/savings, metric-class, ownership, construction-order, precedence, and solution-trait metadata. Domain parsing, app spec, and web UI projection preserve the exact values. | Included as opt-in metadata; the neutral scaffold remains domain-free. The CLI rejects profile-owned overrides alongside `--domain cvrp`. | Keep hook bodies and CVRP trait implementation domain-owned. Do not inject fake route logic or a domain-specific default model. | Generator/parser tests, app-spec/UI projection coverage, CVRP conflict validation, upstream macro tests, and generated-app checks for the neutral/list templates. |
| Canonical local-search defaults | CLI templates still specify explicit late-acceptance plus accepted-count local search. | No immediate change. Explicit scaffold defaults are stable and compile; upstream omitted-selector defaults are runtime-owned. | Leave current `solver.toml` templates alone. Consider a later docs note that deleting `move_selector` lets runtime choose canonical defaults. | Existing scaffold/runtime tests. |
| Scoring collectors and grouped/complemented stream APIs, including `consecutive_runs`, `indexed_presence`, and `collect_vec` | Implemented as opt-in advanced constraint skeleton flags while leaving scoring logic to the app. | Include as skeletons only. These APIs are important, but generated neutral constraints should not choose domain-specific collectors. | Keep the skeletons on the public stream surface with explicit panic placeholders. | Existing constraint-generation tests plus focused skeleton assertions. |
| Conflict repair providers via `conflict_repairs = "path"` | Implemented as `solverforge generate conflict-repair CONSTRAINT_ID --provider provider_fn` with optional selector config. | Included, opt-in only. It requires constraint-specific provider code and is not neutral starter behavior. | The command wires `conflict_repairs = "conflict_repairs"`, emits a provider stub for local function names, and stores the exact snake_case constraint ID in generated Rust, app metadata, and solver config. | Unit tests for rendering and config mutation; generated-app `cargo check` after provider implementation exists. |
| Retained `SolverManager` lifecycle | Already represented in templates, routes, DTOs, JS hooks, runtime tests, and E2E tests. | Already covered. | No action. Keep scaffold assertions protecting snapshots, lifecycle metadata, and pause/resume/cancel/delete semantics. | Existing scaffold, runtime, and Playwright lifecycle tests. |
| Clean public stream surface | CLI-generated constraints already use `ConstraintFactory::new().for_each(Plan::...)` and do not teach generated helper-trait imports. | Already covered. | No action beyond keeping examples/docs on public SolverForge API only. | Existing constraint-generation compile tests. |

## Recommended Follow-Up Implementation Slice

The current starter-safe 0.19.0 surface is implemented. The next high-value slice
is domain-specific documentation and executable examples, not more neutral
scaffold defaults.

1. Keep `solverforge new` neutral and continue treating scalar groups and
   conflict repairs as explicit post-scaffold modeling choices.
2. Add human-facing examples for list profiles, scalar groups, conflict repair,
   and qualified diagnostics only with real hook bodies and real provenance.
3. Prove any runtime behavior claims with generated apps whose hook bodies are
   real Rust, not TODO stubs.

## Implementation Status

- Release state: `solverforge 0.18.0` is the latest crates.io publication, while
  this source surface and generated target are `0.19.0`. Use
  `SF_USE_LOCAL_PATCHES=1` for coordinated prerelease validation and rerun the
  registry-only scaffold gate after `0.19.0` is published. The audit uses the
  current `scalar_groups` / `ScalarGroup::assignment` vocabulary instead of the
  superseded coverage-group vocabulary from the earlier 0.12.0 candidate
  surface.
- Implemented: scalar fact-collection and countable value sources plus
  `candidate_values`, nearby candidate hooks, nearby distance meters, and
  construction order keys are accepted by
  `solverforge generate variable --kind scalar`, rendered into the
  `#[planning_variable(...)]` attribute, parsed from handwritten domain files,
  persisted in `solverforge.app.toml`, and projected into
  `static/generated/ui-model.json`.
- Aligned: ordered sequence and route modeling uses list variables end to end.
  The public CLI, canonical parser, app spec, generated data, UI projection,
  checks, and runtime scenarios contain only scalar and list variable contracts.
- Implemented: list `domain`, distance meters, split route/savings hooks,
  savings metric class, fixed ownership, construction ordering, precedence
  hooks, and solution-trait metadata across CLI generation, canonical parsing,
  app-spec persistence, and web UI projection. The stock CVRP profile is
  accepted without allowing conflicting profile-owned overrides.
- Implemented: every compact `SolverTelemetry` field and nested
  phase/selector/move/applied-move breakdown is projected into generated
  status, snapshot, and SSE payloads. Bounded candidate pulls use the separate
  atomic detail accessor and typed diagnostic DTO.
- Implemented: candidate tracing is configurable through
  `candidate_trace.max_entries` with positive-capacity validation. Generated
  web/API apps expose ordinary trace detail and explicitly qualified trace jobs
  with the five immutable external SHA-256 digests required by the current
  runtime contract.
- Implemented: opt-in scalar groups through `solverforge generate scalar-group`
  for assignment-backed and candidate-backed groups, including solution
  attribute wiring, app-spec/UI metadata, local hook stubs, solver config phase
  insertion inside a single CLI-managed `solver.toml` region, and destroy
  cleanup.
- Implemented: `solver.toml` validation now treats scalar groups and conflict
  repairs as exact-ID graph references. `solverforge check` and destroy
  planning inspect construction phases, top-level selectors, neighborhoods,
  nested selector children, and partition child phases instead of scanning only
  flat top-level TOML. This is implemented inside `solverforge-cli`; no
  unpublished upstream `solverforge-config` API is required.
- Implemented: scalar-group model-contract validation is shared by
  `solverforge check` and destructive commands. Candidate-backed groups reject
  assignment-only hooks, assignment rules require sequence keys, stale
  scalar-group targets are reported, solver config group references are checked,
  and `destroy entity` / `destroy variable` reject targets that are still owned
  by scalar groups.
- Implemented: opt-in conflict repairs through
  `solverforge generate conflict-repair`, including solution attribute wiring,
  app-spec/UI metadata, local provider stubs, solver config phase insertion,
  and destroy cleanup.
- Implemented: `solverforge config set` performs lossless non-phase TOML edits
  such as `termination.seconds_spent_limit`; ordered `phases` are edited
  manually or by future phase-specific commands, not by dotted-key mutation.
- Implemented: advanced public collector skeletons for consecutive runs,
  indexed presence, collected vectors, grouped complement, and projected
  grouped constraints.
- Hardened: advanced grouped constraint skeletons use named source helpers and
  typed scoring closures instead of unsupported post-group filters, and fresh
  generated apps now compile before users replace the TODO predicates.
- Aligned: the list scaffold's built-in soft constraints use the current
  typed dynamic `penalize(|item| Score::...)` scoring API instead of the removed
  `penalize_with` helper.
- Preserved: the CLI still does not generate real Rust hook bodies. Generated
  stubs fail fast with TODO panics until the user-owned hook functions are
  implemented.
- Preserved: neutral `solver.toml` policy is unchanged. Its commented candidate
  trace block documents the opt-in without enabling diagnostic overhead.
  Scalar/list metadata flags do not select unrelated heuristics automatically.
- Not implemented by design: neutral scalar-group, conflict-repair, or grouped
  solver defaults. These remain opt-in modeling resources.

## Non-Goals

- Do not add `minimal-shift-scheduling` or any domain-specific demo to
  `solverforge new`.
- Do not make grouped scalar construction or `grouped_scalar_move_selector` the
  neutral default.
- Do not generate hook functions that silently return fake business logic.
  Generated local stubs must panic until the user supplies real logic.
- Do not reintroduce scaffold-family aliases, legacy variable kinds, or
  compatibility rewrites for old generated project shapes.
- Do not add scalar predecessor topology; ordered sequences and routes belong to
  list variables.

## Validation Gate For Future Code Changes

Any implementation from this audit should pass:

- focused parser/generator/app-spec tests for the new metadata;
- scaffold contract tests proving fresh generated projects include only the
  opt-in scalar-group surface requested by the user;
- generated app `cargo check` through `SF_USE_LOCAL_PATCHES=1` while the
  coordinated `0.19.0` runtime is unpublished, followed by the same check with
  local patches disabled after publication;
- runtime pipeline coverage only if the new command claims actual solving
  behavior;
- `SF_USE_LOCAL_PATCHES=1 cargo test --test scaffold_test` for local upstream
  compatibility before the public `0.19.0` release exists.
