use std::fs;
use std::path::Path;

use crate::error::{CliError, CliResult};
use crate::output;
use crate::solver_config;
use toml_edit::{value, DocumentMut, Item, Table, TableLike, Value};

const CONFIG_PATH: &str = "solver.toml";

// Print the contents of solver.toml.
pub fn run_show() -> CliResult {
    let path = Path::new(CONFIG_PATH);
    if !path.exists() {
        return Err(CliError::NotInProject {
            missing: "solver.toml",
        });
    }

    let content = fs::read_to_string(path).map_err(|e| CliError::IoError {
        context: "failed to read solver.toml".to_string(),
        source: e,
    })?;

    output::print_status("config", CONFIG_PATH);
    println!();
    println!("{}", content.trim_end());
    println!();

    Ok(())
}

// Set a dotted key path in solver.toml to the given value.
pub fn run_set(key: &str, value: &str) -> CliResult {
    let path = Path::new(CONFIG_PATH);
    if !path.exists() {
        return Err(CliError::NotInProject {
            missing: "solver.toml",
        });
    }

    let content = fs::read_to_string(path).map_err(|e| CliError::IoError {
        context: "failed to read solver.toml".to_string(),
        source: e,
    })?;

    let segments = parse_key_segments(key)?;
    if segments
        .first()
        .is_some_and(|segment| segment.name == "phases")
    {
        return Err(CliError::with_hint(
            "cannot edit ordered solver.toml `phases` with `solverforge config set`",
            "edit solver.toml manually; `config set` is limited to non-phase scalar and table settings",
        ));
    }
    solver_config::validate_managed_blocks(&content)?;

    let mut doc: DocumentMut =
        content
            .parse::<DocumentMut>()
            .map_err(|e: toml_edit::TomlError| {
                CliError::general(format!("failed to parse solver.toml: {}", e))
            })?;

    set_toml_edit_segments(&mut doc, &segments, value)?;
    let new_content = doc.to_string();

    fs::write(path, &new_content).map_err(|e| CliError::IoError {
        context: "failed to write solver.toml".to_string(),
        source: e,
    })?;

    output::print_update(CONFIG_PATH);
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct KeySegment {
    name: String,
    index: Option<usize>,
}

// Navigate and set a dotted key path (e.g. "termination.seconds_spent_limit") in a toml::Value.
#[cfg(test)]
fn set_toml_key(doc: &mut toml::Value, key: &str, value: &str) -> CliResult {
    let segments = parse_key_segments(key)?;
    set_toml_segments(doc, &segments, value)
}

#[cfg(test)]
fn set_toml_segments(doc: &mut toml::Value, segments: &[KeySegment], value: &str) -> CliResult {
    let Some((segment, rest)) = segments.split_first() else {
        return Err(CliError::general("solver.toml key cannot be empty"));
    };

    if rest.is_empty() {
        let table = doc
            .as_table_mut()
            .ok_or_else(|| CliError::general("solver.toml root is not a TOML table"))?;
        if let Some(index) = segment.index {
            let array = table
                .get_mut(&segment.name)
                .and_then(toml::Value::as_array_mut)
                .ok_or_else(|| {
                    CliError::general(format!("solver.toml `{}` is not an array", segment.name))
                })?;
            let item = array.get_mut(index).ok_or_else(|| {
                CliError::general(format!(
                    "solver.toml `{}` has no index {}",
                    segment.name, index
                ))
            })?;
            *item = parse_toml_value(value);
        } else {
            table.insert(segment.name.clone(), parse_toml_value(value));
        }
        return Ok(());
    }

    let table = doc
        .as_table_mut()
        .ok_or_else(|| CliError::general("solver.toml root is not a TOML table"))?;

    let child = if let Some(index) = segment.index {
        let array = table
            .get_mut(&segment.name)
            .and_then(toml::Value::as_array_mut)
            .ok_or_else(|| {
                CliError::general(format!("solver.toml `{}` is not an array", segment.name))
            })?;
        array.get_mut(index).ok_or_else(|| {
            CliError::general(format!(
                "solver.toml `{}` has no index {}",
                segment.name, index
            ))
        })?
    } else {
        table
            .entry(segment.name.clone())
            .or_insert_with(|| toml::Value::Table(toml::map::Map::new()))
    };

    set_toml_segments(child, rest, value)
}

fn set_toml_edit_segments(
    doc: &mut DocumentMut,
    segments: &[KeySegment],
    raw_value: &str,
) -> CliResult {
    let Some((segment, rest)) = segments.split_first() else {
        return Err(CliError::general("solver.toml key cannot be empty"));
    };
    set_toml_edit_table(doc.as_table_mut(), segment, rest, raw_value)
}

fn set_toml_edit_table(
    table: &mut dyn TableLike,
    segment: &KeySegment,
    rest: &[KeySegment],
    raw_value: &str,
) -> CliResult {
    if rest.is_empty() {
        if let Some(index) = segment.index {
            let item = table.get_mut(&segment.name).ok_or_else(|| {
                CliError::general(format!("solver.toml `{}` is not an array", segment.name))
            })?;
            let array = item
                .as_value_mut()
                .and_then(Value::as_array_mut)
                .ok_or_else(|| {
                    CliError::general(format!("solver.toml `{}` is not an array", segment.name))
                })?;
            let slot = array.get_mut(index).ok_or_else(|| {
                CliError::general(format!(
                    "solver.toml `{}` has no index {}",
                    segment.name, index
                ))
            })?;
            *slot = parse_toml_edit_value(raw_value);
        } else {
            table.insert(&segment.name, parse_toml_edit_item(raw_value));
        }
        return Ok(());
    }

    let child_table = if let Some(index) = segment.index {
        let item = table.get_mut(&segment.name).ok_or_else(|| {
            CliError::general(format!("solver.toml `{}` is not an array", segment.name))
        })?;
        let array = item.as_array_of_tables_mut().ok_or_else(|| {
            CliError::general(format!("solver.toml `{}` is not an array", segment.name))
        })?;
        array.get_mut(index).ok_or_else(|| {
            CliError::general(format!(
                "solver.toml `{}` has no index {}",
                segment.name, index
            ))
        })?
    } else {
        if table.get(&segment.name).is_none() {
            table.insert(&segment.name, Item::Table(Table::new()));
        }
        table
            .get_mut(&segment.name)
            .and_then(Item::as_table_like_mut)
            .ok_or_else(|| {
                CliError::general(format!(
                    "solver.toml `{}` is not a TOML table",
                    segment.name
                ))
            })?
    };

    let Some((next, remaining)) = rest.split_first() else {
        return Err(CliError::general("solver.toml key cannot be empty"));
    };
    set_toml_edit_table(child_table, next, remaining, raw_value)
}

fn parse_key_segments(key: &str) -> CliResult<Vec<KeySegment>> {
    if key.trim().is_empty() {
        return Err(CliError::general("solver.toml key cannot be empty"));
    }
    key.split('.')
        .map(|segment| {
            if segment.is_empty() {
                return Err(CliError::general(format!(
                    "invalid solver.toml key path `{key}`"
                )));
            }
            parse_key_segment(segment)
        })
        .collect()
}

fn parse_key_segment(segment: &str) -> CliResult<KeySegment> {
    let Some(open) = segment.find('[') else {
        return Ok(KeySegment {
            name: segment.to_string(),
            index: None,
        });
    };
    let close = segment
        .strip_suffix(']')
        .and_then(|_| segment.rfind(']'))
        .ok_or_else(|| {
            CliError::general(format!(
                "invalid array segment `{segment}` in solver.toml key"
            ))
        })?;
    if close <= open + 1 || close + 1 != segment.len() {
        return Err(CliError::general(format!(
            "invalid array segment `{segment}` in solver.toml key"
        )));
    }
    let name = &segment[..open];
    if name.is_empty() {
        return Err(CliError::general(format!(
            "invalid array segment `{segment}` in solver.toml key"
        )));
    }
    let index = segment[open + 1..close].parse::<usize>().map_err(|_| {
        CliError::general(format!(
            "invalid array index in segment `{segment}` for solver.toml key"
        ))
    })?;
    Ok(KeySegment {
        name: name.to_string(),
        index: Some(index),
    })
}

// Parse a string into a toml::Value: try integer, then float, then bool, then string.
#[cfg(test)]
fn parse_toml_value(s: &str) -> toml::Value {
    if let Ok(i) = s.parse::<i64>() {
        return toml::Value::Integer(i);
    }
    if let Ok(f) = s.parse::<f64>() {
        return toml::Value::Float(f);
    }
    match s {
        "true" => return toml::Value::Boolean(true),
        "false" => return toml::Value::Boolean(false),
        _ => {}
    }
    toml::Value::String(s.to_string())
}

fn parse_toml_edit_item(s: &str) -> Item {
    Item::Value(parse_toml_edit_value(s))
}

fn parse_toml_edit_value(s: &str) -> Value {
    if let Ok(i) = s.parse::<i64>() {
        return Value::from(i);
    }
    if let Ok(f) = s.parse::<f64>() {
        return Value::from(f);
    }
    match s {
        "true" => return Value::from(true),
        "false" => return Value::from(false),
        _ => {}
    }
    value(s)
        .into_value()
        .expect("toml_edit scalar values should convert to values")
}

#[cfg(test)]
#[path = "config_tests.rs"]
mod tests;
