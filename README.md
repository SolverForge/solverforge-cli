# solverforge-cli

Default entry point for new SolverForge projects.

Use this CLI to scaffold, grow, and manage SolverForge applications. The generated
projects depend on the published `solverforge` crates and are meant to be extended
from here rather than hand-assembled from the runtime workspace.

```bash
cargo install solverforge-cli
solverforge new my-scheduler --standard
```

The CLI lives outside `solverforge-rs` and generates problem-type projects that depend on the published SolverForge crates.

Built-in scaffolds:

- `solverforge new <name> --standard`
- `solverforge new <name> --list`

The generated frontend is intentionally thin and composes shipped `solverforge-ui` primitives instead of vendoring template-specific web assets. Domain-specific examples such as employee scheduling and vehicle routing belong in quickstarts, not in the CLI's built-in scaffold catalog.

For solver and domain extension guidance after scaffolding, see the runtime docs
in [solverforge-rs](https://github.com/solverforge/solverforge-rs):
[Extend the solver](https://github.com/solverforge/solverforge-rs/blob/main/docs/extend-solver.md)
and [Extend the domain](https://github.com/solverforge/solverforge-rs/blob/main/docs/extend-domain.md).

## License

Licensed under the Apache License, Version 2.0. See [LICENSE](LICENSE).
