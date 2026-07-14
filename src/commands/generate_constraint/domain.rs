use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use crate::list_variable_metadata::ListVariableMetadata;
use crate::scalar_variable_hooks::ScalarVariableHooks;
use quote::ToTokens;
use syn::parse::{Parse, ParseStream, Parser};
use syn::{
    Attribute, Error, Expr, Fields, GenericArgument, Ident, Item, ItemMacro, ItemMod, ItemStruct,
    ItemType, ItemUse, Lit, LitStr, Meta, PathArguments, Token, Type, UseTree, Visibility,
};

#[derive(Debug, Clone)]
pub(crate) struct ScalarVarInfo {
    pub field: String,
    pub value_range_provider: String,
    pub countable_range: Option<String>,
    pub allows_unassigned: bool,
    pub hooks: ScalarVariableHooks,
}

#[derive(Debug, Clone)]
pub(crate) struct ListVarInfo {
    pub field: String,
    pub element_collection: String,
    pub metadata: ListVariableMetadata,
}

#[derive(Debug)]
pub(crate) struct EntityInfo {
    pub field_name: String,
    pub item_type: String,
    pub scalar_vars: Vec<ScalarVarInfo>,
    pub list_vars: Vec<ListVarInfo>,
}

#[derive(Debug)]
pub(crate) struct FactInfo {
    pub field_name: String,
    pub item_type: String,
}

#[derive(Debug)]
pub(crate) struct DomainModel {
    pub solution_type: String,
    pub score_type: String,
    pub scalar_groups_path: Option<String>,
    pub conflict_repairs_path: Option<String>,
    pub entities: Vec<EntityInfo>,
    pub facts: Vec<FactInfo>,
}

struct PlanningModelInput {
    root: LitStr,
    items: Vec<ManifestItem>,
}

enum ManifestItem {
    Mod(ItemMod),
    Use(ItemUse),
}

struct ModuleSource {
    file: syn::File,
}

struct ManifestSurface {
    root: String,
    modules: Vec<String>,
    exports: BTreeMap<String, BTreeSet<String>>,
}

struct SolutionInfo {
    type_name: String,
    score_type: String,
    scalar_groups_path: Option<String>,
    conflict_repairs_path: Option<String>,
    entity_collections: Vec<(String, String)>,
    fact_collections: Vec<(String, String)>,
}

struct EntityStructInfo {
    scalar_vars: Vec<ScalarVarInfo>,
    list_vars: Vec<ListVarInfo>,
}

impl Parse for PlanningModelInput {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let root_ident: Ident = input.parse()?;
        if root_ident != "root" {
            return Err(Error::new_spanned(root_ident, "expected `root`"));
        }
        input.parse::<Token![=]>()?;
        let root = input.parse::<LitStr>()?;
        input.parse::<Token![;]>()?;

        let mut items = Vec::new();
        while !input.is_empty() {
            let item = input.parse::<Item>()?;
            match item {
                Item::Mod(item_mod) => {
                    if item_mod.content.is_some() {
                        return Err(Error::new_spanned(
                            item_mod,
                            "planning_model! only accepts file-backed `mod name;` declarations",
                        ));
                    }
                    items.push(ManifestItem::Mod(item_mod));
                }
                Item::Use(item_use) => {
                    if !matches!(item_use.vis, Visibility::Public(_)) {
                        return Err(Error::new_spanned(
                            item_use,
                            "planning_model! only accepts public use exports",
                        ));
                    }
                    items.push(ManifestItem::Use(item_use));
                }
                other => {
                    return Err(Error::new_spanned(
                        other,
                        "planning_model! accepts only `mod name;` and `pub use ...;` items",
                    ));
                }
            }
        }

        Ok(Self { root, items })
    }
}

/// Parses the canonical `solverforge::planning_model!` domain manifest plus the
/// file-backed modules it declares.
pub(crate) fn parse_domain() -> Result<DomainModel, String> {
    parse_domain_from_manifest(Path::new("src/domain/mod.rs"))
}

