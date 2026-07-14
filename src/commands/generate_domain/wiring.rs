use std::collections::{BTreeMap, BTreeSet};

use crate::list_variable_metadata::ListVariableMetadata;
use crate::managed_block;
use crate::scalar_variable_hooks::ScalarVariableHooks;

const DOMAIN_EXPORTS_BLOCK: &str = "domain-exports";
const SOLUTION_IMPORTS_BLOCK: &str = "solution-imports";
const SOLUTION_COLLECTIONS_BLOCK: &str = "solution-collections";
const SOLUTION_CONSTRUCTOR_PARAMS_BLOCK: &str = "solution-constructor-params";
const SOLUTION_CONSTRUCTOR_INIT_BLOCK: &str = "solution-constructor-init";
const ENTITY_VARIABLES_BLOCK: &str = "entity-variables";
const ENTITY_VARIABLE_INIT_BLOCK: &str = "entity-variable-init";

type DomainUseMap = BTreeMap<String, Vec<String>>;

#[derive(Debug, Clone)]
struct DomainExportSurface {
    module_names: Vec<String>,
    use_map: DomainUseMap,
}

#[derive(Debug, Clone)]
struct ManagedFieldEntry {
    field_name: String,
    lines: Vec<String>,
}

pub(crate) fn rewrite_domain_mod_source(
    src: &str,
    mod_name: &str,
    pascal: &str,
    solution_module_name: Option<&str>,
) -> Result<String, String> {
    let mut surface = DomainExportSurface::parse(src)?;
    let module_names = &mut surface.module_names;
    let use_map = &mut surface.use_map;
    if !module_names.iter().any(|existing| existing == mod_name) {
        module_names.push(mod_name.to_string());
    }
    ensure_primary_use_line(use_map, mod_name, pascal);
    if let Some(solution_module_name) = solution_module_name {
        move_name_to_end(module_names, solution_module_name);
    }

    managed_block::replace_block(
        src,
        DOMAIN_EXPORTS_BLOCK,
        &render_domain_export_block(module_names, use_map)?,
    )
}

pub(crate) fn remove_domain_mod_entry_source(
    src: &str,
    mod_name: &str,
    solution_module_name: Option<&str>,
) -> Result<String, String> {
    let mut surface = DomainExportSurface::parse(src)?;
    let module_names = &mut surface.module_names;
    let use_map = &mut surface.use_map;
    module_names.retain(|name| name != mod_name);
    use_map.remove(mod_name);
    if let Some(solution_module_name) = solution_module_name {
        move_name_to_end(module_names, solution_module_name);
    }

    managed_block::replace_block(
        src,
        DOMAIN_EXPORTS_BLOCK,
        &render_domain_export_block(module_names, use_map)?,
    )
}

pub(crate) fn validate_domain_mod_source(src: &str) -> Result<Vec<String>, String> {
    let manifest_body = planning_model_manifest_body(src)?;
    validate_planning_model_manifest(manifest_body)?;
    Ok(DomainExportSurface::parse(manifest_body)?.module_names)
}

fn planning_model_manifest_body(src: &str) -> Result<&str, String> {
    let macro_start = src.find("solverforge::planning_model!").ok_or_else(|| {
        "src/domain/mod.rs must declare solverforge::planning_model! { ... }".to_string()
    })?;
    let after_macro = macro_start + "solverforge::planning_model!".len();
    let open_offset = src[after_macro..].find('{').ok_or_else(|| {
        "src/domain/mod.rs must declare solverforge::planning_model! { ... }".to_string()
    })?;
    let open = after_macro + open_offset;
    let close = matching_brace(src, open).ok_or_else(|| {
        "src/domain/mod.rs planning_model! manifest has an unclosed body".to_string()
    })?;

    Ok(&src[open + 1..close])
}

fn matching_brace(src: &str, open: usize) -> Option<usize> {
    let mut depth = 0usize;
    for (offset, ch) in src[open..].char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => {
                depth = depth.checked_sub(1)?;
                if depth == 0 {
                    return Some(open + offset);
                }
            }
            _ => {}
        }
    }
    None
}

fn validate_planning_model_manifest(manifest_body: &str) -> Result<(), String> {
    if !manifest_body.contains("root = \"src/domain\"") {
        return Err(
            "src/domain/mod.rs planning_model! manifest must set root = \"src/domain\"".into(),
        );
    }
    Ok(())
}

