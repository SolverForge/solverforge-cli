//! Proves the network-retry boundary: a transient registry failure is retried,
//! and a compile error is returned immediately without retrying.
//!
//! The failure text used here is verbatim from a real crates.io outage
//! (`Updating crates.io index` / `spurious network error` / `unable to update
//! registry`), so the test asserts the helper recognizes the exact shape cargo
//! emits in practice rather than a hand-invented marker.
//!
//! Run with: cargo test --test network_retry_test

#[path = "support/network_retry.rs"]
mod network_retry;

use std::process::Command;
use std::time::{Duration, Instant};

use network_retry::{cargo_with_network_retry, output_with_network_retry};

/// The verbatim stderr shape of the outage that failed the scaffold suite.
const REAL_OUTAGE_STDERR: &str = "\
warning: spurious network error (3 tries remaining): [7] Could not connect to server (Failed to connect to index.crates.io:443 after 0 ms: Could not connect to server)
error: failed to get `axum` as a dependency of package `test_cargo_check_unified v0.1.0`
Caused by:
  failed to load source for dependency `axum`
Caused by:
  unable to update registry `crates-io`
Caused by:
  download of config.json failed
";

fn failing_command(stderr_text: &str) -> Command {
    let mut command = Command::new("sh");
    command.args([
        "-c",
        &format!("printf '%s' {} >&2; exit 101", sh_quote(stderr_text)),
    ]);
    command
}

fn sh_quote(text: &str) -> String {
    format!("'{}'", text.replace('\'', "'\\''"))
}

/// A registry transport failure must be retried, not returned on attempt one.
#[test]
fn retries_a_transient_registry_failure() {
    let started = Instant::now();
    let output =
        output_with_network_retry(failing_command(REAL_OUTAGE_STDERR), "probe cargo check");
    let elapsed = started.elapsed();

    assert!(
        !output.status.success(),
        "a failing command must not report success"
    );
    // Three attempts with a pause between them cannot fit in one attempt's time.
    assert!(
        elapsed >= Duration::from_secs(10),
        "network failures should be retried with a delay, elapsed: {elapsed:?}"
    );
}

/// A compile error must be returned on the first attempt: no retry, no delay.
#[test]
fn does_not_retry_a_compile_error() {
    let compile_stderr = "\
error[E0425]: cannot find value `missing` in this scope
 --> src/lib.rs:1:13
  |
1 | pub fn bad() { missing }
  |               ^^^^^^^ not found in this scope
";
    let started = Instant::now();
    let output = output_with_network_retry(failing_command(compile_stderr), "probe cargo check");
    let elapsed = started.elapsed();

    assert!(
        !output.status.success(),
        "a compile error must not report success"
    );
    assert!(
        elapsed < Duration::from_secs(5),
        "a compile error must not be retried, elapsed: {elapsed:?}"
    );
}

/// A successful command returns immediately regardless of stderr chatter.
#[test]
fn returns_success_without_retrying() {
    let mut command = Command::new("sh");
    command.args(["-c", "echo done; exit 0"]);
    let started = Instant::now();
    let output = output_with_network_retry(command, "probe success");
    assert!(output.status.success());
    assert!(started.elapsed() < Duration::from_secs(5));
}

/// The cargo wrapper runs a real cargo command in the given directory and
/// returns its output, so the harness call sites exercise the same path.
#[test]
fn cargo_wrapper_runs_a_real_cargo_command() {
    let tmp = tempfile::tempdir().expect("failed to create temp dir");
    std::fs::write(
        tmp.path().join("Cargo.toml"),
        "[package]\nname = \"wrapper_probe\"\nversion = \"0.0.0\"\nedition = \"2021\"\n",
    )
    .expect("failed to write manifest");
    std::fs::create_dir_all(tmp.path().join("src")).expect("failed to create src");
    std::fs::write(tmp.path().join("src/lib.rs"), "pub fn ok() -> u8 { 1 }\n")
        .expect("failed to write lib.rs");

    // `cargo metadata` resolves without a network fetch for a dependency-free
    // crate, so this stays fast and deterministic.
    let output = cargo_with_network_retry(tmp.path(), &["metadata", "--no-deps"], "probe metadata");
    assert!(
        output.status.success(),
        "wrapper should return the real cargo output: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        String::from_utf8_lossy(&output.stdout).contains("wrapper_probe"),
        "wrapper should pass through cargo stdout"
    );

    // A failing invocation returns its failure status rather than panicking.
    let failing = cargo_with_network_retry(tmp.path(), &["check", "--bogus-flag"], "probe failure");
    assert!(
        !failing.status.success(),
        "an invalid flag must not report success"
    );
}
