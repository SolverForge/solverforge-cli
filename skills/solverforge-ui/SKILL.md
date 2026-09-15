---
name: solverforge-ui
description: Build, extend, and polish the web UI of a SolverForge constraint-optimization app using the shipped solverforge-ui component library (SF.* vanilla-JS globals) against the app's existing backend. Use when a project scaffolded with solverforge-cli still has the thin neutral shell in static/ and the user wants a domain-faithful interface — solver controls, status bar and score, rail scheduling timeline, Gantt, map routes, tables, modals, toasts, and score analysis. Covers mapping generated entities, facts, scalar/list variables, and constraints onto timeline/Gantt/map/table surfaces in a domain-agnostic way while preserving the generated model and the /sf + /jobs + /demo-data contract. Do not use for editing solverforge-ui's own component internals unless changing the shipped public API.
license: Apache-2.0
compatibility: opencode, claude-code, codex, generic Agent Skills harnesses
metadata:
  api-reference: README.md
---

# Building SolverForge web UIs with solverforge-ui

The user's app was scaffolded with `solverforge-cli`. It already boots, already
serves the solver, and already ships a **thin neutral shell** built from these
components. Your job is not to invent a frontend. Your job is to **extend that
shell into a domain-faithful interface using only the shipped components**, so
the planning model stays the single source of truth and the backend stays
untouched.

Golden rule: **every backend element has a corresponding UI element**. Read the
model, decide which surface expresses it, map the model onto that surface. Never
build a parallel model in the browser.

This file is the workflow and the judgment calls. Load the reference file named
for the task:

| Need | Read |
| --- | --- |
| Scaffold anatomy, boot contract, `ui-model.json`, module layout, CSS | `references/app-architecture.md` |
| Exact `SF.*` factories, config keys, return values, tones | `references/components.md` |
| Backend seam, `createSolver` contract, lifecycle rules | `references/solver-lifecycle.md` |
| Turning domain records into timeline/Gantt/map/table models | `references/data-mapping.md` |
| "My problem is X, which surface should I build?" | `references/problem-shapes.md` |
| Proving the UI works end to end | `references/validation.md` |

The library's `README.md` at the matching `solverforge-ui` version is the
authoritative API contract; this skill is a playbook over it. Check the version
the app actually pins in `Cargo.lock` before relying on a detail.

## Version compatibility

This playbook targets the `solverforge-ui` version your app pins; read
`Cargo.toml`/`Cargo.lock`, then read that version's `README.md` as the API
contract. The current `solverforge-cli` scaffolds the latest published line.

## Non-negotiable rules

These preserve correctness and the user's model. Breaking any of them produces a
UI that looks fine and is wrong.

1. **Keep the backend seam.** The app serves `/sf/*` from
   `solverforge_ui::routes()` and the app's `static/` from `ServeDir`. Do not
   replace `SF.createBackend()`/`SF.createSolver()` with hand-rolled `fetch` or
   `EventSource`, do not reimplement SSE parsing, and do not move lifecycle
   logic into render code. The library owns the `/jobs` lifecycle state machine.
2. **Render the user's records, not your own domain.** Use the field names in
   the app's `src/domain/`, `src/api/dto.rs`, and `generated/ui-model.json`.
   Never rename a domain concept to something more familiar, and never
   synthesize fields the backend does not send.
3. **Deep-clone every plan before it reaches the solver.** `JSON.parse(JSON.stringify(plan))`.
   The rendered plan and the payload sent to `/jobs` must never share object
   identity with backend state.
4. **Timeline time is integer minutes only.** Every minute field and viewport
   boundary is an integer. Reject/convert strings, `Date`s, and fractional
   minutes on the consumer side; the library rejects them rather than coercing.
5. **Never hand-edit `static/generated/ui-model.json`.** `solverforge generate`
   and `solverforge destroy` regenerate it. Put app-owned presentation metadata
   in a separate file (e.g. `static/ui-overrides.json`) or derive it at runtime.