fn render_domain_export_block(
    module_names: &[String],
    use_map: &DomainUseMap,
) -> Result<String, String> {
    if module_names.is_empty() {
        return Ok(String::new());
    }

    let module_lines = module_names
        .iter()
        .map(|name| format!("mod {name};"))
        .collect::<Vec<_>>()
        .join("\n");
    let use_lines = module_names
        .iter()
        .map(|name| managed_use_lines_for(use_map, name))
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .flatten()
        .collect::<Vec<_>>()
        .join("\n");

    Ok(format!("{module_lines}\n\n{use_lines}"))
}

fn parse_managed_mod_line(line: &str) -> Option<&str> {
    line.strip_prefix("mod ")?.strip_suffix(';')
}

fn parse_managed_use_line(line: &str) -> Option<(String, &str)> {
    let rest = line.strip_prefix("pub use ")?;
    let rest = rest.strip_prefix("self::").unwrap_or(rest);
    let module_name = if let Some((module_name, _)) = rest.split_once("::") {
        module_name
    } else if let Some((module_name, _)) = rest.split_once(' ') {
        module_name
    } else {
        rest.strip_suffix(';')?
    };
    if !is_simple_ident(module_name) {
        return None;
    }
    Some((module_name.to_string(), line))
}

fn move_name_to_end(names: &mut Vec<String>, target: &str) {
    if let Some(index) = names.iter().position(|name| name == target) {
        let name = names.remove(index);
        names.push(name);
    }
}

fn push_use_line(use_map: &mut DomainUseMap, module_name: &str, line: String) {
    let lines = use_map.entry(module_name.to_string()).or_default();
    lines.push(line);
}

fn ensure_primary_use_line(use_map: &mut DomainUseMap, module_name: &str, type_name: &str) {
    let lines = use_map.entry(module_name.to_string()).or_default();
    if !lines
        .iter()
        .any(|existing| is_primary_use_line(module_name, type_name, existing))
    {
        lines.insert(0, canonical_primary_use_line(module_name, type_name));
    }
}

fn managed_use_lines_for(use_map: &DomainUseMap, module_name: &str) -> Result<Vec<String>, String> {
    use_map
        .get(module_name)
        .cloned()
        .filter(|lines| !lines.is_empty())
        .ok_or_else(|| format!("missing managed re-export for module '{module_name}'"))
}

impl DomainExportSurface {
    fn parse(src: &str) -> Result<Self, String> {
        let block = managed_block::read_block(src, DOMAIN_EXPORTS_BLOCK)?;
        let mut module_names = Vec::new();
        let mut use_map = DomainUseMap::new();
        let mut seen_modules = BTreeSet::new();
        let mut seen_use_lines = BTreeSet::new();

        for line in block.lines().map(str::trim).filter(|line| !line.is_empty()) {
            if let Some(name) = parse_managed_mod_line(line) {
                if !seen_modules.insert(name.to_string()) {
                    return Err(format!(
                        "duplicate managed module entry for module '{name}'"
                    ));
                }
                module_names.push(name.to_string());
                continue;
            }
            if let Some((name, use_line)) = parse_managed_use_line(line) {
                if !seen_use_lines.insert(use_line.trim().to_string()) {
                    return Err(format!(
                        "duplicate managed re-export line in block '{DOMAIN_EXPORTS_BLOCK}': {use_line}"
                    ));
                }
                push_use_line(&mut use_map, &name, use_line.to_string());
                continue;
            }
            return Err(format!(
                "unsupported line in managed domain block '{DOMAIN_EXPORTS_BLOCK}': {line}"
            ));
        }

        for name in use_map.keys() {
            if !seen_modules.contains(name) {
                return Err(format!(
                    "managed domain block contains a re-export for undeclared module '{name}'"
                ));
            }
        }

        for name in &module_names {
            let use_lines = use_map.get(name).ok_or_else(|| {
                format!(
                    "managed domain block is missing the primary re-export '{}' for module '{name}'",
                    canonical_primary_use_line(name, &super::snake_to_pascal(name))
                )
            })?;
            let type_name = super::snake_to_pascal(name);
            if !use_lines
                .iter()
                .any(|line| is_primary_use_line(name, &type_name, line))
            {
                return Err(format!(
                    "managed domain block is missing the primary re-export '{}' for module '{name}'",
                    canonical_primary_use_line(name, &type_name)
                ));
            }
        }

        Ok(Self {
            module_names,
            use_map,
        })
    }
}