fn parse_domain_from_manifest(mod_path: &Path) -> Result<DomainModel, String> {
    let manifest_src = fs::read_to_string(mod_path)
        .map_err(|err| format!("failed to read {}: {}", mod_path.display(), err))?;
    let manifest = parse_manifest_surface(&manifest_src)?;
    if manifest.root != "src/domain" {
        return Err(
            "src/domain/mod.rs planning_model! manifest must set root = \"src/domain\"".to_string(),
        );
    }

    let root = Path::new(&manifest.root);
    let modules = read_manifest_modules(root, &manifest.modules)?;
    collect_domain_model(&modules, &manifest)
}

fn parse_manifest_surface(src: &str) -> Result<ManifestSurface, String> {
    let file = syn::parse_file(src)
        .map_err(|err| format!("failed to parse src/domain/mod.rs: {}", err))?;
    let item_macro = find_planning_model_macro(&file)?;
    let input = syn::parse2::<PlanningModelInput>(item_macro.mac.tokens.clone())
        .map_err(|err| format!("invalid planning_model! manifest: {}", err))?;

    let modules = input
        .items
        .iter()
        .filter_map(|item| match item {
            ManifestItem::Mod(item_mod) => Some(item_mod.ident.to_string()),
            ManifestItem::Use(_) => None,
        })
        .collect::<Vec<_>>();
    let declared = modules.iter().cloned().collect::<BTreeSet<_>>();
    let mut exports: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();

    for item in &input.items {
        let ManifestItem::Use(item_use) = item else {
            continue;
        };
        collect_exports(&item_use.tree, None, &mut exports)?;
    }

    for module in exports.keys() {
        if !declared.contains(module) {
            return Err(format!(
                "planning_model! exports type(s) from undeclared module `{module}`"
            ));
        }
    }

    Ok(ManifestSurface {
        root: input.root.value(),
        modules,
        exports,
    })
}

fn find_planning_model_macro(file: &syn::File) -> Result<&ItemMacro, String> {
    let mut matches = file.items.iter().filter_map(|item| match item {
        Item::Macro(item_macro) if path_matches_ident(&item_macro.mac.path, "planning_model") => {
            Some(item_macro)
        }
        _ => None,
    });
    let first = matches.next().ok_or_else(|| {
        "src/domain/mod.rs must declare solverforge::planning_model! { ... }".to_string()
    })?;
    if matches.next().is_some() {
        return Err("src/domain/mod.rs must declare exactly one planning_model! manifest".into());
    }
    Ok(first)
}

fn collect_exports(
    tree: &UseTree,
    module: Option<String>,
    exports: &mut BTreeMap<String, BTreeSet<String>>,
) -> Result<(), String> {
    match tree {
        UseTree::Path(path) => {
            let segment = path.ident.to_string();
            let next_module = if segment == "self" {
                module
            } else if module.is_none() {
                Some(segment)
            } else {
                module
            };
            collect_exports(&path.tree, next_module, exports)
        }
        UseTree::Name(name) => {
            let module = module.ok_or_else(|| {
                "planning_model! public exports must use `pub use module::Type;`".to_string()
            })?;
            exports
                .entry(module)
                .or_default()
                .insert(name.ident.to_string());
            Ok(())
        }
        UseTree::Group(group) => {
            for item in &group.items {
                collect_exports(item, module.clone(), exports)?;
            }
            Ok(())
        }
        UseTree::Rename(rename) => Err(format!(
            "planning_model! public export renames are not supported: {} as {}",
            rename.ident, rename.rename
        )),
        UseTree::Glob(_) => Err("planning_model! public glob exports are not supported".into()),
    }
}

fn read_manifest_modules(root: &Path, modules: &[String]) -> Result<Vec<ModuleSource>, String> {
    let mut sources = Vec::new();
    for name in modules {
        let path = module_path(root, name).ok_or_else(|| {
            format!(
                "planning_model! module `{name}` must resolve to `{}/{name}.rs` or `{}/{name}/mod.rs`",
                root.display(),
                root.display(),
            )
        })?;
        let source = fs::read_to_string(&path).map_err(|err| {
            format!(
                "planning_model! could not read module `{name}` at `{}`: {err}",
                path.display()
            )
        })?;
        let file = syn::parse_file(&source).map_err(|err| {
            format!(
                "planning_model! could not parse module `{name}` at `{}`: {err}",
                path.display()
            )
        })?;
        sources.push(ModuleSource { file });
    }
    Ok(sources)
}

