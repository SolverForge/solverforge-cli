use std::collections::BTreeSet;

use crate::managed_block;

const CONSTRAINT_MODULES_BLOCK: &str = "constraint-modules";
const CONSTRAINT_CALLS_BLOCK: &str = "constraint-calls";
const MAX_CONSTRAINT_TUPLE_ARITY: usize = 12;

#[derive(Debug, Clone)]
enum ConstraintExpr {
    Call(String),
    Tuple(Vec<ConstraintExpr>),
}

#[derive(Debug, Clone)]
struct ConstraintSurface {
    modules: Vec<String>,
    calls: Vec<String>,
}

/// Rewrites `src/constraints/mod.rs` using the managed module and tuple blocks.
pub(crate) fn rewrite_mod(src: &str, name: &str) -> Result<String, String> {
    let mut surface = ConstraintSurface::parse(src)?;
    if !surface.modules.iter().any(|existing| existing == name) {
        surface.modules.push(name.to_string());
    }

    if !surface.calls.iter().any(|existing| existing == name) {
        surface.calls.push(name.to_string());
    }

    let src = managed_block::replace_block(
        src,
        CONSTRAINT_MODULES_BLOCK,
        &render_constraint_modules(&surface.modules),
    )?;
    managed_block::replace_block(
        &src,
        CONSTRAINT_CALLS_BLOCK,
        &render_constraint_calls(&surface.calls),
    )
}

pub(crate) fn remove_constraint_from_source(src: &str, name: &str) -> Result<String, String> {
    let mut surface = ConstraintSurface::parse(src)?;
    surface.modules.retain(|existing| existing != name);

    surface.calls.retain(|existing| existing != name);

    let src = managed_block::replace_block(
        src,
        CONSTRAINT_MODULES_BLOCK,
        &render_constraint_modules(&surface.modules),
    )?;
    managed_block::replace_block(
        &src,
        CONSTRAINT_CALLS_BLOCK,
        &render_constraint_calls(&surface.calls),
    )
}

pub(crate) fn validate_constraint_mod_source(src: &str) -> Result<Vec<String>, String> {
    Ok(ConstraintSurface::parse(src)?.modules)
}

fn parse_constraint_modules(src: &str) -> Result<Vec<String>, String> {
    let block = managed_block::read_block(src, CONSTRAINT_MODULES_BLOCK)?;
    let mut modules = Vec::new();
    let mut seen = BTreeSet::new();
    for line in block.lines().map(str::trim).filter(|line| !line.is_empty()) {
        let module = line
            .strip_prefix("mod ")
            .and_then(|value| value.strip_suffix(';'))
            .ok_or_else(|| {
                format!(
                    "unsupported line in managed constraint block '{CONSTRAINT_MODULES_BLOCK}': {line}"
                )
            })?;
        if !seen.insert(module.to_string()) {
            return Err(format!(
                "duplicate constraint module '{module}' in managed block '{CONSTRAINT_MODULES_BLOCK}'"
            ));
        }
        modules.push(module.to_string());
    }
    Ok(modules)
}

fn parse_constraint_calls(src: &str) -> Result<Vec<String>, String> {
    let block = managed_block::read_block(src, CONSTRAINT_CALLS_BLOCK)?;
    let trimmed = block.trim();
    if trimmed.is_empty() {
        return Ok(Vec::new());
    }
    let mut parser = ConstraintCallParser::new(trimmed);
    let calls = parser.parse_calls()?;
    parser.finish()?;
    let mut seen = BTreeSet::new();
    for call in &calls {
        if !seen.insert(call.clone()) {
            return Err(format!(
                "duplicate constraint call '{call}' in managed block '{CONSTRAINT_CALLS_BLOCK}'"
            ));
        }
    }
    Ok(calls)
}

fn render_constraint_modules(modules: &[String]) -> String {
    modules
        .iter()
        .map(|module| format!("mod {module};"))
        .collect::<Vec<_>>()
        .join("\n")
}

fn render_constraint_calls(calls: &[String]) -> String {
    if calls.is_empty() {
        return "        ()".to_string();
    }

    let tree = build_constraint_tree(
        &calls
            .iter()
            .cloned()
            .map(ConstraintExpr::Call)
            .collect::<Vec<_>>(),
    );
    render_constraint_expr(&tree, 8, false).join("\n")
}

fn build_constraint_tree(exprs: &[ConstraintExpr]) -> ConstraintExpr {
    if exprs.is_empty() {
        return ConstraintExpr::Tuple(Vec::new());
    }

    if exprs.len() <= MAX_CONSTRAINT_TUPLE_ARITY {
        return ConstraintExpr::Tuple(exprs.to_vec());
    }

    let grouped = exprs
        .chunks(MAX_CONSTRAINT_TUPLE_ARITY)
        .map(build_constraint_tree)
        .collect::<Vec<_>>();
    build_constraint_tree(&grouped)
}

