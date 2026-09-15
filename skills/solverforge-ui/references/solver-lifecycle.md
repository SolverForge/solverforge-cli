# Backend seam and solver lifecycle

The library owns the entire `/jobs` lifecycle. Your app supplies a backend,
subscribes to events, and renders. Any lifecycle logic written in the app is a
bug waiting to happen.

## Backend endpoints (Axum default)

| Method | Path | Purpose |
| --- | --- | --- |
| `GET` | `/demo-data` | `{ defaultId, availableIds }` catalog |
| `GET` | `/demo-data/{id}` | a ready-to-solve plan |
| `POST` | `/jobs` | create a retained job → job id |
| `GET` | `/jobs/{id}` | job summary |
| `GET` | `/jobs/{id}/status` | status summary |
| `GET` | `/jobs/{id}/snapshot[?snapshot_revision=N]` | retained snapshot |
| `GET` | `/jobs/{id}/analysis[?snapshot_revision=N]` | score analysis |
| `GET` | `/jobs/{id}/telemetry` | aggregate + candidate-trace diagnostics |
| `POST` | `/jobs/{id}/pause` | request a runtime-managed pause |
| `POST` | `/jobs/{id}/resume` | resume from the retained checkpoint |
| `POST` | `/jobs/{id}/cancel` | cancel a live or paused job (user-facing Stop) |
| `DELETE` | `/jobs/{id}` | delete a terminal retained job |
| `GET` | `/jobs/{id}/events` | SSE lifecycle stream |

```js
var backend = SF.createBackend({ type: 'axum', baseUrl: '' });
```

Other transports: `{ type: 'fetch', baseUrl: '/api/v1', headers: {...} }` for
Rails-style APIs; `{ type: 'tauri', invoke, listen, eventName }` for Tauri.

Any custom backend passed to `createSolver` must implement `createJob`,
`streamJobEvents`, `getSnapshot`, `analyzeSnapshot`, `pauseJob`, `resumeJob`,
`cancelJob`, and `deleteJob`.

## Stream contract

Events are canonical camelCase: `eventType`, `jobId`, `eventSequence`,
`lifecycleState`, `snapshotRevision`, `currentScore`, `bestScore`, `telemetry`,
`solution`. `eventType` is explicit and one of `progress`, `best_solution`,
`pause_requested`, `paused`, `resumed`, `completed`, `cancelled`, `failed`.

- `progress` is metadata-only — it must not carry `solution`.
- `best_solution` carries `solution` and `snapshotRevision`.
- A run may begin with either a scored `progress` or a scored `best_solution`.
  Do not require `progress` first.
- `paused`, `completed`, `cancelled`, `failed` are authoritative; the solver
  synchronizes the retained snapshot before firing the matching callback.

```json
{ "eventType": "best_solution", "jobId": "job-42", "eventSequence": 2,
  "lifecycleState": "SOLVING", "snapshotRevision": 1,
  "currentScore": "0hard/-1soft", "bestScore": "0hard/-1soft",
  "telemetry": { "movesPerSecond": 15, "stepCount": 4200 },
  "solution": { "id": "job-42", "score": "0hard/-1soft" } }
```

`createJob()` may resolve to a non-empty string, any finite number including
`0`, or an object with a scalar `id`/`jobId`/`job_id`. Non-scalar ids are
rejected before any stream or snapshot call.

## `createSolver(config)` callbacks

| Callback | Fires when | Argument shape |
| --- | --- | --- |
| `onProgress(meta)` | scored progress (no solution) | metadata |
| `onSolution(snapshot, meta)` | live `best_solution` | `snapshot.solution` has the plan |
| `onPauseRequested(meta)` | pause acknowledged, checkpoint not ready | metadata |
| `onPaused(snapshot, meta)` | authoritative pause + snapshot sync | exact retained snapshot |
| `onResumed(meta)` | authoritative resume | metadata |
| `onCancelled(snapshot, meta)` | authoritative cancel | snapshot may be null |
| `onComplete(snapshot, meta)` | authoritative completion (`COMPLETED`/`TERMINATED_BY_CONFIG`) | snapshot |
| `onFailure(message, meta, snapshot, analysis)` | terminal failure | may be partial |
| `onAnalysis(analysis, meta)` | analysis sync (pause/terminal, or `analyzeSnapshot`) | unwrapped analysis |
| `onError(message)` | transport/lifecycle error | string |