6. **Text is safe by default.** `SF.el`, `createTable`, `createModal`, and
   `createTabs` render strings as text. Use the documented `unsafeHtml` fields
   only with trusted, escaped content. Never interpolate backend strings into
   `innerHTML` without `SF.escHtml`.
7. **Surface real state, not optimism.** Wire every authoritative solver event
   (`onSolution`, `onPaused`, `onCancelled`, `onComplete`, `onFailure`) to a
   re-render, and expose loading/empty/error states. Do not render a result the
   backend has not confirmed.
8. **Preserve generated views until the replacement is better.** The scaffold's
   generic scalar/list timelines already work. Replace or augment them only when
   a domain-faithful surface expresses the model more clearly, and keep an
   escape hatch (the Data tab) always available.

## The extension workflow

### 1. Recon — read the model before writing UI

From the app root, read (in this order) and note the exact field names:

- `solverforge.app.toml` — app name, solution name, score type, demo sizes.
- `static/generated/ui-model.json` — `entities`, `facts`, `constraints`,
  `views[]` (each with `kind`, `entity`, `entityPlural`, `sourcePlural`,
  `variableField`, `allowsUnassigned`), `scalarGroups`, `conflictRepairs`.
- `src/domain/*.rs` and `src/api/dto.rs` — the real serialized shape of a plan
  and a solution: which collection is the planning entity, which holds the
  assigned values, how time/distance/quantity fields are typed and named.
- `src/constraints/*.rs` — filenames and functions name each constraint and its
  hard/soft nature (the generated `constraints` list is names only).
- `src/main.rs`, `static/index.html`, `static/app.js` — the existing seam and
  shell, so you extend rather than replace.

Boot the app once (`solverforge server`, or `cargo run`) and fetch
`/demo-data` and `/demo-data/<defaultId>` to see a real plan. Read the actual
JSON; do not assume.

### 2. Classify the problem shape

Each `views[]` entry is one planning variable: a `scalar` assignment
(entity → one value from a fact collection or countable range) or a `list`
ordering (entity → an ordered sequence of elements). Decide what each variable
*means in the real world*: a time slot, a resource, a position, a route.

Then pick the primary surface for each view. See `references/problem-shapes.md`
for the full catalog; the short version:

- Time-bearing assignment/shift/booking → **rail timeline** (`SF.rail.createTimeline`) with one lane per assignee or location.
- Time slot grid (days × periods) → **rail timeline** with a day axis and one lane per group/room/teacher.
- Ordered sequence or route → **rail timeline** per route lane, **Gantt**, or **map** when geo.
- Precedence-based project tasks → **Gantt** (`SF.gantt.create`).
- Geospatial routing → **map module** plus route cards and a timeline. Render
  backend geometry only; how a `solverforge-maps`-backed app builds travel
  times, route geometry, and the `/jobs/{id}/routes` contract is the
  `solverforge-modeling` skill's routing-and-maps reference.
- Non-temporal assignment / packing / counts → **tables + KPI cards**.

### 3. Preserve the shell skeleton

The neutral shell already wires the correct lifecycle. Keep its structure and
extend it (or refactor it into the modular shell in
`references/app-architecture.md` without changing behaviour):

```
#sf-app
  SF.createHeader({ logo, title, subtitle, tabs, actions, onTabChange })
  statusBar = SF.createStatusBar({ constraints }); statusBar.bindHeader(header)
  statusBar.el
  one hidden .sf-content panel per tab
  SF.createFooter({ links })
SF.createSolver({ backend, statusBar, <all callbacks> })
```

Copy the scaffold's `createSolver` callback set and demo bootstrap verbatim
first, then add renderers. The lifecycle is the hard part; do not get creative
with it. See `references/solver-lifecycle.md`.

### 4. Map the model onto the surface

Write pure transform functions (`buildAxis`, `buildLanes`, `toItem`, `toneFor`)
that take the plan JSON and return component configs. Keep them free of DOM and
side effects so they are unit-testable. Follow `references/data-mapping.md`,
which documents the four time-normalization conventions seen in shipped apps and
the lane/item/overlay recipes.

