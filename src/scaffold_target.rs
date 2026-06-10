pub const RUNTIME_CRATE_VERSION: &str = "0.15.1";
pub const UI_CRATE_VERSION: &str = "0.6.5";
pub const MAPS_CRATE_VERSION: &str = "2.1.4";

pub const RUNTIME_TARGET_LABEL: &str = "solverforge 0.15.1";
pub const UI_TARGET_LABEL: &str = "solverforge-ui 0.6.5";
pub const MAPS_TARGET_LABEL: &str = "solverforge-maps 2.1.4";
pub const RUNTIME_TARGET_DISPLAY: &str = "SolverForge crate target 0.15.1";
pub const RUNTIME_SOURCE_PATH: &str = "crates.io: solverforge 0.15.1";
pub const UI_SOURCE_PATH: &str = "crates.io: solverforge-ui 0.6.5";
pub const MAPS_SOURCE_PATH: &str = "crates.io: solverforge-maps 2.1.4";

pub const LONG_VERSION_TEXT: &str = concat!(
    "solverforge-cli ",
    env!("CARGO_PKG_VERSION"),
    "\nCLI version: ",
    env!("CARGO_PKG_VERSION"),
    "\nScaffold runtime target: SolverForge crate target 0.15.1",
    "\nScaffold UI target: solverforge-ui 0.6.5",
    "\nScaffold maps target: solverforge-maps 2.1.4",
    "\nRuntime source: ",
    "crates.io: solverforge 0.15.1",
    "\nUI source: ",
    "crates.io: solverforge-ui 0.6.5",
    "\nMaps source: ",
    "crates.io: solverforge-maps 2.1.4",
);
