// ─── Utilities ────────────────────────────────────────────────────────────────

use std::fs;
use std::path::Path;
use owo_colors::OwoColorize;

pub(crate) const KNOWN_SCORE_TYPES: &[&str] = &[
    "HardSoftScore",
    "HardSoftDecimalScore",
    "HardMediumSoftScore",
    "SimpleScore",
    "BendableScore",
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
    if name.ends_with('s') || name.ends_with('x') || name.ends_with('z')
        || name.ends_with("ch") || name.ends_with("sh")
    {
        format!("{}es", name)
    } else if name.ends_with('y') && !name.ends_with("ay") && !name.ends_with("ey")
        && !name.ends_with("iy") && !name.ends_with("oy") && !name.ends_with("uy")
    {
        format!("{}ies", &name[..name.len() - 1])
    } else {
        format!("{}s", name)
    }
}

pub(crate) fn validate_score_type(score: &str) -> Result<(), String> {
    if KNOWN_SCORE_TYPES.contains(&score) {
        Ok(())
    } else {
        Err(format!(
            "unknown score type '{}'\n\nKnown score types: {}",
            score,
            KNOWN_SCORE_TYPES.join(", ")
        ))
    }
}

pub(crate) fn ensure_domain_dir(domain_dir: &Path) -> Result<(), String> {
    if !domain_dir.exists() {
        return Err("not a SolverForge project directory (src/domain/ not found)".to_string());
    }
    Ok(())
}

pub(crate) fn print_created(path: &str) {
    println!("{} Created {}", "▸".bright_green(), path.bright_cyan());
}

pub(crate) fn print_updated(path: &str) {
    println!("{} Updated {}", "▸".bright_green(), path.bright_cyan());
}

/// Finds the `*.rs` file in `domain_dir` that contains `pub struct <TypeName>`.
pub(crate) fn find_file_for_type(domain_dir: &Path, type_name: &str) -> Result<std::path::PathBuf, String> {
    let needle = format!("pub struct {}", type_name);
    let entries = fs::read_dir(domain_dir)
        .map_err(|e| format!("failed to read src/domain/: {}", e))?;

    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("rs") {
            continue;
        }
        if let Ok(src) = fs::read_to_string(&path) {
            if src.contains(&needle) {
                return Ok(path);
            }
        }
    }

    Err(format!("struct '{}' not found in src/domain/", type_name))
}