fn module_path(root: &Path, name: &str) -> Option<PathBuf> {
    let file_path = root.join(format!("{name}.rs"));
    if file_path.exists() {
        return Some(file_path);
    }
    let mod_path = root.join(name).join("mod.rs");
    mod_path.exists().then_some(mod_path)
}

fn collect_domain_model(
    modules: &[ModuleSource],
    manifest: &ManifestSurface,
) -> Result<DomainModel, String> {
    let mut solution: Option<SolutionInfo> = None;
    let mut entities = BTreeMap::new();
    let mut facts = BTreeSet::new();
    let mut aliases = BTreeMap::new();

    for module in modules {
        for item in &module.file.items {
            match item {
                Item::Struct(item_struct) => {
                    if has_attribute(&item_struct.attrs, "planning_solution") {
                        if let Some(existing) = &solution {
                            return Err(format!(
                                "planning_model! found duplicate #[planning_solution]; `{}` is already the model solution",
                                existing.type_name
                            ));
                        }
                        solution = Some(parse_solution(item_struct)?);
                    }
                    if has_attribute(&item_struct.attrs, "planning_entity") {
                        entities.insert(item_struct.ident.to_string(), parse_entity(item_struct)?);
                    }
                    if has_attribute(&item_struct.attrs, "problem_fact") {
                        facts.insert(item_struct.ident.to_string());
                    }
                }
                Item::Type(item_type) => {
                    if let Some(target) = alias_target_name(item_type) {
                        aliases.insert(item_type.ident.to_string(), target);
                    }
                }
                _ => {}
            }
        }
    }

    let solution = solution.ok_or_else(|| {
        "planning_model! requires exactly one #[planning_solution] in the listed modules"
            .to_string()
    })?;

    require_public_export(manifest, &solution.type_name)?;

    let mut domain_entities = Vec::new();
    for (field_name, collection_type) in &solution.entity_collections {
        let resolved = resolve_alias(collection_type, &aliases);
        let entity = entities.get(resolved).ok_or_else(|| {
            format!(
                "planning_model! entity collection `{field_name}` references unknown #[planning_entity] type `{collection_type}`"
            )
        })?;
        require_public_export(manifest, collection_type)?;
        domain_entities.push(EntityInfo {
            field_name: field_name.clone(),
            item_type: collection_type.to_string(),
            scalar_vars: entity.scalar_vars.clone(),
            list_vars: entity.list_vars.clone(),
        });
    }

    let mut domain_facts = Vec::new();
    for (field_name, collection_type) in &solution.fact_collections {
        let resolved = resolve_alias(collection_type, &aliases);
        if !facts.contains(resolved) && !entities.contains_key(resolved) {
            return Err(format!(
                "planning_model! problem fact collection `{field_name}` references unknown #[problem_fact] type `{collection_type}`"
            ));
        }
        require_public_export(manifest, collection_type)?;
        domain_facts.push(FactInfo {
            field_name: field_name.clone(),
            item_type: collection_type.to_string(),
        });
    }

    let solution_fields = solution
        .entity_collections
        .iter()
        .chain(solution.fact_collections.iter())
        .map(|(field, _)| field.as_str())
        .collect::<BTreeSet<_>>();
    for entity in &domain_entities {
        for list_var in &entity.list_vars {
            if !solution_fields.contains(list_var.element_collection.as_str()) {
                return Err(format!(
                    "planning_model! list entity `{}` requires a solution collection field named `{}`",
                    entity.item_type, list_var.element_collection
                ));
            }
        }
    }

    Ok(DomainModel {
        solution_type: solution.type_name,
        score_type: solution.score_type,
        scalar_groups_path: solution.scalar_groups_path,
        conflict_repairs_path: solution.conflict_repairs_path,
        entities: domain_entities,
        facts: domain_facts,
    })
}

