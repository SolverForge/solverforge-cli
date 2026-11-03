//! `--field name:Type` handling shared by `generate fact` and `generate entity`.
//!
//! Generated domain files only import serde and the SolverForge prelude, so any
//! other type a user names in a `--field` must be resolved here: well-known std
//! collection and chrono types get `use` lines (and chrono gets a dependency),
//! and remaining bare type names are imported from the domain module.

use std::collections::BTreeSet;

use crate::error::{CliError, CliResult};

/// chrono release written into a generated `Cargo.toml` the first time a field
/// uses a chrono type; SolverForge itself does not depend on chrono.
pub(crate) const CHRONO_DEPENDENCY: &str = r#"{ version = "0.4.45", features = ["serde"] }"#;

const STD_COLLECTION_TYPES: &[&str] = &["BTreeMap", "BTreeSet", "HashMap", "HashSet", "VecDeque"];

const CHRONO_TYPES: &[&str] = &[
    "DateTime",
    "Duration",
    "NaiveDate",
    "NaiveDateTime",
    "NaiveTime",
    "Utc",
    "Weekday",
];

/// Names in scope without an import: primitives, the std prelude, and the
/// score types re-exported by `solverforge::prelude`.
const IN_SCOPE_TYPES: &[&str] = &[
    "bool",
    "char",
    "str",
    "u8",
    "u16",
    "u32",
    "u64",
    "u128",
    "usize",
    "i8",
    "i16",
    "i32",
    "i64",
    "i128",
    "isize",
    "f32",
    "f64",
    "String",
    "Vec",
    "Option",
    "Box",
    "Result",
    "HardSoftScore",
    "HardSoftDecimalScore",
    "HardMediumSoftScore",
    "SoftScore",
    "BendableScore",
    "dyn",
    "mut",
    "const",
];

/// Rejects `--field` names that collide with a field the generator always
/// writes (`builtins`) or with an earlier `--field`; either collision would
/// emit a struct with a duplicated field that cannot compile.
pub(crate) fn reject_duplicate_fields(
    kind: &str,
    fields: &[(String, String)],
    builtins: &[&str],
) -> CliResult {
    let mut seen = BTreeSet::new();
    for (name, _) in fields {
        if builtins.contains(&name.as_str()) {
            return Err(CliError::with_hint(
                format!("--field '{name}' duplicates the built-in '{name}' field that generate {kind} already writes"),
                format!(
                    "drop --field {name}:...; generate {kind} already declares {}",
                    builtins
                        .iter()
                        .map(|builtin| format!("'{builtin}'"))
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
            ));
        }
        if !seen.insert(name.as_str()) {
            return Err(CliError::general(format!(
                "--field '{name}' is given more than once"
            )));
        }
    }
    Ok(())
}

/// Bare (unqualified) type names referenced by a field type, in source order.
/// Path-qualified segments such as `chrono::NaiveDate` and lifetimes are
/// skipped because they need no import.
fn bare_type_names(ty: &str) -> Vec<String> {
    let chars: Vec<char> = ty.chars().collect();
    let mut names = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c.is_alphabetic() || c == '_' {
            let start = i;
            while i < chars.len() && (chars[i].is_alphanumeric() || chars[i] == '_') {
                i += 1;
            }
            let preceded_by_path = start >= 2 && chars[start - 1] == ':' && chars[start - 2] == ':';
            let preceded_by_lifetime = start >= 1 && chars[start - 1] == '\'';
            let followed_by_path = i + 1 < chars.len() && chars[i] == ':' && chars[i + 1] == ':';
            if !preceded_by_path && !preceded_by_lifetime && !followed_by_path {
                names.push(chars[start..i].iter().collect());
            }
        } else {
            i += 1;
        }
    }
    names
}

fn referenced_types(fields: &[(String, String)]) -> BTreeSet<String> {
    fields
        .iter()
        .flat_map(|(_, ty)| bare_type_names(ty))
        .collect()
}

/// Whether any field names a chrono type, so the app needs the chrono crate.
pub(crate) fn uses_chrono(fields: &[(String, String)]) -> bool {
    fields.iter().any(|(_, ty)| {
        ty.contains("chrono::")
            || bare_type_names(ty)
                .iter()
                .any(|name| CHRONO_TYPES.contains(&name.as_str()))
    })
}

/// Bare type names that are neither in scope nor well-known std/chrono types;
/// the generator imports them from the domain module.
pub(crate) fn domain_type_names(fields: &[(String, String)], own_type: &str) -> Vec<String> {
    referenced_types(fields)
        .into_iter()
        .filter(|name| {
            name != own_type
                && !IN_SCOPE_TYPES.contains(&name.as_str())
                && !STD_COLLECTION_TYPES.contains(&name.as_str())
                && !CHRONO_TYPES.contains(&name.as_str())
        })
        .collect()
}

/// `use` lines (each newline-terminated) that bring every field type into
/// scope for the generated domain file.
pub(crate) fn field_type_imports(fields: &[(String, String)], own_type: &str) -> String {
    let referenced = referenced_types(fields);
    let pick = |known: &[&str]| -> Vec<String> {
        known
            .iter()
            .filter(|name| referenced.contains(**name))
            .map(|name| name.to_string())
            .collect()
    };
    let mut imports = String::new();
    for (path, names) in [
        ("std::collections", pick(STD_COLLECTION_TYPES)),
        ("chrono", pick(CHRONO_TYPES)),
    ] {
        match names.as_slice() {
            [] => {}
            [single] => imports.push_str(&format!("use {path}::{single};\n")),
            many => imports.push_str(&format!("use {path}::{{{}}};\n", many.join(", "))),
        }
    }
    for name in domain_type_names(fields, own_type) {
        imports.push_str(&format!("use super::{name};\n"));
    }
    imports
}

/// Returns the rewritten `Cargo.toml` when `chrono` must be added, or `None`
/// when the manifest already declares it.
pub(crate) fn add_chrono_dependency(cargo_toml: &str) -> Result<Option<String>, String> {
    let mut doc: toml_edit::DocumentMut = cargo_toml
        .parse()
        .map_err(|err| format!("failed to parse Cargo.toml: {err}"))?;
    let deps = doc
        .get_mut("dependencies")
        .and_then(|item| item.as_table_like_mut())
        .ok_or_else(|| "Cargo.toml has no [dependencies] table".to_string())?;
    if deps.contains_key("chrono") {
        return Ok(None);
    }
    let value: toml_edit::Value = CHRONO_DEPENDENCY
        .parse()
        .map_err(|err| format!("invalid chrono dependency spec: {err}"))?;
    deps.insert("chrono", toml_edit::Item::Value(value));
    Ok(Some(doc.to_string()))
}