fn canonical_primary_use_line(module_name: &str, type_name: &str) -> String {
    format!("pub use {module_name}::{type_name};")
}

fn is_primary_use_line(module_name: &str, type_name: &str, line: &str) -> bool {
    let trimmed = line.trim();
    trimmed == canonical_primary_use_line(module_name, type_name)
        || trimmed == format!("pub use self::{module_name}::{type_name};")
}

fn is_simple_ident(value: &str) -> bool {
    !value.is_empty()
        && value
            .chars()
            .all(|ch| ch == '_' || ch.is_ascii_alphanumeric())
}

#[cfg(test)]
pub(crate) fn insert_field_and_import(
    src: &str,
    _solution_type: &str,
    pascal: &str,
    _plural: &str,
    field_block: &str,
) -> Result<String, String> {
    let (annotation, field_name, _type_name) = parse_collection_field_block(field_block)?;
    wire_solution_collection_source(src, pascal, &field_name, &annotation)
}

pub(crate) fn unwire_collection_from_solution_source(
    src: &str,
    field_name: &str,
    type_name: &str,
) -> Result<String, String> {
    let import_line = format!("use super::{type_name};");
    let param_line = format!("        {field_name}: Vec<{type_name}>,");
    let field_line = format!("pub {field_name}: Vec<{type_name}>");
    let init_none = format!("{field_name}: None,");
    let init_vec = format!("{field_name}: Vec::new(),");
    let init_direct = format!("{field_name},");

    let src = remove_exact_line_from_block(src, SOLUTION_IMPORTS_BLOCK, &import_line)?;
    let src = remove_entry_from_block(&src, SOLUTION_COLLECTIONS_BLOCK, |line| {
        line.trim().starts_with(&field_line)
    })?;
    let src = remove_exact_line_from_block(&src, SOLUTION_CONSTRUCTOR_PARAMS_BLOCK, &param_line)?;
    remove_entry_from_block(src.as_str(), SOLUTION_CONSTRUCTOR_INIT_BLOCK, |line| {
        let trimmed = line.trim();
        trimmed == init_none || trimmed == init_vec || trimmed == init_direct
    })
}

pub(crate) fn wire_solution_collection_source(
    src: &str,
    pascal: &str,
    field_name: &str,
    annotation: &str,
) -> Result<String, String> {
    let import_line = format!("use super::{pascal};");
    let field_line = format!("    pub {field_name}: Vec<{pascal}>,");
    if block_contains(src, SOLUTION_COLLECTIONS_BLOCK, |line| {
        line.trim() == field_line.trim()
    })? {
        return Ok(src.to_string());
    }

    let annotation_line = format!("    #[{annotation}]");
    let param_line = format!("        {field_name}: Vec<{pascal}>,");
    let init_line = format!("            {field_name},");

    let src = append_unique_line(src, SOLUTION_IMPORTS_BLOCK, &import_line)?;
    let src = append_unique_lines(
        &src,
        SOLUTION_COLLECTIONS_BLOCK,
        &[annotation_line, field_line],
    )?;
    let src = append_unique_line(&src, SOLUTION_CONSTRUCTOR_PARAMS_BLOCK, &param_line)?;
    append_unique_line(&src, SOLUTION_CONSTRUCTOR_INIT_BLOCK, &init_line)
}

#[cfg(test)]
fn parse_collection_field_block(field_block: &str) -> Result<(String, String, String), String> {
    let lines = field_block
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>();
    if lines.len() != 2 {
        return Err(
            "collection field block must contain exactly an attribute and a field".to_string(),
        );
    }

    let annotation = lines[0]
        .strip_prefix("#[")
        .and_then(|value| value.strip_suffix(']'))
        .ok_or_else(|| "collection field block is missing a Rust attribute".to_string())?;

    let field_line = lines[1]
        .strip_prefix("pub ")
        .and_then(|value| value.strip_suffix(','))
        .ok_or_else(|| "collection field block is missing a public Vec field".to_string())?;
    let (field_name, ty_part) = field_line
        .split_once(':')
        .ok_or_else(|| "collection field block has an invalid field declaration".to_string())?;
    let ty_part = ty_part.trim();
    let ty_name = ty_part
        .strip_prefix("Vec<")
        .and_then(|value| value.strip_suffix('>'))
        .ok_or_else(|| "collection field block must declare a Vec collection".to_string())?;

    Ok((
        annotation.to_string(),
        field_name.trim().to_string(),
        ty_name.to_string(),
    ))
}

