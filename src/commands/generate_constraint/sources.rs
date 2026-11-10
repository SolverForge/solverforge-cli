//! The two files that hold a project's managed constraint wiring.
//!
//! `src/constraints/mod.rs` declares the constraint modules inside the
//! `constraint-modules` block and re-exports `create_constraints`; it holds no
//! code. `src/constraints/assemble.rs` defines `create_constraints` and owns
//! the `constraint-calls` block that lists every constraint in evaluation
//! order.

use std::fs;
use std::path::Path;

use crate::error::{CliError, CliResult};

/// Project-relative path of the constraint module manifest.
pub(crate) const CONSTRAINTS_MOD_PATH: &str = "src/constraints/mod.rs";
/// Project-relative path of the constraint set assembly.
pub(crate) const CONSTRAINTS_ASSEMBLE_PATH: &str = "src/constraints/assemble.rs";

/// The constraint module manifest and the constraint set assembly, as text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ConstraintSources {
    /// Contents of `src/constraints/mod.rs`.
    pub(crate) module: String,
    /// Contents of `src/constraints/assemble.rs`.
    pub(crate) assemble: String,
}

impl ConstraintSources {
    /// Reads both files relative to `root`.
    pub(crate) fn read(root: &Path) -> CliResult<Self> {
        Ok(Self {
            module: read(root, CONSTRAINTS_MOD_PATH)?,
            assemble: read(root, CONSTRAINTS_ASSEMBLE_PATH)?,
        })
    }

    /// Writes both files relative to `root`.
    pub(crate) fn write(&self, root: &Path) -> CliResult {
        write(root, CONSTRAINTS_MOD_PATH, &self.module)?;
        write(root, CONSTRAINTS_ASSEMBLE_PATH, &self.assemble)
    }

    /// Sources of an empty constraint set for `solution_type` scored by
    /// `score_type`.
    pub(crate) fn empty(solution_type: &str, score_type: &str) -> Self {
        Self {
            module: EMPTY_CONSTRAINTS_MOD.to_string(),
            assemble: format!(
                r#"//! Assembles the project's constraint set.

use crate::domain::{solution_type};
use solverforge::prelude::*;

/// Builds the constraint set the solver scores every solution against.
pub fn create_constraints() -> impl ConstraintSet<{solution_type}, {score_type}> {{
    // @solverforge:begin constraint-calls
    ()
    // @solverforge:end constraint-calls
}}
"#
            ),
        }
    }
}

const EMPTY_CONSTRAINTS_MOD: &str = r#"//! Constraint definitions.
//!
//! Add constraint modules with `solverforge generate constraint ...`.
//! The neutral shell starts with an empty constraint set.

// @solverforge:begin constraint-modules
// @solverforge:end constraint-modules

mod assemble;

pub use assemble::create_constraints;
"#;

fn read(root: &Path, relative: &'static str) -> CliResult<String> {
    let path = root.join(relative);
    if !path.exists() {
        return Err(CliError::NotInProject { missing: relative });
    }
    fs::read_to_string(&path).map_err(|source| CliError::IoError {
        context: format!("failed to read {relative}"),
        source,
    })
}

fn write(root: &Path, relative: &str, contents: &str) -> CliResult {
    fs::write(root.join(relative), contents).map_err(|source| CliError::IoError {
        context: format!("failed to write {relative}"),
        source,
    })
}
