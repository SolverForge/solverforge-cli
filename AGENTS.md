# Repository Guidelines

## Project Structure & Module Organization
`src/main.rs` defines the CLI entrypoint and Clap command tree. Command implementations live in `src/commands/`; larger generators use submodules such as `src/commands/generate_constraint/` and `src/commands/generate_domain/`. Shared support code sits in files like `src/error.rs`, `src/output.rs`, `src/rc.rs`, and `src/template.rs`. Integration tests live in `tests/`, and scaffold/template assets live in `templates/`.

## Build, Test, and Development Commands
- `cargo build`: compile the `solverforge` binary.
- `cargo run -- --help`: run the CLI locally and inspect commands.
- `cargo test`: run unit tests and integration tests that do not require external downloads.
- `cargo test -- --ignored`: run scaffold checks that invoke `cargo check` in temp projects; these need a full Rust toolchain and network access.
- `cargo fmt --all`: apply Rust formatting.
- `cargo clippy --all-targets -- -D warnings`: enforce lint-clean code.
- `pre-commit run --all-files`: run the repository hooks, including YAML checks, `gitleaks`, `fmt`, and `clippy`.

## Coding Style & Naming Conventions
Use standard Rust style with 4-space indentation and `rustfmt` output as the source of truth. Prefer `snake_case` for modules, files, functions, and test names; use `PascalCase` for types and enums. Keep CLI flags, generated file names, and module names descriptive and consistent with existing commands such as `generate_constraint` and `sf_config`.

## Testing Guidelines
Add narrow unit tests beside the code under `#[cfg(test)]` when validating parsing, rewriting, or template helpers. Put end-to-end CLI behavior in `tests/`, following existing names like `*_test.rs` and `test_*` functions. When changing scaffolding or generated code, cover both file creation and key output content assertions.

## Commit & Pull Request Guidelines
Follow the commit style already used in history: `fix: ...`, `refactor: ...`, `style: ...`, `chore: ...`. Keep commits scoped to one behavior change. PRs should explain user-visible CLI impact, list verification commands run, and link the relevant issue. Include concrete examples when flags, generated files, or template output change.

## Security & Configuration Tips
Do not commit generated projects, secrets, or credentials. The pre-commit config runs `gitleaks`; keep it enabled. When tests or generated scaffolds touch published SolverForge crate versions, verify `Cargo.toml` template changes and corresponding integration tests together.
