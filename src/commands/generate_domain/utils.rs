use std::path::Path;

use crate::error::{CliError, CliResult};

pub(crate) const KNOWN_SCORE_TYPES: &[&str] = &[
    "HardSoftScore",
    "HardSoftDecimalScore",
    "HardMediumSoftScore",
    "SoftScore",
    "BendableScore<N, M>",
];

/// Converts `snake_case` to `PascalCase`.
pub(crate) fn snake_to_pascal(name: &str) -> String {
    name.split('_')
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                None => String::new(),
                Some(c) => c.to_uppercase().collect::<String>() + chars.as_str(),
            }
        })
        .collect()
}

/// Simple English pluralization (covers common cases).
pub(crate) fn pluralize(name: &str) -> String {
    if name.ends_with('s')
        || name.ends_with('x')
        || name.ends_with('z')
        || name.ends_with("ch")
        || name.ends_with("sh")
    {
        format!("{}es", name)
    } else if name.ends_with('y')
        && !name.ends_with("ay")
        && !name.ends_with("ey")
        && !name.ends_with("iy")
        && !name.ends_with("oy")
        && !name.ends_with("uy")
    {
        format!("{}ies", &name[..name.len() - 1])
    } else {
        format!("{}s", name)
    }
}

pub(crate) fn validate_score_type(score: &str) -> CliResult {
    if is_supported_score_type(score) {
        Ok(())
    } else {
        Err(CliError::InvalidScoreType {
            score: score.to_string(),
            known: KNOWN_SCORE_TYPES,
        })
    }
}

pub(crate) fn is_soft_score(score: &str) -> bool {
    score.trim() == "SoftScore"
}

fn is_supported_score_type(score: &str) -> bool {
    let score = score.trim();
    matches!(
        score,
        "HardSoftScore" | "HardSoftDecimalScore" | "HardMediumSoftScore" | "SoftScore"
    ) || parse_bendable_score(score).is_some()
}

fn parse_bendable_score(score: &str) -> Option<(usize, usize)> {
    let inner = score
        .strip_prefix("BendableScore<")?
        .strip_suffix('>')?
        .trim();
    let mut parts = inner.split(',');
    let hard = parts.next()?.trim().parse::<usize>().ok()?;
    let soft = parts.next()?.trim().parse::<usize>().ok()?;
    if parts.next().is_some() || hard == 0 || soft == 0 {
        return None;
    }
    Some((hard, soft))
}

pub(crate) fn ensure_domain_dir(domain_dir: &Path) -> CliResult {
    if !domain_dir.exists() {
        return Err(CliError::NotInProject {
            missing: "src/domain/",
        });
    }
    Ok(())
}

/// Finds the `*.rs` file in `domain_dir` that contains `pub struct <TypeName>`.
pub(crate) fn find_file_for_type(
    domain_dir: &Path,
    type_name: &str,
) -> Result<std::path::PathBuf, String> {
    crate::commands::generate_constraint::domain::find_file_for_type(domain_dir, type_name)
}
