//! Enforces that module roots (`mod.rs`, `lib.rs`) hold no code.
//!
//! A module root may declare child modules, re-export or import items, and,
//! for the domain manifest, invoke `solverforge::planning_model!`. Functions,
//! types, impls, constants and inline module bodies belong in their own files.

use std::fs;
use std::path::{Path, PathBuf};

use syn::{Item, Meta};

/// Rejects any item in `src` other than a module declaration, a `use`, an
/// `extern crate`, or a `planning_model!` invocation.
pub(crate) fn ensure_declarations_only(src: &str) -> Result<(), String> {
    let file = syn::parse_file(src).map_err(|err| format!("failed to parse: {err}"))?;
    for attr in &file.attrs {
        if !is_documentation(&attr.meta) {
            return Err("module roots may not carry inner attributes other than docs".to_string());
        }
    }
    for item in &file.items {
        let allowed = match item {
            Item::Mod(module) => module.content.is_none(),
            Item::Use(_) | Item::ExternCrate(_) => true,
            Item::Macro(invocation) => invocation
                .mac
                .path
                .segments
                .last()
                .is_some_and(|segment| segment.ident == "planning_model"),
            _ => false,
        };
        if !allowed {
            return Err(format!(
                "module roots may hold only module declarations and re-exports; move {} into its own file",
                describe(item)
            ));
        }
    }
    Ok(())
}

/// Every `mod.rs` and `lib.rs` under `src_dir`, sorted.
pub(crate) fn module_roots(src_dir: &Path) -> Vec<PathBuf> {
    let mut roots = Vec::new();
    collect(src_dir, &mut roots);
    roots.sort();
    roots
}

fn collect(dir: &Path, roots: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect(&path, roots);
        } else if matches!(
            path.file_name().and_then(|name| name.to_str()),
            Some("mod.rs" | "lib.rs")
        ) {
            roots.push(path);
        }
    }
}

fn is_documentation(meta: &Meta) -> bool {
    meta.path().is_ident("doc")
}

fn describe(item: &Item) -> String {
    match item {
        Item::Fn(function) => format!("fn `{}`", function.sig.ident),
        Item::Struct(definition) => format!("struct `{}`", definition.ident),
        Item::Enum(definition) => format!("enum `{}`", definition.ident),
        Item::Trait(definition) => format!("trait `{}`", definition.ident),
        Item::Const(definition) => format!("const `{}`", definition.ident),
        Item::Static(definition) => format!("static `{}`", definition.ident),
        Item::Type(definition) => format!("type `{}`", definition.ident),
        Item::Mod(module) => format!("the body of `mod {}`", module.ident),
        Item::Impl(_) => "the impl block".to_string(),
        Item::Macro(_) => "the macro invocation".to_string(),
        _ => "the item".to_string(),
    }
}

#[cfg(test)]
#[path = "module_manifest_tests.rs"]
mod tests;
