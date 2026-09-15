# Output shells

`--shell` is the "type of output" selector. It changes adapters, not the model:
the neutral scaffold is identical for every shell, and facts/entities/variables/
constraints behave the same. The current public set is exactly `web`, `api`,
`cli`, `mcp`.

| Shell | Frontend assets | HTTP surface | MCP tools | Job lifecycle | Typical use |
| --- | --- | --- | --- | --- | --- |
| `web` (default) | yes (`static/`, `solverforge-ui`) | Axum JSON/SSE | no | yes | end-to-end demo / UI |
| `api` | no | Axum JSON/SSE | no | yes | headless service, other clients |
| `mcp` | no | stateless Streamable HTTP at `/mcp` (with `--http`) | yes | yes | agent/LLM integration |
| `cli` | no | no | no | no | demo-data terminal scaffold; add solving yourself |

Recorded in `solverforge.app.toml` as `[app].shell`. API, CLI, and MCP specs omit
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
- Best choice when the consumer is an application over HTTP rather than an MCP
  client.

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

## mcp

```bash
solverforge new agent-scheduler --shell mcp
cd agent-scheduler
cargo run --release              # stdio MCP server (default transport)
cargo run --release -- --http    # Streamable HTTP at http://127.0.0.1:7860/mcp
solverforge connect              # ready-to-paste client configs
```

- Emits `src/mcp/` (tools, task wiring, progress) plus the shared core, and the
  `rmcp` dependency. No Axum REST routes, no SSE job routes, no `static/`.
- The solve lifecycle is exposed as schema-typed tools: `list_demo_data`,
  `get_demo_data`, `solve`, `get_status`, `get_best_solution`,
  `analyze_solution`, `get_telemetry`, `get_candidate_trace`, `pause`, `resume`,
  `cancel`, `delete`.
- `solve` is task-backed for task-capable clients (MCP 2026-07-28), returning the
  retained `jobId` in result metadata, and an immediate summary to others. Tasks
  have no TTL and live for the server process lifetime.
- `solverforge server` boots the mcp project's HTTP transport for you; the raw
  `cargo run --release` default is stdio. `solverforge routes`/`/health`/`/jobs`
  do not exist here.
- HTTP binds loopback unless `--host` selects a concrete IP; wildcard binds are
  rejected. `solverforge connect` prints client config and
  `--write opencode|claude|cursor|vscode` merges the in-project config for that
  harness.
- The runtime `console` feature stays off because the banner would corrupt the
  stdio channel; diagnostics go to stderr.

## Choosing for the user

- They want to *see* it: `web`.
- They want to *integrate* an HTTP service: `api`.
- Their consumer is an agent/LLM harness: `mcp`.
- They want a *command*: `cli`.
