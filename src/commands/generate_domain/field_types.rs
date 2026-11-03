//! `--field name:Type` handling shared by `generate fact` and `generate entity`.

use std::collections::BTreeSet;

use crate::error::{CliError, CliResult};

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