fn append_unique_line(src: &str, label: &str, line: &str) -> Result<String, String> {
    let mut lines = read_non_empty_block_lines(src, label)?;
    if !lines.iter().any(|existing| existing.trim() == line.trim()) {
        lines.push(line.to_string());
    }
    managed_block::replace_block(src, label, &lines.join("\n"))
}

fn append_unique_lines(src: &str, label: &str, new_lines: &[String]) -> Result<String, String> {
    let mut lines = read_non_empty_block_lines(src, label)?;
    lines.extend(new_lines.iter().cloned());
    managed_block::replace_block(src, label, &lines.join("\n"))
}

fn remove_exact_line_from_block(src: &str, label: &str, line: &str) -> Result<String, String> {
    let mut lines = read_non_empty_block_lines(src, label)?;
    lines.retain(|existing| existing.trim() != line.trim());
    managed_block::replace_block(src, label, &lines.join("\n"))
}

fn remove_entry_from_block<F>(src: &str, label: &str, mut matches: F) -> Result<String, String>
where
    F: FnMut(&str) -> bool,
{
    let mut lines = read_non_empty_block_lines(src, label)?;
    if let Some(index) = lines.iter().position(|line| matches(line)) {
        let start = find_entry_start(&lines, index);
        lines.drain(start..=index);
    }
    managed_block::replace_block(src, label, &lines.join("\n"))
}

fn block_has_field_entry(src: &str, label: &str, field: &str) -> Result<bool, String> {
    Ok(read_managed_field_entries(src, label)?
        .iter()
        .any(|entry| entry.field_name == field))
}

fn render_managed_field_entries(entries: &[ManagedFieldEntry]) -> String {
    entries
        .iter()
        .flat_map(|entry| entry.lines.iter().map(String::as_str))
        .collect::<Vec<_>>()
        .join("\n")
}

fn block_contains<F>(src: &str, label: &str, mut predicate: F) -> Result<bool, String>
where
    F: FnMut(&str) -> bool,
{
    Ok(read_non_empty_block_lines(src, label)?
        .iter()
        .any(|line| predicate(line)))
}

fn read_non_empty_block_lines(src: &str, label: &str) -> Result<Vec<String>, String> {
    Ok(managed_block::read_block(src, label)?
        .lines()
        .map(|line| line.to_string())
        .filter(|line| !line.trim().is_empty())
        .collect())
}

fn read_managed_field_entries(src: &str, label: &str) -> Result<Vec<ManagedFieldEntry>, String> {
    let lines = read_non_empty_block_lines(src, label)?;
    let mut entries = Vec::new();
    let mut current = Vec::new();
    let mut seen = BTreeSet::new();

    for line in lines {
        let is_field_line = is_managed_field_declaration_line(&line);
        current.push(line);
        if is_field_line {
            let entry = parse_managed_field_entry(label, &current)?;
            if !seen.insert(entry.field_name.clone()) {
                return Err(format!(
                    "duplicate managed field '{}' in block '{label}'",
                    entry.field_name
                ));
            }
            entries.push(entry);
            current.clear();
        }
    }

    if !current.is_empty() {
        return Err(format!(
            "managed block '{label}' contains attached lines without a field declaration"
        ));
    }

    Ok(entries)
}

fn is_managed_field_declaration_line(line: &str) -> bool {
    let trimmed = line.trim();
    trimmed.starts_with("pub ") && trimmed.ends_with(',')
}

