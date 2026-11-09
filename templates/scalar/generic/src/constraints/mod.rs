//! Constraint definitions.
//!
//! Add constraint modules with `solverforge generate constraint ...`.
//! The neutral shell starts with an empty constraint set.

// @solverforge:begin constraint-modules
// @solverforge:end constraint-modules

mod assemble;

pub use assemble::create_constraints;
