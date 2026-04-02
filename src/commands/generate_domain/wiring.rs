use std::fs;
use std::path::Path;

use super::utils::{find_file_for_type, pluralize};
use crate::commands::generate_constraint::parse_domain;
use crate::output;

/// Adds `mod <name>; pub use <name>::<Pascal>;` to `src/domain/mod.rs`.
pub(crate) fn update_domain_mod(mod_name: &str, pascal: &str) -> Result<(), String> {
    let mod_path = Path::new("src/domain/mod.rs");
    if !mod_path.exists() {
        return Ok(()); // nothing to wire
    }

    let src = fs::read_to_string(mod_path)
        .map_err(|e| format!("failed to read src/domain/mod.rs: {}", e))?;

    let mod_line = format!("mod {};", mod_name);
    let use_line = format!("pub use {}::{};", mod_name, pascal);

    if src.contains(&mod_line) {
        return Ok(()); // already present
    }

    let new_src = format!("{}\n{}\n{}\n", src.trim_end(), mod_line, use_line);
    fs::write(mod_path, new_src).map_err(|e| format!("failed to write src/domain/mod.rs: {}", e))
}

/// Inserts a `#[annotation] pub <plural>: Vec<Type>` field into the solution struct,
/// adds a `use super::Type;` import, and updates the `new()` constructor.
pub(crate) fn wire_collection_into_solution(
    name: &str,
    pascal: &str,
    annotation: &str,
) -> Result<(), String> {
    let domain = parse_domain();
    let solution_type = match &domain {
        Some(d) => d.solution_type.clone(),
        None => return Ok(()), // no solution yet, nothing to wire
    };

    let domain_dir = Path::new("src/domain");
    let solution_file = match find_file_for_type(domain_dir, &solution_type) {
        Ok(f) => f,
        Err(_) => return Ok(()),
    };

    let src = fs::read_to_string(&solution_file)
        .map_err(|e| format!("failed to read {}: {}", solution_file.display(), e))?;

    let plural = pluralize(name);
    let field_block = format!(
        "    #[{}]\n    pub {}: Vec<{}>,",
        annotation, plural, pascal
    );

    // Skip if already wired
    if src.contains(&format!("pub {}: Vec<{}>", plural, pascal)) {
        return Ok(());
    }

    let new_src = insert_field_and_import(&src, &solution_type, pascal, &plural, &field_block)?;
    fs::write(&solution_file, new_src)
        .map_err(|e| format!("failed to write {}: {}", solution_file.display(), e))?;

    output::print_update(solution_file.to_str().unwrap());
    Ok(())
}

/// Inserts the field block before the `#[planning_score]` field (or before the closing `}`),
/// adds the import, and patches the `new()` constructor.
pub(crate) fn insert_field_and_import(
    src: &str,
    solution_type: &str,
    pascal: &str,
    _plural: &str,
    field_block: &str,
) -> Result<String, String> {
    // 1. Add import after last `use` line (or at top of file)
    let src = add_import(src, &format!("use super::{};", pascal));

    // 2. Insert field into struct before `#[planning_score]` or before last field's `}`
    let src = insert_struct_field(&src, solution_type, field_block)?;

    rebuild_solution_constructor(&src)
}

fn rebuild_solution_constructor(src: &str) -> Result<String, String> {
    let collection_fields: Vec<(String, String)> =
        src.lines().filter_map(parse_solution_vec_field).collect();
    let new_signature = if collection_fields.is_empty() {
        "    pub fn new() -> Self {".to_string()
    } else {
        format!(
            "    pub fn new({}) -> Self {{",
            collection_fields
                .iter()
                .map(|(field, ty)| format!("{field}: Vec<{ty}>"))
                .collect::<Vec<_>>()
                .join(", ")
        )
    };
    let new_body = if collection_fields.is_empty() {
        "        Self { score: None }".to_string()
    } else {
        format!(
            "        Self {{ {}, score: None }}",
            collection_fields
                .iter()
                .map(|(field, _)| format!("{field}: {field}"))
                .collect::<Vec<_>>()
                .join(", ")
        )
    };

    let lines: Vec<String> = src.lines().map(|line| line.to_string()).collect();
    let fn_start = lines
        .iter()
        .position(|line| line.contains("pub fn new("))
        .ok_or_else(|| "could not find solution constructor".to_string())?;
    let mut fn_end = None;
    let mut depth = 0i32;
    for (idx, line) in lines.iter().enumerate().skip(fn_start) {
        depth += line.chars().filter(|&c| c == '{').count() as i32;
        depth -= line.chars().filter(|&c| c == '}').count() as i32;
        if idx > fn_start && depth == 0 {
            fn_end = Some(idx);
            break;
        }
    }
    let fn_end = fn_end.ok_or_else(|| "could not find constructor end".to_string())?;

    let mut out = Vec::new();
    out.extend(lines[..fn_start].iter().cloned());
    out.push(new_signature);
    out.push(new_body);
    out.push("    }".to_string());
    out.extend(lines[fn_end + 1..].iter().cloned());
    Ok(out.join("\n") + "\n")
}

