use crate::error::{CliError, CliResult};

pub(crate) fn validate_name(name: &str) -> CliResult {
    let valid = name.chars().enumerate().all(|(i, c)| {
        if i == 0 {
            c.is_ascii_lowercase()
        } else {
            c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_'
        }
    });
    if !valid || name.is_empty() {
        return Err(CliError::InvalidName {
            name: name.to_string(),
        });
    }
    Ok(())
}
