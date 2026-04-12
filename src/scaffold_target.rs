pub const RUNTIME_TARGET_LABEL: &str = "solverforge 0.8.3";
pub const RUNTIME_TARGET_DISPLAY: &str = "SolverForge crate target 0.8.3";
pub const RUNTIME_SOURCE_PATH: &str = "crates.io: solverforge 0.8.3";
pub const UI_SOURCE_PATH: &str = "crates.io: solverforge-ui 0.4.2";

pub const LONG_VERSION_TEXT: &str = concat!(
    "solverforge-cli ",
    env!("CARGO_PKG_VERSION"),
    "\nCLI version: ",
    env!("CARGO_PKG_VERSION"),
    "\nScaffold runtime target: SolverForge crate target 0.8.3",
    "\nRuntime source: ",
    "crates.io: solverforge 0.8.3",
    "\nUI source: ",
    "crates.io: solverforge-ui 0.4.2",
);