fn parse_solution_vec_field(line: &str) -> Option<(String, String)> {
    let trimmed = line.trim();
    if !trimmed.starts_with("pub ") || trimmed.contains("score:") {
        return None;
    }
    let trimmed = trimmed.trim_start_matches("pub ").trim_end_matches(',');
    let colon = trimmed.find(':')?;
    let field = trimmed[..colon].trim().to_string();
    let type_part = trimmed[colon + 1..].trim();
    if type_part.starts_with("Vec<") && type_part.ends_with('>') {
        Some((field, type_part[4..type_part.len() - 1].to_string()))
    } else {
        None
    }
}

pub(crate) fn add_import(src: &str, import: &str) -> String {
    if src.contains(import) {
        return src.to_string();
    }
    // Insert after the last `use ` line
    let mut lines: Vec<&str> = src.lines().collect();
    let last_use = lines
        .iter()
        .rposition(|l| l.trim_start().starts_with("use "));
    let insert_at = last_use.map(|i| i + 1).unwrap_or(0);
    lines.insert(insert_at, import);
    lines.join("\n") + "\n"
}

pub(crate) fn insert_struct_field(
    src: &str,
    _solution_type: &str,
    field_block: &str,
) -> Result<String, String> {
    // Insert before `#[planning_score]` if it exists, else before the last `}` of a struct block
    if let Some(pos) = src.find("    #[planning_score]") {
        let mut result = src.to_string();
        result.insert_str(pos, &format!("{}\n", field_block));
        return Ok(result);
    }

    // Find the closing `}` of the struct definition by locating `pub struct` and tracking brace depth
    let lines: Vec<&str> = src.lines().collect();
    let struct_line = lines.iter().position(|l| l.contains("pub struct "));
    if let Some(start) = struct_line {
        let mut depth = 0;
        let mut struct_close = None;
        for (i, line) in lines.iter().enumerate().skip(start) {
            for ch in line.chars() {
                match ch {
                    '{' => depth += 1,
                    '}' => {
                        depth -= 1;
                        if depth == 0 {
                            struct_close = Some(i);
                            break;
                        }
                    }
                    _ => {}
                }
            }
            if struct_close.is_some() {
                break;
            }
        }
        if let Some(i) = struct_close {
            let mut result_lines = lines.to_vec();
            let field_lines: Vec<&str> = field_block.lines().collect();
            for (j, fl) in field_lines.iter().enumerate() {
                result_lines.insert(i + j, fl);
            }
            return Ok(result_lines.join("\n") + "\n");
        }
    }

    Err("could not find insertion point in solution struct".to_string())
}

#[cfg(test)]
/// Adds a `#[planning_variable]` field to an existing entity struct and patches `new()`.
pub(crate) fn inject_planning_variable(
    src: &str,
    entity: &str,
    field: &str,
) -> Result<String, String> {
    inject_standard_variable(src, entity, field, "", true)
}

pub(crate) fn inject_standard_variable(
    src: &str,
    entity: &str,
    field: &str,
    range: &str,
    allows_unassigned: bool,
) -> Result<String, String> {
    let annotation = if range.is_empty() {
        format!(
            "    #[planning_variable(allows_unassigned = {})]",
            allows_unassigned
        )
    } else {
        format!(
            "    #[planning_variable(value_range = \"{}\", allows_unassigned = {})]",
            range, allows_unassigned
        )
    };
    let field_block = format!("{}\n    pub {}: Option<usize>,", annotation, field);
    if src.contains(&format!("pub {}: Option<usize>", field)) {
        return Err(format!("field '{}' already exists in {}", field, entity));
    }

    // Insert before closing `}` of the struct
    let src = insert_struct_field(src, entity, &field_block)?;

    // Patch new(): add `, <field>: None` to Self { ... }
    // Actually patch Self init with `field: None`
    let field_none = format!("{}: None,", field);
    let src = if src.contains(&field_none) {
        src
    } else {
        add_self_none_init(&src, field)
    };

    Ok(src)
}

