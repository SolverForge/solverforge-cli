solverforge::planning_model! {
    root = "src/domain";

    // @solverforge:begin domain-exports
    mod plan;

    pub use plan::Plan;
    // @solverforge:end domain-exports
}
