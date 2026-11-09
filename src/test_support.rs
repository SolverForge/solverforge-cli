use std::path::Path;
use std::sync::{Mutex, MutexGuard, OnceLock};

use crate::commands::generate_constraint::ConstraintSources;

static CWD_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

/// Serializes tests that change the process working directory.
///
/// A test that panics while holding the lock poisons it. Every holder restores
/// the directory through a `Drop` guard, which still runs during the unwind, so
/// the poison carries no broken state; taking the lock anyway keeps one failing
/// test from failing every later cwd test and hiding the real failure.
pub(crate) fn lock_cwd() -> MutexGuard<'static, ()> {
    CWD_LOCK
        .get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// The two managed constraint files: `mod.rs` declares `modules` (one
/// `mod name;` line each) inside the `constraint-modules` block, and
/// `assemble.rs` holds `calls` inside the `constraint-calls` block of
/// `create_constraints`.
pub(crate) fn constraint_sources(modules: &str, calls: &str) -> ConstraintSources {
    ConstraintSources {
        module: format!(
            "// @solverforge:begin constraint-modules\n{modules}// @solverforge:end constraint-modules\n\nmod assemble;\n\npub use assemble::create_constraints;\n"
        ),
        assemble: format!(
            "use crate::domain::Plan;\nuse solverforge::prelude::*;\n\npub fn create_constraints() -> impl ConstraintSet<Plan, HardSoftScore> {{\n    // @solverforge:begin constraint-calls\n{calls}    // @solverforge:end constraint-calls\n}}\n"
        ),
    }
}

/// Writes [`constraint_sources`] under `src/constraints/` of the current
/// directory.
pub(crate) fn write_constraint_sources(modules: &str, calls: &str) {
    std::fs::create_dir_all("src/constraints").expect("failed to create constraints dir");
    constraint_sources(modules, calls)
        .write(Path::new("."))
        .expect("failed to write constraint sources");
}