fn parse_managed_field_entry(label: &str, lines: &[String]) -> Result<ManagedFieldEntry, String> {
    let entry_src = lines.join("\n");
    let wrapped = format!("struct __SolverForgeManagedField {{\n{entry_src}\n}}\n");
    let file = syn::parse_file(&wrapped)
        .map_err(|err| format!("invalid managed field entry in block '{label}': {err}"))?;
    let mut items = file.items.into_iter();
    let Some(syn::Item::Struct(item)) = items.next() else {
        return Err(format!(
            "managed field entry in block '{label}' did not parse as a struct field"
        ));
    };
    if items.next().is_some() {
        return Err(format!(
            "managed field entry in block '{label}' parsed extra Rust items"
        ));
    }

    let syn::Fields::Named(fields) = item.fields else {
        return Err(format!(
            "managed field entry in block '{label}' must use named fields"
        ));
    };
    if fields.named.len() != 1 {
        return Err(format!(
            "managed field entry in block '{label}' must contain exactly one field"
        ));
    }
    let field = fields
        .named
        .into_iter()
        .next()
        .expect("field length checked above");
    if !matches!(field.vis, syn::Visibility::Public(_)) {
        return Err(format!(
            "managed field entry in block '{label}' must declare a public field"
        ));
    }
    let field_name = field
        .ident
        .ok_or_else(|| format!("managed field entry in block '{label}' is missing a field name"))?
        .to_string();

    Ok(ManagedFieldEntry {
        field_name,
        lines: lines.to_vec(),
    })
}

fn find_entry_start(lines: &[String], index: usize) -> usize {
    let mut start = index;
    while start > 0 && is_attached_prefix_line(lines[start - 1].trim()) {
        start -= 1;
    }
    start
}

fn is_attached_prefix_line(line: &str) -> bool {
    line.starts_with("#[")
        || line.starts_with("#!")
        || line.starts_with("///")
        || line.starts_with("//")
        || line.starts_with("/*")
        || line.starts_with('*')
        || line.starts_with("*/")
}

#[cfg(test)]
pub(crate) fn inject_planning_variable(
    src: &str,
    entity: &str,
    field: &str,
) -> Result<String, String> {
    inject_scalar_variable(
        src,
        entity,
        field,
        ScalarVariableWiring {
            range: "",
            countable_range: None,
            allows_unassigned: true,
            hooks: &ScalarVariableHooks::default(),
        },
    )
}

pub(crate) struct ScalarVariableWiring<'a> {
    pub(crate) range: &'a str,
    pub(crate) countable_range: Option<&'a str>,
    pub(crate) allows_unassigned: bool,
    pub(crate) hooks: &'a ScalarVariableHooks,
}

pub(crate) fn inject_scalar_variable(
    src: &str,
    entity: &str,
    field: &str,
    wiring: ScalarVariableWiring<'_>,
) -> Result<String, String> {
    let ScalarVariableWiring {
        range,
        countable_range,
        allows_unassigned,
        hooks,
    } = wiring;
    if block_has_field_entry(src, ENTITY_VARIABLES_BLOCK, field)? {
        return Err(format!("field '{field}' already exists in {entity}"));
    }

    let annotation =
        render_scalar_variable_annotation(range, countable_range, allows_unassigned, hooks);
    let field_lines = vec![annotation, format!("    pub {field}: Option<usize>,")];
    let init_lines = [format!("            {field}: None,")];

    let src = append_unique_lines(src, ENTITY_VARIABLES_BLOCK, &field_lines)?;
    append_unique_lines(&src, ENTITY_VARIABLE_INIT_BLOCK, &init_lines)
}

fn render_scalar_variable_annotation(
    range: &str,
    countable_range: Option<&str>,
    allows_unassigned: bool,
    hooks: &ScalarVariableHooks,
) -> String {
    let mut args = Vec::new();
    if !range.is_empty() {
        args.push(format!("value_range_provider = \"{range}\""));
    }
    if let Some(range) = countable_range {
        args.push(format!("countable_range = \"{range}\""));
    }
    args.push(format!("allows_unassigned = {allows_unassigned}"));
    for (name, value) in hooks.entries() {
        if let Some(value) = value {
            args.push(format!("{name} = \"{value}\""));
        }
    }

    if hooks.is_empty() {
        return format!("    #[planning_variable({})]", args.join(", "));
    }

    let rendered_args = args
        .iter()
        .enumerate()
        .map(|(index, arg)| {
            let suffix = if index + 1 == args.len() { "" } else { "," };
            format!("        {arg}{suffix}")
        })
        .collect::<Vec<_>>()
        .join("\n");
    format!("    #[planning_variable(\n{rendered_args}\n    )]")
}

