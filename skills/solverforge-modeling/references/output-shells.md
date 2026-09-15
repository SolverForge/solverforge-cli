# Output shells

`--shell` is the "type of output" selector. It changes adapters, not the model:
the neutral scaffold is identical for every shell, and facts/entities/variables/
constraints behave the same. The current public set is exactly `web`, `api`,
`cli`.

| Shell | Frontend assets | HTTP API (Axum/SSE) | Clap CLI | Job lifecycle | Typical use |
| --- | --- | --- | --- | --- | --- |
| `web` (default) | yes (`static/`, `solverforge-ui`) | yes | no | yes | end-to-end demo / UI |
| `api` | no | yes | no | yes | headless service, other clients |
| `cli` | no | no | yes | no | demo-data terminal scaffold; add solving yourself |

Recorded in `solverforge.app.toml` as `[app].shell`. API and CLI specs omit
`ui_source`, and later mutations preserve that absence.

## web

```bash
solverforge new my-app                 # or --shell web
cd my-app
solverforge server
```

- Emits `static/`, `static/generated/ui-model.json`, `static/sf-config.json`, and
  the `solverforge-ui` + `solverforge-maps` dependencies.
- Full JSON/SSE API: `/health`, `/info`, `/demo-data`, `/demo-data/{id}`,
  `/jobs`, `/jobs/{id}`, `/jobs/{id}/status`, `/jobs/{id}/snapshot`,
  `/jobs/{id}/analysis`, `/jobs/{id}/telemetry`, `/jobs/{id}/pause`,
  `/jobs/{id}/resume`, `/jobs/{id}/cancel`, `/jobs/{id}/events` (SSE),
  `/jobs/qualified`, and the demo catalog.
- The default frontend is intentionally thin and composes `SF.*` primitives.
  For UI work, use the `solverforge-ui` skill.

## api

```bash
solverforge new my-service --shell api
cd my-service
solverforge server
```

- Same routes and job lifecycle as web, without `static/`, `solverforge-ui`,
  `solverforge-maps`, or the UI projection.
- `solverforge routes` works.
- Best current choice when the user wants a machine-facing service (the MCP
  shell is not available on this release line).

## cli

```bash
solverforge new my-tool --shell cli
cd my-tool
cargo run -- demo-data
```

- Clap entry point; no Axum, no SSE, no `static/`.
- `solverforge server` is rejected. `solverforge routes` does not apply.
- Shares the same domain/data contract and exposes `PlanDto` JSON via
  `demo-data`.
- The generated command does not invoke the solver. If the requested product is
  a terminal optimizer rather than a data-preview command, add a solve
  subcommand backed by `src/solver/service.rs` and test its terminal score.

## MCP — deferred on this line

`solverforge new --shell mcp` is rejected with
`possible values: web, api, cli`. MCP support is being developed on a separate
branch and is not part of this release. Do not document it as available, do not
pass `--shell mcp`, and do not promise an MCP output. If a user needs a
programmatic interface today, use `api`.

## Choosing for the user

- They want to *see* it: `web`.
- They want to *integrate* it: `api`.
- They want a *command*: `cli`.
- They asked for MCP: explain it is upcoming, and offer `api` now.
