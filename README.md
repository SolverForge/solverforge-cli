# solverforge-cli

Standalone CLI for scaffolding and managing SolverForge projects.

```bash
cargo install solverforge-cli
solverforge new my-scheduler --standard
```

The CLI lives outside `solverforge-rs` and generates problem-type projects that depend on the published SolverForge crates.

Built-in scaffolds:

- `solverforge new <name> --standard`
- `solverforge new <name> --list`

The generated frontend is intentionally thin and composes shipped `solverforge-ui` primitives instead of vendoring template-specific web assets. Domain-specific examples such as employee scheduling and vehicle routing belong in quickstarts, not in the CLI's built-in scaffold catalog.

## License

Licensed under the Apache License, Version 2.0. See [LICENSE](LICENSE).