fn parse_solution(item_struct: &ItemStruct) -> Result<SolutionInfo, String> {
    let fields = named_fields(item_struct, "#[planning_solution] requires named fields")?;
    let mut entity_collections = Vec::new();
    let mut fact_collections = Vec::new();
    let mut score_type = None;
    let planning_solution_attr = get_attribute(&item_struct.attrs, "planning_solution");

    for field in fields {
        let field_name = field
            .ident
            .as_ref()
            .map(ToString::to_string)
            .ok_or_else(|| "#[planning_solution] requires named fields".to_string())?;
        if has_attribute(&field.attrs, "planning_entity_collection") {
            let type_name = collection_type_name(&field.ty).ok_or_else(|| {
                format!("#[planning_entity_collection] field `{field_name}` requires Vec<T>")
            })?;
            entity_collections.push((field_name, type_name));
        } else if has_attribute(&field.attrs, "problem_fact_collection") {
            let type_name = collection_type_name(&field.ty).ok_or_else(|| {
                format!("#[problem_fact_collection] field `{field_name}` requires Vec<T>")
            })?;
            fact_collections.push((field_name, type_name));
        } else if has_attribute(&field.attrs, "planning_score") {
            let inner = option_inner_type(&field.ty).unwrap_or(&field.ty);
            score_type = Some(canonical_type(inner));
        }
    }

    let score_type = score_type.ok_or_else(|| {
        format!(
            "planning solution `{}` is missing #[planning_score]",
            item_struct.ident
        )
    })?;

    Ok(SolutionInfo {
        type_name: item_struct.ident.to_string(),
        score_type,
        scalar_groups_path: planning_solution_attr
            .and_then(|attr| parse_attribute_string(attr, "scalar_groups")),
        conflict_repairs_path: planning_solution_attr
            .and_then(|attr| parse_attribute_string(attr, "conflict_repairs")),
        entity_collections,
        fact_collections,
    })
}

fn parse_entity(item_struct: &ItemStruct) -> Result<EntityStructInfo, String> {
    let fields = named_fields(item_struct, "#[planning_entity] requires named fields")?;
    let mut scalar_vars = Vec::new();
    let mut list_vars = Vec::new();

    for field in fields {
        if has_attribute(&field.attrs, "planning_variable") {
            if !field_is_option_usize(&field.ty) {
                continue;
            }
            let Some(field_ident) = field.ident.as_ref() else {
                continue;
            };
            let attr = get_attribute(&field.attrs, "planning_variable").unwrap();
            scalar_vars.push(ScalarVarInfo {
                field: field_ident.to_string(),
                value_range_provider: parse_attribute_string(attr, "value_range_provider")
                    .unwrap_or_default(),
                countable_range: parse_attribute_string(attr, "countable_range"),
                allows_unassigned: parse_attribute_bool(attr, "allows_unassigned").unwrap_or(false),
                hooks: parse_scalar_variable_hooks(attr),
            });
        }

        if has_attribute(&field.attrs, "planning_list_variable") {
            let Some(field_ident) = field.ident.as_ref() else {
                continue;
            };
            let attr = get_attribute(&field.attrs, "planning_list_variable").unwrap();
            let element_collection = parse_attribute_string(attr, "element_collection")
                .ok_or_else(|| {
                    format!(
                        "#[planning_list_variable] field `{}` requires element_collection",
                        field_ident
                    )
                })?;
            list_vars.push(ListVarInfo {
                field: field_ident.to_string(),
                element_collection,
                metadata: parse_list_variable_metadata(attr),
            });
        }
    }

    Ok(EntityStructInfo {
        scalar_vars,
        list_vars,
    })
}