Every snapshot-bearing callback should re-render `snapshot.solution` (guarded for
null). Every callback should resync lifecycle data attributes.

Returned API: `start(plan)`, `pause()`, `resume()`, `cancel()`, `delete()`,
`getSnapshot(rev?)`, `analyzeSnapshot(rev?)`, `isRunning()`, `getJobId()`,
`getLifecycleState()`, `getSnapshotRevision()`.

## Lifecycle states

`IDLE → STARTING → SOLVING ⇄ PAUSE_REQUESTED → PAUSED → RESUMING`,
plus `CANCELLING` and terminals `COMPLETED`, `CANCELLED`, `FAILED`,
`TERMINATED_BY_CONFIG`.

The status bar reveals controls from state: Solve on `IDLE` and terminals, Pause
on `STARTING`/`SOLVING`/`PAUSE_REQUESTED`, Resume only on `PAUSED`, Stop on
everything active including `CANCELLING`.

## Rules that keep it correct

- **One retained job at a time.** `start()` rejects while a retained job exists,
  even a terminal one. Before starting, delete any retained terminal job:
  ```js
  function cleanupTerminalJob() {
    var phase = solver.getLifecycleState();
    if (!solver.getJobId() || phase === 'IDLE' || phase === 'PAUSED' || solver.isRunning()) {
      return Promise.resolve();
    }
    return solver.delete();   // terminal-only destructive cleanup
  }
  ```
- **`delete()` is terminal-only and destructive.** It waits for in-flight
  terminal snapshot sync, clears local retained state only after backend deletion
  succeeds, and a failure preserves the job id and terminal state. `COMPLETED`
  and `TERMINATED_BY_CONFIG` require a successful terminal sync first (the solver
  retries once).
- **Pause** sends `pauseJob()` only from `SOLVING`; a pause during `STARTING`
  queues until the job id exists; `PAUSE_REQUESTED` blocks duplicates;
  `RESUMING` cannot pause.
- **Resume** sends `resumeJob()` only from `PAUSED` and settles on `resumed`.
- **Cancel (Stop)** is legal from `SOLVING`, `PAUSE_REQUESTED`, `PAUSED`,
  `RESUMING`; during `STARTING` it queues. During `CANCELLING` Stop stays visible
  and may reattach a closed stream listen-only, but must not send a second cancel.
- **Transport is not lifecycle.** HTTP `EventSource.onerror` means transport
  state. Reconnecting errors are ignored; a closed stream surfaces through
  `onError` while preserving the last authoritative lifecycle, job id, score,
  metadata, and snapshot revision. Interruption never makes `delete()` legal and
  never collapses `PAUSE_REQUESTED`/`RESUMING`/`CANCELLING` to `SOLVING`/`IDLE`.
- **Status bar score** uses `currentScore` (live) during solving.
- Malformed typed fields are ignored, not silently normalized.

## Optional domain routes must be snapshot-scoped

If you add a domain endpoint the UI reads (e.g. server-computed route geometry),
authorize the read against the exact retained snapshot so stale geometry never
renders. Pattern from `uc-deliveries` / `uc-fsr`:

```js
function routeIdentity(solver, plan) {
  var jobId = solver.getJobId();
  var rev = solver.getSnapshotRevision();
  if (!jobId || rev == null) return null;
  return { jobId: String(jobId), revision: String(rev) };
}
// bump a request token on every dataset switch/solve; ignore any response
// whose identity no longer matches the current token.
```

Only fetch `/jobs/{id}/routes?snapshot_revision=N` (or similar) when the
identity is non-null, and redraw on identity change only.

The lifecycle is already proven by the library's own tests. Do not re-test the
state machine in the app; test your transforms instead.
