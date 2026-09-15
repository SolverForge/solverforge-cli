# Proving the UI works

Compilation is not evidence. The UI is done when the real app boots, solves, and
renders correct geometry in a live browser. Everything below is cheap; do it
before reporting completion.

## 1. Run the app

```sh
# from the app root
solverforge check          # model/config consistency
solverforge server         # builds and serves on http://localhost:7860
# or: cargo run
```

Concurrently `cargo test` for the domain/solver.

If you changed solverforge-ui itself (not just the app), also run:

```sh
make lint-frontend         # ESLint on js-src/, tests/, scripts/
make test-frontend         # node --test tests/*.test.js
make test-browser          # Playwright demo fixtures; needs one-time make browser-setup
make assets                # rebuild static/sf/sf*.css|js after source edits
```

## 2. Browser acceptance checklist

Open the served URL and verify, in order — the same path a user takes:

1. **Assets load.** `/sf/sf.css`, `/sf/sf.js`, Font Awesome, and any module/vendor
   assets return 200 (no 404 in the console). `SF` is defined.
2. **Boot.** Header shows the app title/subtitle; the status bar reads "Ready";
   demo data loads and renders without a bootstrap error.
3. **Solve.** Click Solve: spinner starts, state moves to `SOLVING`, score
   updates, the status text changes, Pause/Stop appear.
4. **Pause / Resume.** Pause settles to `PAUSED` with the exact snapshot; Resume
   returns to `SOLVING`. Stop is visible in every active state including
   `CANCELLING`.
5. **Stop, then new solve.** Stop reaches `CANCELLED`; pressing Solve deletes the
   retained job and starts a clean run (no "retained job exists" error).
6. **Analyze.** The chart button opens the Score Analysis modal populated with
   constraint rows; Escape closes it.
7. **Geometry.** Timeline blocks align to their real intervals; adjacent blocks
   do not overlap; true overlaps pack onto separate rows; the sticky header and
   lane labels stay put while the body scrolls horizontally.
8. **Unassigned** work is visible (lane, badge, or table), not silently dropped.
9. **Tabs.** Every view renders; empty views show an explicit message; the Data
   tab lists the model; the API tab lists endpoints.
10. **Errors.** A demo-data failure shows the inline banner and disables Solve
    rather than throwing.

Inspect the console: no uncaught exceptions, no React-style hydration noise,
no repeated duplicate `best_solution` for the same revision.

## 3. Lifecycle data attributes (make state observable)

After every solver event, write these onto the app root (`#sf-app`):

```js
function syncMarkers(meta) {
  const jobId = solver.getJobId();
  const rev = solver.getSnapshotRevision();
  const state = (meta && meta.lifecycleState) || solver.getLifecycleState();
  if (jobId) app.dataset.jobId = String(jobId); else delete app.dataset.jobId;
  if (rev != null) app.dataset.snapshotRevision = String(rev); else delete app.dataset.snapshotRevision;
  if (state && state !== 'IDLE') app.dataset.lifecycleState = state; else delete app.dataset.lifecycleState;
}
```

Add `app.dataset.bootstrapError = 'true'` on demo failure. These are the hooks
browser tests and humans use to confirm the machine is in the state it claims.
`#sf-app[data-job-id]` is the standard "a job exists" readiness signal.

## 4. Unit-test the transforms

The renders are hard to test; the transforms are not. Extract pure functions and
test them with Node (`node --test`):

- date/time parsing → integer minutes for a known input,
- axis bounds and day grouping,
- lane grouping preserves empty entities and appends unassigned,
- item geometry is exact for adjacent and overlapping intervals,
- tone assignment is deterministic for a key.

This matches the library's own approach (`tests/*.test.js`) and the use cases
(`uc-hospital/tests/frontend/*.test.mjs`). Keep solver lifecycle tests out of the
app — the library owns that state machine.

## 5. Stable selectors for browser tests

Prefer, in this order: the lifecycle data attributes, `#sf-app`, stock component
classes (`.sf-rail-timeline`, `.sf-constraint-dot`, `#sfStatusText`,
`.sf-marker-vehicle`), then app-prefixed classes (`.deliveries-route-row`). Avoid
selectors tied to copy or nesting depth.

## 6. Common runtime failures

| Symptom | Cause | Fix |
| --- | --- | --- |
| `rail.createTimeline` throws about normalization | string/`Date`/float minute field | convert to integer minutes at the boundary |
| Solve does nothing after a previous run | retained terminal job not deleted | call `solver.delete()` before `start()` |
| Pause/Resume buttons never appear | status bar not bound to header | `statusBar.bindHeader(header)` before solving |
| Timeline header/body out of sync | timeline created before mount, or re-created without viewport | rely on post-mount sync, or re-apply `setViewport` |
| Overlapping blocks draw on one track | intervals are actually overlapping, not adjacent | expected: overlaps pack onto rows; fix only if data is wrong |
| Empty entities vanish | lanes built only from assigned records | build one lane per fact entity |
| `/sf/*` 404 | backend not merging `solverforge_ui::routes()` or asset path wrong | use the mount template |
| Model changes disappear | hand-edited `generated/ui-model.json` | re-run `solverforge generate`; move overrides to an app file |
| Duplicate startup `best_solution` | backend re-emits a retained bootstrap snapshot | do not emit an identical duplicate after seeding |
| Stale route geometry after a re-solve | geometry not scoped to snapshot | guard fetches with `{jobId, snapshotRevision}` |