Prefer the module layout for anything non-trivial: pure model builders,
a rail renderer, a shell, a solver controller. The hospital app in
`solverforge-usecases` is the reference architecture.

### 5. Wire interactions and feedback

- Header actions: `onSolve` (gate on `canSolve()`), `onPause`, `onResume`,
  `onCancel`, `onAnalyze` (opens the score-analysis modal).
- Cleanup: before starting, delete any retained terminal job (`solver.delete()`),
  exactly as the scaffold does.
- Surface errors: inline bootstrap banner for demo-data failures, and
  `SF.showError`/`SF.showToast` for runtime failures.
- Set the lifecycle data attributes on the app root after every event
  (`data-job-id`, `data-snapshot-revision`, `data-lifecycle-state`) so browser
  tests and the user can observe real state.

### 6. Style with tokens, not forks

Use `--sf-*` design tokens and existing `sf-*` classes. App CSS is for layout
gaps (grids, responsive reflow, app-specific density), never for re-skinning
shipped components. Scope app classes with an app prefix. See the CSS section
of `references/app-architecture.md`.

### 7. Validate end to end

Do not call it done on a successful compile. Boot the real app, solve, and check
the live browser: assets load, Solve/Pause/Resume/Stop behave, the score moves,
the analysis modal is populated, the timeline geometry is exact, unassigned work
is visible. Follow `references/validation.md`. If the repo has browser tests,
mirror their selectors and data attributes.

## Reference files

- `references/app-architecture.md` — scaffold anatomy, the boot contract, the
  generated UI model, extension patterns, CSS layering, version pinning.
- `references/components.md` — every `SF.*` factory with config keys, return
  values, the tone palette, and unsafe-HTML fields.
- `references/solver-lifecycle.md` — backend endpoints, `createBackend`,
  `createSolver`, the full state machine and the rules that keep it correct.
- `references/data-mapping.md` — the transform playbook: axes, lanes, items,
  overlays, summaries, tones, unassigned work, Gantt tasks, map routes.
- `references/problem-shapes.md` — optimization shape → surface → composition.
- `references/validation.md` — how to prove the UI works, and the failure modes.

Paired skill: `solverforge-modeling` turns a described problem into the app this
skill extends, and its routing-and-maps reference covers the `solverforge-maps`
backend behind a geospatial UI.

## Worked examples to imitate

The `solverforge-usecases` repo holds four finished apps. Read the one closest
to your shape and copy its *structure*, not its domain:

| Shape | Example | What to study |
| --- | --- | --- |
| Timetable with discrete slots | `uc-lessons/static/` | day+clock → minutes, one lane per group/room/teacher, subject tones, empty lanes kept |
| Shift rostering by employee/location | `uc-hospital/static/app/` | modular shell, TZ-free timestamp parsing, availability overlays, unassigned lane, pure model modules |
| Vehicle routing with a map | `uc-deliveries/static/app/` | map + timeline + route cards, seconds → minutes, encoded polylines, snapshot-scoped geometry |
| Field service routes | `uc-fsr/static/` | index-based bitmask domains, replaying routes to build items, backend-supplied route colors |

In-repo runnable fixtures live in `demos/` (`full-surface.html`, `timeline.html`,
`timeline-dense.html`, `rail.html`) — the fastest way to confirm exact rendering
and geometry.

## Top failure modes

- Rendering `Date`/string/float time into the timeline → rejected input or wrong
  geometry. Normalize to integer minutes.
- Forking the lifecycle: local `fetch('/jobs')` + manual state. Use `createSolver`.
- Starting a second solve without deleting the retained terminal job → rejected.
- Hand-editing `generated/ui-model.json` → overwritten on the next `generate`.
- Dropping empty entity lanes, hiding unassigned work, or inventing a prettier
  label than the model's → the UI no longer reflects the model.
- Re-skinning `sf-*` classes globally → breaks the shared component contract.
- Claiming completion from `cargo build` alone — solve and inspect the browser.