fn parse_scalar_variable_hooks(attr: &Attribute) -> ScalarVariableHooks {
    ScalarVariableHooks {
        candidate_values: parse_attribute_string(attr, "candidate_values"),
        nearby_value_candidates: parse_attribute_string(attr, "nearby_value_candidates"),
        nearby_entity_candidates: parse_attribute_string(attr, "nearby_entity_candidates"),
        nearby_value_distance_meter: parse_attribute_string(attr, "nearby_value_distance_meter"),
        nearby_entity_distance_meter: parse_attribute_string(attr, "nearby_entity_distance_meter"),
        construction_entity_order_key: parse_attribute_string(
            attr,
            "construction_entity_order_key",
        ),
        construction_value_order_key: parse_attribute_string(attr, "construction_value_order_key"),
    }
}

fn parse_list_variable_metadata(attr: &Attribute) -> ListVariableMetadata {
    ListVariableMetadata {
        domain: parse_attribute_string(attr, "domain"),
        distance_meter: parse_attribute_string(attr, "distance_meter"),
        intra_distance_meter: parse_attribute_string(attr, "intra_distance_meter"),
        route_hooks: parse_attribute_string(attr, "route_hooks"),
        savings_hooks: parse_attribute_string(attr, "savings_hooks"),
        savings_metric_class_fn: parse_attribute_string(attr, "savings_metric_class_fn"),
        element_owner_fn: parse_attribute_string(attr, "element_owner_fn"),
        construction_element_order_key: parse_attribute_string(
            attr,
            "construction_element_order_key",
        ),
        precedence_duration_fn: parse_attribute_string(attr, "precedence_duration_fn"),
        precedence_successors_fn: parse_attribute_string(attr, "precedence_successors_fn"),
        solution_trait: parse_attribute_string(attr, "solution_trait"),
    }
}

fn named_fields<'a>(
    item_struct: &'a ItemStruct,
    message: &'static str,
) -> Result<&'a syn::punctuated::Punctuated<syn::Field, Token![,]>, String> {
    match &item_struct.fields {
        Fields::Named(fields) => Ok(&fields.named),
        _ => Err(message.to_string()),
    }
}

fn alias_target_name(item_type: &ItemType) -> Option<String> {
    type_name(&item_type.ty)
}

fn resolve_alias<'a>(type_name: &'a str, aliases: &'a BTreeMap<String, String>) -> &'a str {
    aliases
        .get(type_name)
        .map(String::as_str)
        .unwrap_or(type_name)
}

fn require_public_export(manifest: &ManifestSurface, type_name: &str) -> Result<(), String> {
    if manifest
        .exports
        .values()
        .any(|types| types.contains(type_name))
    {
        return Ok(());
    }
    Err(format!(
        "planning_model! type `{type_name}` must be publicly exported from src/domain/mod.rs"
    ))
}

fn collection_type_name(ty: &Type) -> Option<String> {
    let inner = collection_inner_type(ty)?;
    type_name(inner)
}

fn collection_inner_type(ty: &Type) -> Option<&Type> {
    let Type::Path(type_path) = ty else {
        return None;
    };
    let segment = type_path.path.segments.last()?;
    if segment.ident != "Vec" {
        return None;
    }
    let PathArguments::AngleBracketed(args) = &segment.arguments else {
        return None;
    };
    let Some(GenericArgument::Type(inner)) = args.args.first() else {
        return None;
    };
    Some(inner)
}

fn option_inner_type(ty: &Type) -> Option<&Type> {
    let Type::Path(type_path) = ty else {
        return None;
    };
    let segment = type_path.path.segments.last()?;
    if segment.ident != "Option" {
        return None;
    }
    let PathArguments::AngleBracketed(args) = &segment.arguments else {
        return None;
    };
    let Some(GenericArgument::Type(inner)) = args.args.first() else {
        return None;
    };
    Some(inner)
}

fn type_name(ty: &Type) -> Option<String> {
    let Type::Path(type_path) = ty else {
        return None;
    };
    Some(type_path.path.segments.last()?.ident.to_string())
}

fn field_is_option_usize(ty: &Type) -> bool {
    option_inner_type(ty).and_then(type_name).as_deref() == Some("usize")
}

fn canonical_type(ty: &Type) -> String {
    let raw = ty.to_token_stream().to_string();
    canonicalize_type(&raw)
}