fn render_constraint_expr(
    expr: &ConstraintExpr,
    indent: usize,
    as_tuple_item: bool,
) -> Vec<String> {
    let pad = " ".repeat(indent);
    match expr {
        ConstraintExpr::Call(module) => vec![format!(
            "{pad}{module}::constraint(){}",
            if as_tuple_item { "," } else { "" }
        )],
        ConstraintExpr::Tuple(items) => {
            if items.is_empty() {
                return vec![format!("{pad}(){}", if as_tuple_item { "," } else { "" })];
            }

            let mut lines = vec![format!("{pad}(")];
            for item in items {
                lines.extend(render_constraint_expr(item, indent + 4, true));
            }
            lines.push(format!("{pad}){}", if as_tuple_item { "," } else { "" }));
            lines
        }
    }
}

fn is_simple_ident(value: &str) -> bool {
    !value.is_empty()
        && value
            .chars()
            .all(|ch| ch == '_' || ch.is_ascii_alphanumeric())
}

impl ConstraintSurface {
    fn parse(src: &str) -> Result<Self, String> {
        let modules = parse_constraint_modules(src)?;
        let calls = parse_constraint_calls(src)?;

        for module in &modules {
            if !calls.iter().any(|call| call == module) {
                return Err(format!(
                    "managed constraint block declares module '{module}' but '{CONSTRAINT_CALLS_BLOCK}' does not invoke it"
                ));
            }
        }
        for call in &calls {
            if !modules.iter().any(|module| module == call) {
                return Err(format!(
                    "managed constraint block invokes undeclared module '{call}' in '{CONSTRAINT_CALLS_BLOCK}'"
                ));
            }
        }
        if modules != calls {
            return Err(format!(
                "managed constraint modules and '{CONSTRAINT_CALLS_BLOCK}' must use the same order"
            ));
        }

        Ok(Self { modules, calls })
    }
}

struct ConstraintCallParser<'a> {
    src: &'a str,
    pos: usize,
}

impl<'a> ConstraintCallParser<'a> {
    fn new(src: &'a str) -> Self {
        Self { src, pos: 0 }
    }

    fn parse_calls(&mut self) -> Result<Vec<String>, String> {
        self.skip_ws();
        if self.peek_char() == Some('(') {
            self.parse_tuple()
        } else {
            Ok(vec![self.parse_call()?])
        }
    }

    fn finish(&mut self) -> Result<(), String> {
        self.skip_ws();
        if self.pos == self.src.len() {
            Ok(())
        } else {
            Err(format!(
                "unsupported constraint call in managed block '{CONSTRAINT_CALLS_BLOCK}': {}",
                &self.src[self.pos..]
            ))
        }
    }

    fn parse_tuple(&mut self) -> Result<Vec<String>, String> {
        self.expect_char('(')?;
        self.skip_ws();
        if self.peek_char() == Some(')') {
            self.pos += 1;
            return Ok(Vec::new());
        }

        let mut calls = Vec::new();
        loop {
            self.skip_ws();
            let mut inner = if self.peek_char() == Some('(') {
                self.parse_tuple()?
            } else {
                vec![self.parse_call()?]
            };
            calls.append(&mut inner);
            self.skip_ws();
            match self.peek_char() {
                Some(',') => {
                    self.pos += 1;
                    self.skip_ws();
                    if self.peek_char() == Some(')') {
                        self.pos += 1;
                        break;
                    }
                }
                Some(')') => {
                    self.pos += 1;
                    break;
                }
                _ => {
                    return Err(format!(
                        "unsupported constraint call in managed block '{CONSTRAINT_CALLS_BLOCK}': {}",
                        &self.src[self.pos..]
                    ));
                }
            }
        }

        Ok(calls)
    }

    fn parse_call(&mut self) -> Result<String, String> {
        let module = self.parse_ident()?;
        self.expect_str("::constraint")?;
        self.skip_ws();
        self.expect_char('(')?;
        self.skip_ws();
        self.expect_char(')')?;
        Ok(module)
    }

    fn parse_ident(&mut self) -> Result<String, String> {
        let start = self.pos;
        while let Some(ch) = self.peek_char() {
            if ch == '_' || ch.is_ascii_alphanumeric() {
                self.pos += ch.len_utf8();
            } else {
                break;
            }
        }

        let ident = &self.src[start..self.pos];
        if is_simple_ident(ident) {
            Ok(ident.to_string())
        } else {
            Err(format!(
                "unsupported constraint call in managed block '{CONSTRAINT_CALLS_BLOCK}': {}",
                &self.src[start..]
            ))
        }
    }

    fn expect_str(&mut self, expected: &str) -> Result<(), String> {
        if self.src[self.pos..].starts_with(expected) {
            self.pos += expected.len();
            Ok(())
        } else {
            Err(format!(
                "unsupported constraint call in managed block '{CONSTRAINT_CALLS_BLOCK}': {}",
                &self.src[self.pos..]
            ))
        }
    }

    fn expect_char(&mut self, expected: char) -> Result<(), String> {
        match self.peek_char() {
            Some(ch) if ch == expected => {
                self.pos += ch.len_utf8();
                Ok(())
            }
            _ => Err(format!(
                "unsupported constraint call in managed block '{CONSTRAINT_CALLS_BLOCK}': {}",
                &self.src[self.pos..]
            )),
        }
    }

    fn peek_char(&self) -> Option<char> {
        self.src[self.pos..].chars().next()
    }

    fn skip_ws(&mut self) {
        while matches!(self.peek_char(), Some(ch) if ch.is_whitespace()) {
            self.pos += self.peek_char().unwrap().len_utf8();
        }
    }
}
