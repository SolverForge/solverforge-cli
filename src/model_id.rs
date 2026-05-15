use std::fmt;

use crate::error::{CliError, CliResult};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct ConstraintId(String);

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct ScalarGroupId(String);

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct ConflictRepairId(ConstraintId);

impl ConstraintId {
    pub(crate) fn parse(raw: &str) -> CliResult<Self> {
        validate_canonical_id(raw, "constraint ID")?;
        Ok(Self(raw.to_string()))
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

impl ScalarGroupId {
    pub(crate) fn parse(raw: &str) -> CliResult<Self> {
        validate_canonical_id(raw, "scalar group ID")?;
        Ok(Self(raw.to_string()))
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

impl ConflictRepairId {
    pub(crate) fn parse(raw: &str) -> CliResult<Self> {
        Ok(Self(ConstraintId::parse(raw)?))
    }

    pub(crate) fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl fmt::Display for ConstraintId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl fmt::Display for ConflictRepairId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

fn validate_canonical_id(raw: &str, kind: &'static str) -> CliResult {
    let valid = raw.chars().enumerate().all(|(idx, ch)| {
        if idx == 0 {
            ch.is_ascii_lowercase()
        } else {
            ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '_'
        }
    });
    if !valid || raw.is_empty() {
        return Err(CliError::general(format!(
            "invalid {kind} '{raw}': use snake_case (lowercase letters, digits, underscores; must start with a letter)"
        )));
    }
    Ok(())
}
