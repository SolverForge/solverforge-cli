# SolverForge Upstream Feature Audit

Audit date: 2026-06-10

This audit compares the current `solverforge-cli` scaffold surface against the
live SolverForge upstream checkout at `/srv/lab/dev/solverforge/solverforge`
and the latest published `solverforge 0.15.1` crate that generated projects
target. The local upstream checkout and published scaffold target are aligned
at `solverforge 0.15.1`.

The inclusion bar is starter-safe only: a feature is worth adding to the CLI
when it helps generated projects express a current SolverForge capability
without turning the neutral scaffold into a domain-specific demo.

## Source Evidence

- Published gate: `cargo info solverforge` confirms `solverforge 0.15.1` is
  the latest published crate with Rust `1.95` and the scaffolded `serde`,
  `console`, and `verbose-logging` feature set still available.
- Upstream local checkout: `solverforge/Cargo.toml` is at `0.15.1`, and
  `solverforge/CHANGELOG.md` lists the published `0.15.1` bridge, dynamic
  runtime slot, precedence, fixed-owner list, and mandatory list construction
  work.
- Upstream release: `solverforge/CHANGELOG.md` lists earlier `0.15.1` features
  for fixed-owner list placement and true-regret list insertion scoring.
- Previous upstream release: `solverforge/CHANGELOG.md` lists `0.15.0` features for
  typed shared constraint sets, shared grouped-node state, assignment
  value-pattern neighborhoods, and required scalar assignment construction.
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
- CLI coverage today: the scaffold targets `solverforge 0.15.1` and includes
  the retained `SolverManager` lifecycle, typed SSE, snapshots, analysis,
  pause/resume/cancel/delete, generated `solverforge.app.toml`, scalar/list
  variable generation, and scalar hook metadata projection.
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
| Current construction heuristic catalog | CLI ships conservative scalar/list template defaults and generic `config set`. | Do not mirror every variant in scaffold defaults. | Document which upstream heuristics need opt-in model hooks; keep `first_fit` and `list_cheapest_insertion` templates stable until a command explicitly owns a configured preset. | Docs/audit assertions only unless a preset command is added. |
| Canonical local-search defaults | CLI templates still specify explicit late-acceptance plus accepted-count local search. | No immediate change. Explicit scaffold defaults are stable and compile; upstream omitted-selector defaults are runtime-owned. | Leave current `solver.toml` templates alone. Consider a later docs note that deleting `move_selector` lets runtime choose canonical defaults. | Existing scaffold/runtime tests. |
| Scoring collectors and grouped/complemented stream APIs, including `consecutive_runs`, `indexed_presence`, and `collect_vec` | Implemented as opt-in advanced constraint skeleton flags while leaving scoring logic to the app. | Include as skeletons only. These APIs are important, but generated neutral constraints should not choose domain-specific collectors. | Keep the skeletons on the public stream surface with explicit panic placeholders. | Existing constraint-generation tests plus focused skeleton assertions. |
| Conflict repair providers via `conflict_repairs = "path"` | Implemented as `solverforge generate conflict-repair CONSTRAINT_ID --provider provider_fn` with optional selector config. | Included, opt-in only. It requires constraint-specific provider code and is not neutral starter behavior. | The command wires `conflict_repairs = "conflict_repairs"`, emits a provider stub for local function names, and stores the exact snake_case constraint ID in generated Rust, app metadata, and solver config. | Unit tests for rendering and config mutation; generated-app `cargo check` after provider implementation exists. |
| Retained `SolverManager` lifecycle | Already represented in templates, routes, DTOs, JS hooks, runtime tests, and E2E tests. | Already covered. | No action. Keep scaffold assertions protecting snapshots, lifecycle metadata, and pause/resume/cancel/delete semantics. | Existing scaffold, runtime, and Playwright lifecycle tests. |
| Clean public stream surface | CLI-generated constraints already use `ConstraintFactory::new().for_each(Plan::...)` and do not teach generated helper-trait imports. | Already covered. | No action beyond keeping examples/docs on public SolverForge API only. | Existing constraint-generation compile tests. |

## Recommended Follow-Up Implementation Slice

Scalar hook metadata is implemented in this worktree. The next high-value
starter-safe slice after this pass is documentation and generated-app smoke
coverage for the opt-in advanced resources.

1. Keep `solverforge new` neutral and continue treating scalar groups and
   conflict repairs as explicit post-scaffold modeling choices.
2. Add human-facing examples for `generate scalar-group` and
   `generate conflict-repair` only after the generated hook-body workflow is
   stable enough to document without implying fake business logic.
3. Prove any runtime behavior claims with generated apps whose hook bodies are
   real Rust, not TODO stubs.

## Implementation Status

- Validated: `solverforge 0.15.1` is the current published scaffold target and
  latest crates.io publication. The local upstream checkout is also `0.15.1`,
  so published scaffolds no longer need a prerelease local-patch gate for this
  target. The audit uses the current `scalar_groups` /
  `ScalarGroup::assignment` vocabulary instead of the superseded coverage-group
  vocabulary from the earlier 0.12.0 candidate surface.
- Implemented: scalar `candidate_values`, nearby candidate hooks, nearby
  distance meters, and construction order keys are accepted by
  `solverforge generate variable --kind scalar`, rendered into the
  `#[planning_variable(...)]` attribute, parsed from handwritten domain files,
  persisted in `solverforge.app.toml`, and projected into
  `static/generated/ui-model.json`.
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
- Preserved: neutral `solver.toml` templates are unchanged. Scalar metadata
  flags do not select nearby, construction, or grouped scalar heuristics
  automatically.
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

## Validation Gate For Future Code Changes

Any implementation from this audit should pass:

- focused parser/generator/app-spec tests for the new metadata;
- scaffold contract tests proving fresh generated projects include only the
  opt-in scalar-group surface requested by the user;
- generated app `cargo check` against published crates with local patches
  disabled;
- runtime pipeline coverage only if the new command claims actual solving
  behavior;
- `SF_USE_LOCAL_PATCHES=1 cargo test --test scaffold_test` only when validating
  local upstream compatibility before a public release exists.
