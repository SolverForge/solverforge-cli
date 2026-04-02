pub const RUNTIME_TARGET_LABEL: &str = "local checkout (pre-0.7.0 release)";
pub const RUNTIME_TARGET_DISPLAY: &str = "SolverForge local checkout (pre-0.7.0 release)";
pub const RUNTIME_SOURCE_PATH: &str = "/srv/lab/dev/solverforge/solverforge-rs/crates/solverforge";
pub const UI_SOURCE_PATH: &str = "/srv/lab/dev/solverforge/solverforge-ui";

pub const LONG_VERSION_TEXT: &str = concat!(
    "solverforge-cli ",
    env!("CARGO_PKG_VERSION"),
    "\nCLI version: ",
    env!("CARGO_PKG_VERSION"),
    "\nScaffold runtime target: SolverForge local checkout (pre-0.7.0 release)",
    "\nRuntime source: ",
    "/srv/lab/dev/solverforge/solverforge-rs/crates/solverforge",
    "\nUI source: ",
    "/srv/lab/dev/solverforge/solverforge-ui",
);
