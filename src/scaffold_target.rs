pub const RUNTIME_TARGET_LABEL: &str = "solverforge 0.8.8";
pub const RUNTIME_TARGET_DISPLAY: &str = "SolverForge crate target 0.8.8";
pub const RUNTIME_SOURCE_PATH: &str = "crates.io: solverforge 0.8.8";
pub const UI_SOURCE_PATH: &str = "crates.io: solverforge-ui 0.4.3";
pub const MAPS_SOURCE_PATH: &str = "crates.io: solverforge-maps 2.1.3";

pub const LONG_VERSION_TEXT: &str = concat!(
    "solverforge-cli ",
    env!("CARGO_PKG_VERSION"),
    "\nCLI version: ",
    env!("CARGO_PKG_VERSION"),
    "\nScaffold runtime target: SolverForge crate target 0.8.8",
    "\nRuntime source: ",
    "crates.io: solverforge 0.8.8",
    "\nUI source: ",
    "crates.io: solverforge-ui 0.4.3",
    "\nMaps source: ",
    "crates.io: solverforge-maps 2.1.3",
);
