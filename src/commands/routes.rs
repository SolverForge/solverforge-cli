use std::fs;
use std::path::Path;

use crate::app_spec;
use crate::error::{CliError, CliResult};
use crate::output;

pub fn run() -> CliResult {
    if Path::new("solverforge.app.toml").exists() {
        let spec = app_spec::load()?;
        if spec.app.shell == "cli" {
            return Err(CliError::with_hint(
                "`solverforge routes` is not available for CLI-shell projects",
                "CLI-shell projects do not generate Axum routes",
            ));
        }
        if spec.app.shell == "mcp" {
            return Err(CliError::with_hint(
                "`solverforge routes` is not available for MCP-shell projects",
                "MCP-shell projects expose solver tools over MCP, not Axum REST routes",
            ));
        }
    }

    let candidates = ["src/api/routes.rs", "src/api/mod.rs", "src/api.rs"];

    let (source_path, content) = candidates
        .iter()
        .find_map(|p| {
            let path = Path::new(p);
            if path.exists() {
                fs::read_to_string(path).ok().map(|c| (*p, c))
            } else {
                None
            }
        })
        .ok_or(CliError::NotInProject {
            missing: "src/api/routes.rs or src/api/mod.rs",
        })?;

    output::print_status("routes", &format!("parsing {}", source_path));
    println!();

    let routes = parse_routes(&content);

    if routes.is_empty() {
        output::print_dim("  No routes found.");
        return Ok(());
    }

    let method_w = routes
        .iter()
        .map(|(m, _, _)| m.len())
        .max()
        .unwrap_or(6)
        .max(6);
    let path_w = routes
        .iter()
        .map(|(_, p, _)| p.len())
        .max()
        .unwrap_or(4)
        .max(4);
    let handler_w = routes
        .iter()
        .map(|(_, _, h)| h.len())
        .max()
        .unwrap_or(7)
        .max(7);

    println!(
        "  {:<method_w$}  {:<path_w$}  {:<handler_w$}",
        "METHOD",
        "PATH",
        "HANDLER",
        method_w = method_w,
        path_w = path_w,
        handler_w = handler_w,
    );
    println!(
        "  {:-<method_w$}  {:-<path_w$}  {:-<handler_w$}",
        "",
        "",
        "",
        method_w = method_w,
        path_w = path_w,
        handler_w = handler_w,
    );

    for (method, path, handler) in &routes {
        println!(
            "  {:<method_w$}  {:<path_w$}  {:<handler_w$}",
            method,
            path,
            handler,
            method_w = method_w,
            path_w = path_w,
            handler_w = handler_w,
        );
    }
    println!();

    Ok(())
}

/// Lists every `(METHOD, path, handler)` registered through `.route(...)`.
///
/// Each `.route(` call is matched to its balanced closing parenthesis, so the
/// call may span several lines, and every method of a chained method router
/// (`get(a).delete(b)`) is reported in source order.
fn parse_routes(content: &str) -> Vec<(String, String, String)> {
    let source = strip_line_comments(content);
    let mut routes = Vec::new();
    let mut cursor = 0;

    while let Some(offset) = source[cursor..].find(".route(") {
        let open = cursor + offset + ".route".len();
        let Some(close) = matching_paren(&source, open) else {
            break;
        };
        let args = &source[open + 1..close];
        cursor = close + 1;

        let Some((path, method_router)) = split_route_args(args) else {
            continue;
        };
        for (method, handler) in method_router_entries(method_router) {
            routes.push((method, path.clone(), handler));
        }
    }

    routes
}

const HTTP_METHODS: [&str; 9] = [
    "get", "post", "put", "delete", "patch", "head", "options", "trace", "any",
];

/// Splits `"/path", <method router>` into the path literal and the router
/// expression.
fn split_route_args(args: &str) -> Option<(String, &str)> {
    let start = args.find('"')? + 1;
    let end = start + args[start..].find('"')?;
    let path = args[start..end].to_string();
    let rest = args[end + 1..].trim_start();
    let rest = rest.strip_prefix(',')?;
    Some((path, rest))
}

/// Finds every `method(handler)` call in a method-router expression, whether
/// it is the leading call (`get(..)`, `routing::get(..)`) or a chained one
/// (`.delete(..)`).
fn method_router_entries(expr: &str) -> Vec<(String, String)> {
    let bytes = expr.as_bytes();
    let mut entries = Vec::new();
    let mut i = 0;

    while i < bytes.len() {
        let c = bytes[i];
        if c == b'"' {
            i = skip_string(bytes, i);
            continue;
        }
        if !(c.is_ascii_alphabetic() || c == b'_') {
            i += 1;
            continue;
        }
        let ident_start = i;
        while i < bytes.len() && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') {
            i += 1;
        }
        let ident = &expr[ident_start..i];
        if !HTTP_METHODS.contains(&ident) || bytes.get(i) != Some(&b'(') {
            continue;
        }
        let Some(close) = matching_paren(expr, i) else {
            break;
        };
        let handler = expr[i + 1..close]
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");
        let handler = handler.trim_end_matches(',').trim().to_string();
        if !handler.is_empty() {
            entries.push((ident.to_uppercase(), handler));
        }
        i = close + 1;
    }

    entries
}

