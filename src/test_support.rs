use std::sync::{Mutex, MutexGuard, OnceLock};

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
