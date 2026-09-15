# solverforge-modeling agent skill

`solverforge-modeling/SKILL.md` is a portable [Agent
Skill](https://opencode.ai/docs/skills/) that teaches a coding agent how to turn
a described planning or optimization problem into a working SolverForge app with
the `solverforge` CLI: decompose it into facts, entities, and scalar/list
planning variables; choose the output shell (web / API / CLI); author hard and
soft constraints against the runtime stream API; generate demo data; and verify
a real solve end to end on web/API shells.

It is harness-agnostic: the same `SKILL.md` folder is discovered by opencode
(`.opencode/skills`, `~/.config/opencode/skills`), Claude Code (`.claude/skills`,
`~/.claude/skills`), and other Agent Skills harnesses (`.agents/skills`,
`~/.agents/skills`).

## Install

The installer copies the skill into each selected harness's **own** skills
directory. There is no symlink and no shared/central location, so every harness
gets an independent, self-contained copy.

```sh
# Default: cross-harness Agent Skills user scope (~/.agents/skills)
./scripts/install-skill

# Explicitly choose harnesses
./scripts/install-skill --only opencode
./scripts/install-skill --only claude
./scripts/install-skill --only agents

# Into a specific scaffolded app; each harness gets its own project directory
./scripts/install-skill --project ../my-scheduler

# Any other skills directory you want
./scripts/install-skill --dir ~/.config/some-harness/skills

# Inspect or remove
./scripts/install-skill --list
./scripts/install-skill --uninstall
```

| Harness | User scope | Project scope (`--project <dir>`) |
| --- | --- | --- |
| opencode | `~/.config/opencode/skills` | `<dir>/.opencode/skills` |
| Claude Code | `~/.claude/skills` | `<dir>/.claude/skills` |
| Agent Skills | `~/.agents/skills` | `<dir>/.agents/skills` |

The default uses only `~/.agents/skills`, which opencode and Codex discover.
Use `--only opencode` or `--only claude` for those harness-specific locations;
this avoids installing duplicate definitions into multiple directories scanned
by the same harness.

Restart the agent after installing so it rescans skill directories. From the
repo root, `make install-skill` runs the same script.

## What it covers

- The intake: asking the user for the problem, the output shell, and the hard and
  soft constraints before building.
- Scaffolding, then establishing the solution identity before adding model
  elements.
- Facts, entities, scalar variables (fact-collection and countable-range), and
  list variables (including the `cvrp` route profile).
- The constraint catalog, the `panic!`-stub problem, and the generated
  `Plan::<collection>()` localizing-source rule that makes joins, self-joins, and
  grouped streams run instead of panic.
- Demo data generation, compiler-owned files, and managed-block boundaries.
- Hard gate verification: `solverforge check`, `cargo check`, and a real web/API
  solve with the bundled `scripts/solve-smoke-test.sh`; CLI serialization is
  identified separately from solve verification.
- Advanced opt-ins: scalar hooks, list metadata, scalar groups, conflict repair,
  candidate traces.

The CLI's `README.md` and `solverforge --help` remain the authoritative command
contract; this skill is a playbook over them. Keep both in sync when the CLI
surface changes.
