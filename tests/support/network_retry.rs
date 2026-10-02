//! Network-tolerant execution for generated-app `cargo` commands.
//!
//! Scaffolded apps ship no `Cargo.lock`, so every `cargo check`/`cargo build`
//! in a generated app resolves its dependency graph against the live crates.io
//! index. Cargo retries that fetch three times internally, which is not enough
//! for a transient registry outage: a blip fails whichever test happens to be
//! building at that moment, so the failure set moves between runs and nothing
//! in the tested code is at fault. These helpers retry only the transport
//! failure and leave compile errors untouched, so a real defect still fails on
//! the first attempt.

use std::process::{Command, Output};
use std::thread;
use std::time::Duration;

const NETWORK_RETRY_ATTEMPTS: usize = 3;
const NETWORK_RETRY_DELAY: Duration = Duration::from_secs(5);

/// Markers that identify a registry transport failure rather than a compile
/// error. None of these appear when the dependency graph resolves offline.
const NETWORK_FAILURE_MARKERS: &[&str] = &[
    "spurious network error",
    "Could not connect to server",
    "failed to download from",
    "download of config.json failed",
    "unable to update registry",
    "failed to load source for dependency",
];

fn is_network_failure(stderr: &[u8]) -> bool {
    let text = String::from_utf8_lossy(stderr);
    NETWORK_FAILURE_MARKERS
        .iter()
        .any(|marker| text.contains(marker))
}

/// Runs `command` retrying only transient registry failures, and returns its
/// output. A compile error is returned immediately so failures caused by the
/// tested code stay deterministic.
pub fn output_with_network_retry(mut command: Command, label: &str) -> Output {
    let mut attempt = 1;
    loop {
        let output = command
            .output()
            .unwrap_or_else(|err| panic!("failed to run {label}: {err}"));

        if output.status.success()
            || attempt == NETWORK_RETRY_ATTEMPTS
            || !is_network_failure(&output.stderr)
        {
            return output;
        }

        eprintln!(
            "retrying {label} after a transient registry failure (attempt {attempt}/{NETWORK_RETRY_ATTEMPTS})"
        );
        thread::sleep(NETWORK_RETRY_DELAY);
        attempt += 1;
    }
}

/// Convenience wrapper for the common `cargo <args...>` shape used by the
/// generated-app harnesses.
pub fn cargo_with_network_retry(
    project_dir: &std::path::Path,
    args: &[&str],
    label: &str,
) -> Output {
    let mut command = Command::new("cargo");
    command.args(args).current_dir(project_dir);
    output_with_network_retry(command, label)
}