pub(crate) fn inject_list_variable(
    src: &str,
    entity: &str,
    field: &str,
    elements: &str,
    metadata: &ListVariableMetadata,
) -> Result<String, String> {
    let field_line = format!("    pub {field}: Vec<usize>,");
    if block_has_field_entry(src, ENTITY_VARIABLES_BLOCK, field)? {
        return Err(format!("field '{field}' already exists in {entity}"));
    }

    let annotation = render_list_variable_annotation(elements, metadata);
    let init_line = format!("            {field}: Vec::new(),");

    let src = append_unique_lines(src, ENTITY_VARIABLES_BLOCK, &[annotation, field_line])?;
    append_unique_line(&src, ENTITY_VARIABLE_INIT_BLOCK, &init_line)
}

fn render_list_variable_annotation(elements: &str, metadata: &ListVariableMetadata) -> String {
    let mut args = vec![format!("element_collection = \"{elements}\"")];
    for (name, value) in metadata.entries() {
        if let Some(value) = value {
            args.push(format!("{name} = \"{value}\""));
        }
    }

    if metadata.is_empty() {
        return format!("    #[planning_list_variable({})]", args.join(", "));
    }

    let rendered_args = args
        .iter()
        .enumerate()
        .map(|(index, arg)| {
            let suffix = if index + 1 == args.len() { "" } else { "," };
            format!("        {arg}{suffix}")
        })
        .collect::<Vec<_>>()
        .join("\n");
    format!("    #[planning_list_variable(\n{rendered_args}\n    )]")
}

pub(crate) fn remove_variable_field(src: &str, field: &str) -> Result<String, String> {
    let entries = read_managed_field_entries(src, ENTITY_VARIABLES_BLOCK)?;
    if !entries.iter().any(|entry| entry.field_name == field) {
        return Err(format!("field '{field}' not found"));
    }
    let retained = entries
        .into_iter()
        .filter(|entry| entry.field_name != field)
        .collect::<Vec<_>>();
    let src = managed_block::replace_block(
        src,
        ENTITY_VARIABLES_BLOCK,
        &render_managed_field_entries(&retained),
    )?;
    let initializers = read_non_empty_block_lines(&src, ENTITY_VARIABLE_INIT_BLOCK)?
        .into_iter()
        .filter(|line| {
            let trimmed = line.trim();
            trimmed != format!("{field}: None,") && trimmed != format!("{field}: Vec::new(),")
        })
        .collect::<Vec<_>>();
    managed_block::replace_block(&src, ENTITY_VARIABLE_INIT_BLOCK, &initializers.join("\n"))
}

/// Replaces the score type in the solution file.
pub(crate) fn replace_score_type(
    src: &str,
    old_score: &str,
    new_score: &str,
) -> Result<String, String> {
    let updated = if old_score.chars().all(is_rust_identifier_char) {
        replace_identifier(src, old_score, new_score)
    } else {
        src.replace(old_score, new_score)
    };
    if updated == src {
        return Err(format!(
            "score type '{old_score}' not found in solution file"
        ));
    }
    Ok(updated)
}

fn replace_identifier(src: &str, from: &str, to: &str) -> String {
    let mut out = String::with_capacity(src.len());
    let mut start = 0;

    for (idx, _) in src.match_indices(from) {
        let before = src[..idx].chars().next_back();
        let after = src[idx + from.len()..].chars().next();
        if before.is_some_and(is_rust_identifier_char) || after.is_some_and(is_rust_identifier_char)
        {
            continue;
        }
        out.push_str(&src[start..idx]);
        out.push_str(to);
        start = idx + from.len();
    }

    out.push_str(&src[start..]);
    out
}

fn is_rust_identifier_char(ch: char) -> bool {
    ch == '_' || ch.is_ascii_alphanumeric()
}

#[cfg(test)]
pub(crate) fn add_import(src: &str, import: &str) -> String {
    if src.contains(import) {
        return src.to_string();
    }
    let mut lines: Vec<&str> = src.lines().collect();
    let last_use = lines
        .iter()
        .rposition(|line| line.trim_start().starts_with("use "));
    let insert_at = last_use.map(|index| index + 1).unwrap_or(0);
    lines.insert(insert_at, import);
    lines.join("\n") + "\n"
}