fn canonicalize_type(raw: &str) -> String {
    let mut value = raw
        .replace(" :: ", "::")
        .replace(":: ", "::")
        .replace(" ::", "::")
        .replace(" < ", "<")
        .replace(" <", "<")
        .replace("< ", "<")
        .replace(" > ", ">")
        .replace(" >", ">")
        .replace("> ", ">")
        .replace(" , ", ", ")
        .replace(" ,", ",")
        .replace(",  ", ", ");
    while value.contains("  ") {
        value = value.replace("  ", " ");
    }
    value
}

fn has_attribute(attrs: &[Attribute], name: &str) -> bool {
    attrs
        .iter()
        .any(|attr| path_matches_ident(attr.path(), name))
}

fn get_attribute<'a>(attrs: &'a [Attribute], name: &str) -> Option<&'a Attribute> {
    attrs
        .iter()
        .find(|attr| path_matches_ident(attr.path(), name))
}

fn path_matches_ident(path: &syn::Path, name: &str) -> bool {
    path.segments
        .last()
        .is_some_and(|segment| segment.ident == name)
}

fn parse_attribute_bool(attr: &Attribute, key: &str) -> Option<bool> {
    parse_attribute_meta(attr).into_iter().find_map(|meta| {
        let Meta::NameValue(name_value) = meta else {
            return None;
        };
        if !path_matches_ident(&name_value.path, key) {
            return None;
        }
        let Expr::Lit(expr_lit) = name_value.value else {
            return None;
        };
        let Lit::Bool(lit_bool) = expr_lit.lit else {
            return None;
        };
        Some(lit_bool.value())
    })
}

fn parse_attribute_string(attr: &Attribute, key: &str) -> Option<String> {
    parse_attribute_meta(attr).into_iter().find_map(|meta| {
        let Meta::NameValue(name_value) = meta else {
            return None;
        };
        if !path_matches_ident(&name_value.path, key) {
            return None;
        }
        let Expr::Lit(expr_lit) = name_value.value else {
            return None;
        };
        let Lit::Str(lit_str) = expr_lit.lit else {
            return None;
        };
        Some(lit_str.value())
    })
}

fn parse_attribute_meta(attr: &Attribute) -> Vec<Meta> {
    let Meta::List(meta_list) = &attr.meta else {
        return Vec::new();
    };
    let parser = syn::punctuated::Punctuated::<Meta, Token![,]>::parse_terminated;
    parser
        .parse2(meta_list.tokens.clone())
        .map(|items| items.into_iter().collect())
        .unwrap_or_default()
}

pub(crate) fn find_file_for_type(domain_dir: &Path, type_name: &str) -> Result<PathBuf, String> {
    let entries =
        fs::read_dir(domain_dir).map_err(|e| format!("failed to read src/domain/: {}", e))?;

    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("rs") {
            continue;
        }
        let src = fs::read_to_string(&path)
            .map_err(|err| format!("failed to read {}: {}", path.display(), err))?;
        let file = syn::parse_file(&src)
            .map_err(|err| format!("failed to parse {}: {}", path.display(), err))?;
        if file
            .items
            .iter()
            .any(|item| type_declares_name(item, type_name))
        {
            return Ok(path);
        }
    }

    Err(format!(
        "struct or type alias '{}' not found in src/domain/",
        type_name
    ))
}

fn type_declares_name(item: &Item, type_name: &str) -> bool {
    match item {
        Item::Struct(item_struct) => item_struct.ident == type_name,
        Item::Type(item_type) => item_type.ident == type_name,
        _ => false,
    }
}

