//! Formats the Rust sources a CLI command writes with the user's `rustfmt`.
//!
//! Generators splice user-chosen names into code, so their raw output cannot
//! match rustfmt's line breaking and import ordering in every case. Running
//! the toolchain's own formatter over exactly the files a command changed
//! keeps a generated project `cargo fmt --check` clean without touching files
//! the command did not write. A missing or failing `rustfmt` never fails the
//! command: the file stays as generated.

use std::collections::HashMap;
use std::ffi::OsStr;
use std::fs;
use std::io::{ErrorKind, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::output;

const DEFAULT_EDITION: &str = "2021";

/// Contents of every `src/**/*.rs` file of a project before a command runs.
pub struct SourceSnapshot {
    root: PathBuf,
    files: HashMap<PathBuf, Vec<u8>>,
}

impl SourceSnapshot {
    /// Records the current Rust sources under `root/src`.
    pub fn capture(root: &Path) -> Self {
        let files = rust_sources(root)
            .into_iter()
            .filter_map(|path| fs::read(&path).ok().map(|raw| (path, raw)))
            .collect();
        Self {
            root: root.to_path_buf(),
            files,
        }
    }

    /// A snapshot with no files, so every source counts as changed.
    pub fn empty(root: &Path) -> Self {
        Self {
            root: root.to_path_buf(),
            files: HashMap::new(),
        }
    }

    /// Formats every source created or modified since the snapshot.
    pub fn format_changed(&self) {
        let program = std::env::var_os("RUSTFMT").unwrap_or_else(|| "rustfmt".into());
        self.format_changed_with(&program);
    }

    fn format_changed_with(&self, program: &OsStr) {
        let changed = rust_sources(&self.root)
            .into_iter()
            .filter(|path| match fs::read(path) {
                Ok(raw) => self.files.get(path) != Some(&raw),
                Err(_) => false,
            })
            .collect::<Vec<_>>();
        if changed.is_empty() {
            return;
        }

        let edition = project_edition(&self.root);
        for path in changed {
            match format_file(program, &self.root, &path, &edition) {
                Ok(()) => {}
                Err(FormatError::RustfmtUnavailable) => {
                    if output::is_verbose() {
                        output::print_dim("  rustfmt not found; generated files left unformatted");
                    }
                    return;
                }
                Err(FormatError::Failed(reason)) => {
                    output::print_skip(&format!(
                        "rustfmt {} ({reason})",
                        output::display_path(path.strip_prefix(&self.root).unwrap_or(&path))
                    ));
                }
            }
        }
    }
}

enum FormatError {
    RustfmtUnavailable,
    Failed(String),
}

fn format_file(
    program: &OsStr,
    root: &Path,
    path: &Path,
    edition: &str,
) -> Result<(), FormatError> {
    let source = fs::read(path).map_err(|err| FormatError::Failed(err.to_string()))?;
    let formatted = rustfmt(program, root, &source, edition)?;
    if formatted != source {
        fs::write(path, formatted).map_err(|err| FormatError::Failed(err.to_string()))?;
    }
    Ok(())
}

/// Pipes `source` through rustfmt. Stdin input keeps rustfmt from following
/// `mod` declarations into files the command did not write, and running from
/// the project root picks up the project's own `rustfmt.toml`.
fn rustfmt(
    program: &OsStr,
    root: &Path,
    source: &[u8],
    edition: &str,
) -> Result<Vec<u8>, FormatError> {
    let mut child = Command::new(program)
        .args(["--edition", edition, "--emit", "stdout"])
        .current_dir(root)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|err| match err.kind() {
            ErrorKind::NotFound | ErrorKind::PermissionDenied => FormatError::RustfmtUnavailable,
            _ => FormatError::Failed(err.to_string()),
        })?;

    let mut stdin = child.stdin.take().expect("rustfmt stdin is piped");
    let (write_result, output) = std::thread::scope(|scope| {
        let writer = scope.spawn(move || stdin.write_all(source));
        let output = child.wait_with_output();
        (writer.join(), output)
    });
    let output = output.map_err(|err| FormatError::Failed(err.to_string()))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let reason = stderr
            .lines()
            .map(str::trim)
            .find(|line| !line.is_empty())
            .unwrap_or("rustfmt failed")
            .to_string();
        return Err(FormatError::Failed(reason));
    }
    match write_result {
        Ok(Ok(())) => {}
        _ => {
            return Err(FormatError::Failed(
                "could not write to rustfmt".to_string(),
            ))
        }
    }
    if output.stdout.is_empty() && !source.is_empty() {
        return Err(FormatError::Failed(
            "rustfmt produced no output".to_string(),
        ));
    }
    Ok(output.stdout)
}

/// Reads `package.edition` from the project manifest, as `cargo fmt` would.
fn project_edition(root: &Path) -> String {
    fs::read_to_string(root.join("Cargo.toml"))
        .ok()
        .and_then(|raw| raw.parse::<toml::Table>().ok())
        .and_then(|manifest| {
            manifest
                .get("package")?
                .get("edition")?
                .as_str()
                .map(str::to_string)
        })
        .unwrap_or_else(|| DEFAULT_EDITION.to_string())
}

fn rust_sources(root: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    collect_rust_sources(&root.join("src"), &mut files);
    files.sort();
    files
}

fn collect_rust_sources(dir: &Path, files: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        if file_type.is_dir() {
            collect_rust_sources(&path, files);
        } else if file_type.is_file() && path.extension().is_some_and(|ext| ext == "rs") {
            files.push(path);
        }
    }
}

#[cfg(test)]
#[path = "source_format_tests.rs"]
mod tests;
