use std::path::PathBuf;

use crate::error::CliResult;

// Persistent preferences loaded from `.solverforgerc` in home dir or project root.
// Project root takes precedence over home directory.
#[derive(Debug, Default)]
pub struct RcConfig {
    // Default server port
    pub port: Option<u16>,
    // Disable colored output
    pub no_color: bool,
    // Suppress all output except errors
    pub quiet: bool,
}

impl RcConfig {
    // Returns true if no preferences were set.
    #[cfg(test)]
    pub fn is_empty(&self) -> bool {
        self.port.is_none() && !self.no_color && !self.quiet
    }
}

// Locate the rc file to use: project root `.solverforgerc` takes precedence over `~/.solverforgerc`.
fn find_rc_path() -> Option<PathBuf> {
    let project_path = PathBuf::from(".solverforgerc");
    if project_path.exists() {
        return Some(project_path);
    }

    if let Some(home) = home_dir() {
        let home_path = home.join(".solverforgerc");
        if home_path.exists() {
            return Some(home_path);
        }
    }

    None
}

// Resolve the user home directory via the `HOME` environment variable.
fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME").map(PathBuf::from)
}

// Load `RcConfig` from the first `.solverforgerc` found (project root or home dir).
// Returns a default `RcConfig` if no file is found or parsing fails silently.
pub fn load_rc() -> CliResult<RcConfig> {
    let Some(path) = find_rc_path() else {
        return Ok(RcConfig::default());
    };

    let contents = match std::fs::read_to_string(&path) {
        Ok(s) => s,
        Err(_) => return Ok(RcConfig::default()),
    };

    parse_rc(&contents)
}

fn parse_rc(contents: &str) -> CliResult<RcConfig> {
    let table: toml::Table = match toml::from_str(contents) {
        Ok(t) => t,
        Err(_) => return Ok(RcConfig::default()),
    };

    let mut cfg = RcConfig::default();

    if let Some(toml::Value::Integer(n)) = table.get("port") {
        if *n > 0 && *n <= 65535 {
            cfg.port = Some(*n as u16);
        }
    }

    if let Some(toml::Value::Boolean(b)) = table.get("no_color") {
        cfg.no_color = *b;
    }

    if let Some(toml::Value::Boolean(b)) = table.get("quiet") {
        cfg.quiet = *b;
    }

    Ok(cfg)
}

#[cfg(test)]
#[path = "rc_tests.rs"]
mod tests;