pub(crate) fn list_constraints(dir: &Path) -> Vec<String> {
    let mut constraints = Vec::new();
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("rs") {
                let name = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
                if name != "mod" {
                    constraints.push(name.to_string());
                }
            }
        }
    }
    constraints.sort();
    constraints
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn parses_multiline_attrs_and_manifest_modules() {
        let _guard = crate::test_support::lock_cwd();
        let temp = tempdir().unwrap();
        let old = std::env::current_dir().unwrap();
        std::env::set_current_dir(temp.path()).unwrap();
        fs::create_dir_all("src/domain").unwrap();
        fs::write(
            "src/domain/mod.rs",
            r#"solverforge::planning_model! {
    root = "src/domain";

    mod task;
    mod worker;
    mod plan;

    pub use task::Task;
    pub use worker::Worker;
    pub use plan::Plan;
}
"#,
        )
        .unwrap();
        fs::write(
            "src/domain/task.rs",
            r#"use solverforge::prelude::*;

#[solverforge::planning_entity]
pub struct Task {
    #[planning_id]
    pub id: usize,

    #[planning_variable(
        value_range_provider = "workers",
        allows_unassigned = true,
        candidate_values = "worker_candidates",
        nearby_value_candidates = "nearby_workers",
        nearby_entity_candidates = "nearby_tasks",
        nearby_value_distance_meter = "worker_distance",
        nearby_entity_distance_meter = "task_distance",
        construction_entity_order_key = "task_priority",
        construction_value_order_key = "worker_priority",
    )]
    pub worker: Option<usize>,

    #[planning_variable(countable_range = "2..7")]
    pub priority: Option<usize>,

    #[planning_list_variable(
        element_collection = "workers",
        distance_meter = "cross_distance",
        intra_distance_meter = "intra_distance",
        route_hooks = "route_hooks",
        savings_hooks = "savings_hooks",
        savings_metric_class_fn = "metric_class",
        element_owner_fn = "fixed_owner",
        construction_element_order_key = "worker_order",
        precedence_duration_fn = "worker_duration",
        precedence_successors_fn = "worker_successors",
        solution_trait = "RouteSolution",
    )]
    pub worker_order: Vec<usize>,
}
"#,
        )
        .unwrap();
        fs::write(
            "src/domain/worker.rs",
            r#"use solverforge::prelude::*;

#[problem_fact]
pub struct Worker {
    #[planning_id]
    pub id: usize,
}
"#,
        )
        .unwrap();
        fs::write(
            "src/domain/plan.rs",
            r#"use solverforge::prelude::*;
use super::{Task, Worker};

#[planning_solution(
    constraints = "crate::constraints::create_constraints",
    solver_toml = "../../solver.toml",
    scalar_groups = "scalar_groups",
    conflict_repairs = "conflict_repairs",
)]
pub struct Plan {
    #[problem_fact_collection]
    pub workers: Vec<Worker>,
    #[planning_entity_collection]
    pub tasks: Vec<Task>,
    #[planning_score]
    pub score: Option<BendableScore<2, 3>>,
}
"#,
        )
        .unwrap();

        let domain = parse_domain().unwrap();
        assert_eq!(domain.solution_type, "Plan");
        assert_eq!(domain.score_type, "BendableScore<2, 3>");
        assert_eq!(domain.scalar_groups_path.as_deref(), Some("scalar_groups"));
        assert_eq!(
            domain.conflict_repairs_path.as_deref(),
            Some("conflict_repairs")
        );
        assert_eq!(domain.entities[0].item_type, "Task");
        assert_eq!(domain.entities[0].scalar_vars[0].field, "worker");
        assert_eq!(
            domain.entities[0].scalar_vars[0].value_range_provider,
            "workers"
        );
        assert!(domain.entities[0].scalar_vars[0].allows_unassigned);
        assert_eq!(
            domain.entities[0].scalar_vars[1].countable_range.as_deref(),
            Some("2..7")
        );
        assert_eq!(
            domain.entities[0].scalar_vars[0]
                .hooks
                .candidate_values
                .as_deref(),
            Some("worker_candidates")
        );
        assert_eq!(
            domain.entities[0].scalar_vars[0]
                .hooks
                .nearby_value_candidates
                .as_deref(),
            Some("nearby_workers")
        );
        assert_eq!(
            domain.entities[0].scalar_vars[0]
                .hooks
                .nearby_entity_candidates
                .as_deref(),
            Some("nearby_tasks")
        );
        assert_eq!(
            domain.entities[0].scalar_vars[0]
                .hooks
                .nearby_value_distance_meter
                .as_deref(),
            Some("worker_distance")
        );
        assert_eq!(
            domain.entities[0].scalar_vars[0]
                .hooks
                .nearby_entity_distance_meter
                .as_deref(),
            Some("task_distance")
        );
        assert_eq!(
            domain.entities[0].scalar_vars[0]
                .hooks
                .construction_entity_order_key
                .as_deref(),
            Some("task_priority")
        );
        assert_eq!(
            domain.entities[0].scalar_vars[0]
                .hooks
                .construction_value_order_key
                .as_deref(),
            Some("worker_priority")
        );
        let list = &domain.entities[0].list_vars[0];
        assert_eq!(list.field, "worker_order");
        assert_eq!(list.element_collection, "workers");
        assert_eq!(
            list.metadata.distance_meter.as_deref(),
            Some("cross_distance")
        );
        assert_eq!(
            list.metadata.intra_distance_meter.as_deref(),
            Some("intra_distance")
        );
        assert_eq!(list.metadata.route_hooks.as_deref(), Some("route_hooks"));
        assert_eq!(
            list.metadata.savings_hooks.as_deref(),
            Some("savings_hooks")
        );
        assert_eq!(
            list.metadata.savings_metric_class_fn.as_deref(),
            Some("metric_class")
        );
        assert_eq!(
            list.metadata.element_owner_fn.as_deref(),
            Some("fixed_owner")
        );
        assert_eq!(
            list.metadata.construction_element_order_key.as_deref(),
            Some("worker_order")
        );
        assert_eq!(
            list.metadata.precedence_duration_fn.as_deref(),
            Some("worker_duration")
        );
        assert_eq!(
            list.metadata.precedence_successors_fn.as_deref(),
            Some("worker_successors")
        );
        assert_eq!(
            list.metadata.solution_trait.as_deref(),
            Some("RouteSolution")
        );
        assert_eq!(domain.facts[0].item_type, "Worker");

        std::env::set_current_dir(old).unwrap();
    }

    #[test]
    fn parses_simple_type_alias_collections() {
        let _guard = crate::test_support::lock_cwd();
        let temp = tempdir().unwrap();
        let old = std::env::current_dir().unwrap();
        std::env::set_current_dir(temp.path()).unwrap();
        fs::create_dir_all("src/domain").unwrap();
        fs::write(
            "src/domain/mod.rs",
            r#"solverforge::planning_model! {
    root = "src/domain";

    mod route;
    mod visit;
    mod plan;

    pub use route::VehicleRoute;
    pub use visit::Visit;
    pub use plan::Plan;
}
"#,
        )
        .unwrap();
        fs::write(
            "src/domain/route.rs",
            r#"use solverforge::prelude::*;

#[planning_entity]
pub struct Route {
    #[planning_id]
    pub id: usize,

    #[planning_list_variable(element_collection = "visits")]
    pub visits: Vec<usize>,
}

pub type VehicleRoute = Route;
"#,
        )
        .unwrap();
        fs::write(
            "src/domain/visit.rs",
            r#"use solverforge::prelude::*;

#[problem_fact]
pub struct Visit {
    #[planning_id]
    pub id: usize,
}
"#,
        )
        .unwrap();
        fs::write(
            "src/domain/plan.rs",
            r#"use solverforge::prelude::*;
use super::{VehicleRoute, Visit};

#[planning_solution]
pub struct Plan {
    #[problem_fact_collection]
    pub visits: Vec<Visit>,
    #[planning_entity_collection]
    pub routes: Vec<VehicleRoute>,
    #[planning_score]
    pub score: Option<HardSoftScore>,
}
"#,
        )
        .unwrap();

        let domain = parse_domain().unwrap();
        assert_eq!(domain.entities[0].item_type, "VehicleRoute");
        assert_eq!(domain.entities[0].list_vars[0].field, "visits");
        assert_eq!(
            find_file_for_type(Path::new("src/domain"), "VehicleRoute").unwrap(),
            Path::new("src/domain/route.rs")
        );

        std::env::set_current_dir(old).unwrap();
    }
}