pub(crate) fn add_self_none_init(src: &str, field: &str) -> String {
    let field_init = format!("{}: None,", field);
    if src.contains(&field_init) {
        return src.to_string();
    }

    // Find `Self {` that is a struct literal (has content after `{` on the same line),
    // not a block opener like `-> Self {` (where `{` is at end of line).
    let self_pos = src
        .match_indices("Self {")
        .find(|(pos, _)| {
            let after_brace = &src[pos + "Self {".len()..];
            // It's a struct literal if the char immediately after `{` is not `\n` or `\r`
            after_brace
                .chars()
                .next()
                .map(|c| c != '\n' && c != '\r')
                .unwrap_or(false)
        })
        .map(|(pos, _)| pos);

    if let Some(self_pos) = self_pos {
        let after_self = &src[self_pos..];
        let mut depth = 0;
        let mut close_pos = None;
        for (i, ch) in after_self.char_indices() {
            match ch {
                '{' => depth += 1,
                '}' => {
                    depth -= 1;
                    if depth == 0 {
                        close_pos = Some(self_pos + i);
                        break;
                    }
                }
                _ => {}
            }
        }
        if let Some(close) = close_pos {
            let indent = "            ";
            // Check if we need to add a comma to the previous field
            let before_close = &src[..close];
            let trimmed = before_close.trim_end();
            let needs_comma = !trimmed.ends_with(',') && !trimmed.ends_with('{');

            if needs_comma {
                // Find where to add the comma (right before any whitespace at the end)
                let content_end = before_close.trim_end().len();
                let with_comma = format!("{},", &src[..content_end]);
                return format!(
                    "{}\n{}{}: None,\n{}",
                    with_comma,
                    indent,
                    field,
                    &src[close..]
                );
            } else {
                return format!(
                    "{}{}{}: None,\n{}",
                    &src[..close],
                    indent,
                    field,
                    &src[close..]
                );
            }
        }
    }
    src.to_string()
}

pub(crate) fn inject_list_variable(
    src: &str,
    entity: &str,
    field: &str,
    elements: &str,
) -> Result<String, String> {
    let field_block = format!(
        "    #[planning_list_variable(element_collection = \"{}\")]\n    pub {}: Vec<usize>,",
        elements, field
    );
    if src.contains(&format!("pub {}: Vec<usize>", field)) {
        return Err(format!("field '{}' already exists in {}", field, entity));
    }

    let src = insert_struct_field(src, entity, &field_block)?;
    let field_init = format!("{}: Vec::new(),", field);
    let src = if src.contains(&field_init) {
        src
    } else {
        add_self_vec_init(&src, field)
    };

    Ok(src)
}

pub(crate) fn remove_variable_field(src: &str, field: &str) -> Result<String, String> {
    if !src.contains(&format!("pub {}", field)) {
        return Err(format!("field '{}' not found", field));
    }

    let mut lines: Vec<String> = src.lines().map(|line| line.to_string()).collect();
    let mut i = 0;
    while i < lines.len() {
        let trimmed = lines[i].trim().to_string();
        if trimmed.starts_with(&format!("pub {}:", field)) {
            let mut start = i;
            while start > 0 && lines[start - 1].trim_start().starts_with("#[") {
                start -= 1;
            }
            lines.drain(start..=i);
            i = start;
            continue;
        }
        if trimmed.contains(&format!("{}: None,", field))
            || trimmed.contains(&format!("{}: Vec::new(),", field))
        {
            lines.remove(i);
            continue;
        }
        i += 1;
    }

    Ok(lines.join("\n") + "\n")
}

fn add_self_vec_init(src: &str, field: &str) -> String {
    let field_init = format!("{}: Vec::new(),", field);
    if src.contains(&field_init) {
        return src.to_string();
    }

    let self_pos = src
        .match_indices("Self {")
        .find(|(pos, _)| {
            let after_brace = &src[pos + "Self {".len()..];
            after_brace
                .chars()
                .next()
                .map(|c| c != '\n' && c != '\r')
                .unwrap_or(false)
        })
        .map(|(pos, _)| pos);

    if let Some(self_pos) = self_pos {
        let after_self = &src[self_pos..];
        let mut depth = 0;
        let mut close_pos = None;
        for (i, ch) in after_self.char_indices() {
            match ch {
                '{' => depth += 1,
                '}' => {
                    depth -= 1;
                    if depth == 0 {
                        close_pos = Some(self_pos + i);
                        break;
                    }
                }
                _ => {}
            }
        }
        if let Some(close) = close_pos {
            let indent = "            ";
            let before_close = &src[..close];
            let trimmed = before_close.trim_end();
            let needs_comma = !trimmed.ends_with(',') && !trimmed.ends_with('{');
            if needs_comma {
                let content_end = before_close.trim_end().len();
                let with_comma = format!("{},", &src[..content_end]);
                return format!(
                    "{}\n{}{}: Vec::new(),\n{}",
                    with_comma,
                    indent,
                    field,
                    &src[close..]
                );
            }
            return format!(
                "{}{}{}: Vec::new(),\n{}",
                &src[..close],
                indent,
                field,
                &src[close..]
            );
        }
    }

    src.to_string()
}

/// Replaces the score type in the solution file.
pub(crate) fn replace_score_type(
    src: &str,
    old_score: &str,
    new_score: &str,
) -> Result<String, String> {
    if !src.contains(old_score) {
        return Err(format!(
            "score type '{}' not found in solution file",
            old_score
        ));
    }
    Ok(src.replace(old_score, new_score))
}