/// Returns the index of the `)` that closes the `(` at `open`, skipping string
/// literals.
fn matching_paren(source: &str, open: usize) -> Option<usize> {
    let bytes = source.as_bytes();
    let mut depth = 0usize;
    let mut i = open;
    while i < bytes.len() {
        match bytes[i] {
            b'"' => {
                i = skip_string(bytes, i);
                continue;
            }
            b'(' => depth += 1,
            b')' => {
                depth -= 1;
                if depth == 0 {
                    return Some(i);
                }
            }
            _ => {}
        }
        i += 1;
    }
    None
}

/// Returns the index just past the string literal starting at `start`.
fn skip_string(bytes: &[u8], start: usize) -> usize {
    let mut i = start + 1;
    while i < bytes.len() {
        match bytes[i] {
            b'\\' => i += 2,
            b'"' => return i + 1,
            _ => i += 1,
        }
    }
    bytes.len()
}

/// Blanks `//` comments so commented-out routes are not listed, leaving string
/// literals such as `"http://..."` intact.
fn strip_line_comments(content: &str) -> String {
    content
        .lines()
        .map(|line| {
            let bytes = line.as_bytes();
            let mut i = 0;
            while i < bytes.len() {
                match bytes[i] {
                    b'"' => i = skip_string(bytes, i),
                    b'/' if bytes.get(i + 1) == Some(&b'/') => return &line[..i],
                    _ => i += 1,
                }
            }
            line
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::parse_routes;

    fn route(method: &str, path: &str, handler: &str) -> (String, String, String) {
        (method.to_string(), path.to_string(), handler.to_string())
    }

    #[test]
    fn single_line_routes_keep_source_order() {
        let source = r#"
            Router::new()
                .route("/health", get(health))
                .route("/jobs", post(create_job))
        "#;
        assert_eq!(
            parse_routes(source),
            vec![
                route("GET", "/health", "health"),
                route("POST", "/jobs", "create_job"),
            ]
        );
    }

    #[test]
    fn chained_method_router_lists_every_method() {
        let source = r#".route("/jobs/{id}", get(get_job).delete(delete_job))"#;
        assert_eq!(
            parse_routes(source),
            vec![
                route("GET", "/jobs/{id}", "get_job"),
                route("DELETE", "/jobs/{id}", "delete_job"),
            ]
        );
    }

    #[test]
    fn chained_methods_are_listed_in_source_order() {
        let source = r#".route("/items", post(create_item).get(list_items))"#;
        assert_eq!(
            parse_routes(source),
            vec![
                route("POST", "/items", "create_item"),
                route("GET", "/items", "list_items"),
            ]
        );
    }

    #[test]
    fn multi_line_route_call_is_listed() {
        let source = r#"
            Router::new()
                .route(
                    "/recommendations/delivery-insertions",
                    post(recommend_delivery_insertions),
                )
                .route("/jobs/{id}/events", get(sse::events))
        "#;
        assert_eq!(
            parse_routes(source),
            vec![
                route(
                    "POST",
                    "/recommendations/delivery-insertions",
                    "recommend_delivery_insertions"
                ),
                route("GET", "/jobs/{id}/events", "sse::events"),
            ]
        );
    }

    #[test]
    fn multi_line_chained_router_lists_every_method() {
        let source = r#"
            .route(
                "/jobs/{id}",
                get(get_job)
                    .put(replace_job)
                    .delete(delete_job),
            )
        "#;
        assert_eq!(
            parse_routes(source),
            vec![
                route("GET", "/jobs/{id}", "get_job"),
                route("PUT", "/jobs/{id}", "replace_job"),
                route("DELETE", "/jobs/{id}", "delete_job"),
            ]
        );
    }

    #[test]
    fn handler_names_containing_method_words_are_not_split() {
        let source = r#".route("/budget", routing::get(get_budget))"#;
        assert_eq!(
            parse_routes(source),
            vec![route("GET", "/budget", "get_budget")]
        );
    }

    #[test]
    fn commented_out_routes_are_ignored() {
        let source = r#"
            // .route("/old", get(old))
            .route("/new", get(new_handler)) // replaces /old
        "#;
        assert_eq!(
            parse_routes(source),
            vec![route("GET", "/new", "new_handler")]
        );
    }
}
