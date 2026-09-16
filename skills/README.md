# SolverForge agent skills

This repository bundles the portable, harness-agnostic SolverForge agent skills:

- `solverforge-modeling/` — turn a described planning or optimization problem
  into a working `solverforge` app with the CLI: facts, entities, scalar/list
  planning variables, hard/soft constraints, demo data, output shell
  (`web` / `api` / `cli` / `mcp`), routing with `solverforge-maps`, and
  behavioral verification.
- `solverforge-ui/` — extend a generated web shell into a domain-faithful UI
  with the shipped `SF.*` components, preserving the generated model and the
  `/sf` + `/jobs` + `/demo-data` contract.

Both are plain directory trees: `SKILL.md`, `references/`, and any bundled
`scripts/`. The same folder is discovered by opencode, Claude Code, Codex, and
other Agent Skills harnesses.

## Install

The installer is agent-centric: you name the harnesses you use and it resolves
each harness's own skills directory. It never installs into a directory you did
not ask for, never assumes `~/.agents`, and refuses to create duplicate
discovery.

```sh
# One copy per harness you use (the default)
./scripts/install-skill --agent opencode
./scripts/install-skill --agent claude
./scripts/install-skill --agent codex

# Two harnesses that overlap: a duplicate-free shared placement
./scripts/install-skill --agent opencode --agent claude --layout covering

# Symlink the skills instead of copying them
./scripts/install-skill --agent opencode --link

# A project instead of user scope
./scripts/install-skill --agent opencode --project ../my-app

# Any explicit skills directory
./scripts/install-skill --dir ~/.config/some-harness/skills

# Inspect or remove
./scripts/install-skill --agent opencode --list
./scripts/install-skill --agent opencode --uninstall
```

Harness directories:

| Harness | User scope | Project scope (`--project <dir>`) | Scanned by |
| --- | --- | --- | --- |
| opencode | `~/.config/opencode/skills` | `<dir>/.opencode/skills` | opencode |
| Claude Code | `~/.claude/skills` | `<dir>/.claude/skills` | opencode, Claude Code |
| Codex / Agent Skills | `~/.agents/skills` | `<dir>/.agents/skills` | opencode, Codex |

Because opencode scans all three directories, per-harness copies for
`{opencode, claude}`, `{opencode, codex}`, or `{opencode, claude, codex}` would
make opencode discover the same skill more than once. Use `--layout covering`
for the two-harness selections; it places one shared copy where possible. The
`{opencode, claude, codex}` combination has no duplicate-free placement, so the
installer reports it and asks you to choose two, or to pass
`--layout per-harness --force` and accept duplicate discovery.

Installed copies carry a `.solverforge-skill` marker that the installer writes;
symlinked installs carry a sidecar receipt, and the link is treated as
installer-owned only while it still points at the receipt's recorded target. A
skill copied by hand has no marker, so the installer treats it as foreign: it
updates or removes only entries it owns, and leaves foreign files, directories,
and symlinks untouched.

Restart the agent after installing so it rescans skill directories.

## What the skills cover

- The intake: ask for the problem, the output shell, and the hard/soft
  constraints before building.
- Scaffolding, then establishing the solution identity before adding model
  elements.
- Facts, entities, scalar variables (fact-collection and countable-range), and
  list variables for any ordered sequence (including the `cvrp` route profile).
- The constraint catalog and the generated `Plan::<collection>()` localizing
  source rule that makes joins, self-joins, and grouped streams run.
- Output shells `web`, `api`, `cli`, and `mcp`; `solverforge connect --write`
  registers an MCP server in opencode, Claude Code, Cursor, or VS Code.
- Routing with `solverforge-maps`: road networks, travel-time matrices,
  `prepare_routing`, distance/hook wiring, route geometry
  (`references/routing-and-maps.md`).
- Demo data, compiler-owned files, and managed-block boundaries.
- Verification: `solverforge check`, `cargo check`, a real web/API solve with
  `scripts/solve-smoke-test.sh`, MCP tool calls through the registered harness,
  and CLI serialization.

The CLI's `README.md` and `solverforge --help` remain the authoritative command
contract; these skills are playbooks over it. Keep them in sync when the CLI or
`solverforge-ui` surface changes.
