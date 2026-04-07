pub const RUNTIME_TARGET_LABEL: &str = "solverforge 0.7.1";
pub const RUNTIME_TARGET_DISPLAY: &str = "SolverForge crates.io release 0.7.1";
pub const RUNTIME_SOURCE_PATH: &str = "crates.io: solverforge 0.7.1";
pub const UI_SOURCE_PATH: &str = "crates.io: solverforge-ui 0.4.2";

pub const LONG_VERSION_TEXT: &str = concat!(
    "solverforge-cli ",
    env!("CARGO_PKG_VERSION"),
    "\nCLI version: ",
    env!("CARGO_PKG_VERSION"),
    "\nScaffold runtime target: SolverForge crates.io release 0.7.1",
    "\nRuntime source: ",
    "crates.io: solverforge 0.7.1",
    "\nUI source: ",
    "crates.io: solverforge-ui 0.4.2",
);
