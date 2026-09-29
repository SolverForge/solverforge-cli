# Verification

Four gates. Pass all of them before calling a model done. Gate 4 is the fastest
and the one that catches a mis-modeled rule; the others catch wiring,
compilation, and real runtime failures.

## 1. `solverforge check` — structural

Validates `src/domain/mod.rs`, `src/constraints/mod.rs`, the domain model,
solution, entities, model resources, and `solver.toml` refs. It does **not**
compile Rust and does **not** run the solver, so it cannot see placeholder
`panic!`s or a broken stream source. Exit code 0 means the managed structure is
well-formed.

## 2. `cargo check` — compiles

```bash
cargo check
```

Catches wrong field names, missing imports, wrong score types, and closure
signature mismatches. It will **not** catch the runtime localizing-source panic
(that is a runtime assertion, not a compile error). The generated scaffolds
target `solverforge 0.19.7`; the first build downloads and compiles the runtime
and web dependencies, which can take a few minutes. `SF_USE_LOCAL_PATCHES=1`
exists in the CLI's own test harness for prerelease targets — do not set it for
ordinary app builds.

## 3. A real solve — behavioral

This is the gate that matters. `check` + `cargo check` passing while the solve
panics is the most common "it looks done but isn't" state.

### Bundled helper

The helper under this loaded skill directory automates the web/API/MCP flow:

```bash
<skill-dir>/scripts/solve-smoke-test.sh <app-dir> [port] [demo-size]
```

For web/API it builds and boots the app, starts a job from generated demo data,
requires `COMPLETED` plus non-null current/best scores, and fails on timeout,
cancel, `FAILED`, or a constraint panic. For MCP it boots the HTTP transport
(`--http`) and requires a healthy, panic-free server; protocol-level proof comes
from calling the tools through the harness the server is registered in (register
with `solverforge connect --write <harness>`, reload, then call `solve`,
`get_status`, and `get_best_solution`). The CLI's own pipeline tests drive the
same server with `rmcp` over stdio and Streamable HTTP. For
CLI it validates compilation and the banner-prefixed demo-data JSON only; the
generated CLI has no solve command. It cleans up the server and temporary files.
Requires `python3` for JSON parsing, plus `curl` for web/API/MCP, and a few
minutes on a cold build. Set `SF_SMOKE_TIMEOUT_SECONDS` when the configured
solver termination needs more or less than the default 120 seconds.

### Manual flow (web / api)

```bash
cd my-app
solverforge server --port 7860 &     # or `--debug` for a faster build

curl -s localhost:7860/health                                   # {"status":"UP"}
curl -s localhost:7860/demo-data                                # defaultId + availableIds
curl -s localhost:7860/demo-data/STANDARD > /tmp/plan.json
JOB=$(curl -s -X POST -H 'content-type: application/json' \
  --data @/tmp/plan.json localhost:7860/jobs | sed -E 's/.*"id":"([^"]+)".*/\1/')
curl -s "localhost:7860/jobs/$JOB/status"
```

Read the response:

- `lifecycleState`: `SOLVING` is healthy; a terminal `FAILED` means the run
  failed. `COMPLETED`/`CANCELLED` are terminal and expected after Stop.
- `currentScore` / `bestScore`: e.g. `0hard/5soft`. Hard must reach `0` when the
  problem is feasible by construction.
- `telemetry.stepCount` / `movesEvaluated` moving means the search is alive.

Then confirm the server log has no constraint panic:

```bash
grep -i panic <server-log>   # expect nothing
```

A panic looks like:

```
constraint `no_double_book` received descriptor 0, but source Unknown cannot localize entity indexes
```

That is the localizing-source defect. Fix it by using `Plan::<collection>()` (see
`constraint-patterns.md`).

### Manual flow (cli)

```bash
cargo run -- demo-data
```

Expect the SolverForge banner followed by valid JSON and exit 0. There is no
generated solve command or job lifecycle. This proves serialization only. To
claim behavioral completion, add and test a solve subcommand backed by
`src/solver/service.rs`, or add a library integration test that drives the
service to a terminal score.

## 4. Per-constraint unit tests — the fast loop

`check`, `cargo check`, and a full solve are all slow. Add one unit test per
constraint so a wrong rule fails in milliseconds, before any solve. This is the
only gate that catches "the rule was modeled as a precomputed flag": flip the
domain input and assert the exact score delta.

The idiom (from
[`uc-lessons`](https://github.com/SolverForge/solverforge-usecases/blob/main/uc-lessons/src/constraints/teacher_availability.rs)):

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use solverforge::ConstraintSet;

    fn score_for(available: bool, assigned: bool) -> HardMediumSoftScore {
        let teachers = vec![Teacher::new(0, "A", vec![available])];
        let timeslots = vec![Timeslot::new(0, Weekday::Mon, time(8), time(9))];
        let mut lessons = vec![Lesson::new(0, "Math".to_string(), 0, None, 60)];
        if assigned {
            lessons[0].timeslot_idx = Some(0);
        }
        let plan = Plan::new(timeslots, teachers, vec![], lessons, vec![]);
        (constraint(),).evaluate_all(&plan)   // ONLY this constraint
    }

    #[test]
    fn penalizes_assigned_unavailable_teacher() {
        assert_eq!(score_for(false, true), HardMediumSoftScore::of_hard(-1));
    }

    #[test]
    fn ignores_available_or_unassigned() {
        assert_eq!(score_for(true, true), HardMediumSoftScore::ZERO);
        assert_eq!(score_for(false, false), HardMediumSoftScore::ZERO);
    }
}
```

- `(constraint(),).evaluate_all(&plan)` isolates one rule; use
  `create_constraints().evaluate_all(&plan)` for whole-model interaction tests.
- Assert the **exact** level: `HardMediumSoftScore::of_hard(-1)` /
  `of_medium(1)` / `of_soft(1)`, or `.hard()` / `.medium()` / `.soft()` on the
  result.
- Cover three cases per rule: the violation, the legal case (delta 0), and the
  unassigned/out-of-scope case (delta 0).
- Use `evaluate_detailed(&plan)` to find a named analysis and inspect
  `analysis.matches` / `analysis.score` (see `uc-hospital/tests/constraints.rs`).
  It is also how you prove a self-join scores each pair exactly once.

Embedded/hand-written models have no other fast gate: `cargo test` with these
tests is the primary verification loop (`existing-model.md`).

## Explaining a plan

After a solve, derive explanations from framework analysis, not from predicates
you rewrite by hand:

- `solution.analyze()` returns a `ScoreAnalysis` for the retained solution.
- `SolverManager::analyze_snapshot(...)` analyzes a retained snapshot; the
  web/API surface exposes it at `GET /jobs/{id}/analysis` (and
  `GET /jobs/{id}/snapshot` for the snapshot itself).
- `evaluate_detailed` gives per-constraint `matches`, so an "unscheduled item
  because…" explanation comes from the same code the solver scored, not a
  parallel copy that can drift.

## What "done" means

- `solverforge check` exits 0.
- `cargo check` exits 0.
- For web/API, a real solve reaches `COMPLETED` with no constraint panic and
  publishes current/best scores; hard score is 0 for a feasible model.
- For CLI, a real solve is required only after you add a solve entry point. The
  generated `demo-data` command alone is not behavioral verification.
- No placeholder `panic!` remains in any constraint you claim to have
  implemented.

If a solve is slow, lower `termination.seconds_spent_limit` for iteration
(`solverforge config set termination.seconds_spent_limit 10`) and raise it for
final runs.
