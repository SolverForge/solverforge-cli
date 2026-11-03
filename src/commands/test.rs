use std::process::Command;

use crate::error::{CliError, CliResult};
use crate::output;

/// Runs `cargo test <cargo_args> [-- <harness_args>]`.
///
/// `cargo_args` are the arguments the user wrote before `--` (test name
/// filters and cargo flags); `harness_args` are the arguments after `--`, which
/// cargo forwards to the test binaries (`--nocapture`, `--test-threads=1`).
pub fn run(cargo_args: &[String], harness_args: &[String]) -> CliResult {
    let args = cargo_test_args(cargo_args, harness_args);
    output::print_status("test", &format!("running cargo {}", args.join(" ")));

    let status = Command::new("cargo")
        .args(&args)
        .status()
        .map_err(|e| CliError::IoError {
            context: "failed to run cargo test".to_string(),
            source: e,
        })?;

    if status.success() {
        Ok(())
    } else {
        Err(CliError::SubprocessFailed {
            command: "cargo test".to_string(),
        })
    }
}

fn cargo_test_args(cargo_args: &[String], harness_args: &[String]) -> Vec<String> {
    let mut args = vec!["test".to_string()];
    args.extend(cargo_args.iter().cloned());
    if !harness_args.is_empty() {
        args.push("--".to_string());
        args.extend(harness_args.iter().cloned());
    }
    args
}

#[cfg(test)]
mod tests {
    use super::cargo_test_args;

    fn strings(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| value.to_string()).collect()
    }

    #[test]
    fn harness_args_follow_the_cargo_separator() {
        assert_eq!(
            cargo_test_args(&[], &strings(&["--nocapture"])),
            strings(&["test", "--", "--nocapture"])
        );
    }

    #[test]
    fn cargo_args_precede_the_separator() {
        assert_eq!(
            cargo_test_args(
                &strings(&["integration", "--release"]),
                &strings(&["--test-threads=1"])
            ),
            strings(&["test", "integration", "--release", "--", "--test-threads=1"])
        );
    }

    #[test]
    fn no_separator_without_harness_args() {
        assert_eq!(
            cargo_test_args(&strings(&["integration"]), &[]),
            strings(&["test", "integration"])
        );
    }
}
