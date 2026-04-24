pub(crate) const ENTITY_REQUIRED_BLOCKS: &[&str] = &["entity-variables", "entity-variable-init"];
pub(crate) const SOLUTION_REQUIRED_BLOCKS: &[&str] = &[
    "solution-imports",
    "solution-collections",
    "solution-constructor-params",
    "solution-constructor-init",
];

pub(crate) fn begin_marker(label: &str) -> String {
    format!("// @solverforge:begin {label}")
}

pub(crate) fn end_marker(label: &str) -> String {
    format!("// @solverforge:end {label}")
}

#[cfg(test)]
pub(crate) fn has_block(src: &str, label: &str) -> bool {
    find_block(src, label).is_ok()
}

pub(crate) fn read_block(src: &str, label: &str) -> Result<String, String> {
    let (lines, begin_idx, end_idx) = find_block(src, label)?;
    if end_idx <= begin_idx + 1 {
        return Ok(String::new());
    }

    Ok(lines[begin_idx + 1..end_idx].join("\n"))
}

pub(crate) fn replace_block(src: &str, label: &str, new_content: &str) -> Result<String, String> {
    let (lines, begin_idx, end_idx) = find_block(src, label)?;
    let mut out = Vec::new();
    out.extend(lines[..=begin_idx].iter().cloned());
    if !new_content.trim().is_empty() {
        out.extend(new_content.trim_end().lines().map(|line| line.to_string()));
    }
    out.extend(lines[end_idx..].iter().cloned());

    if out.is_empty() {
        Ok(String::new())
    } else {
        Ok(out.join("\n") + "\n")
    }
}

pub(crate) fn require_blocks(src: &str, labels: &[&str]) -> Result<(), String> {
    for label in labels {
        read_block(src, label)?;
    }
    Ok(())
}

#[cfg(test)]
pub(crate) fn block_with_content(label: &str, content: &str) -> String {
    let begin = begin_marker(label);
    let end = end_marker(label);
    if content.trim().is_empty() {
        format!("{begin}\n{end}")
    } else {
        format!("{begin}\n{}\n{end}", content.trim_end())
    }
}

fn find_block(src: &str, label: &str) -> Result<(Vec<String>, usize, usize), String> {
    let begin = begin_marker(label);
    let end = end_marker(label);
    let lines: Vec<String> = src.lines().map(|line| line.to_string()).collect();
    let begin_matches: Vec<usize> = lines
        .iter()
        .enumerate()
        .filter_map(|(idx, line)| (line.trim() == begin).then_some(idx))
        .collect();
    let end_matches: Vec<usize> = lines
        .iter()
        .enumerate()
        .filter_map(|(idx, line)| (line.trim() == end).then_some(idx))
        .collect();

    if begin_matches.len() != 1 || end_matches.len() != 1 {
        return Err(format!(
            "missing or duplicated managed block markers for '{label}'"
        ));
    }

    let begin_idx = begin_matches[0];
    let end_idx = end_matches[0];
    if end_idx <= begin_idx {
        return Err(format!("invalid managed block order for '{label}'"));
    }

    Ok((lines, begin_idx, end_idx))
}

#[cfg(test)]
mod tests {
    use super::{block_with_content, has_block, read_block, replace_block};

    #[test]
    fn detects_and_rewrites_managed_block() {
        let src = format!(
            "header\n{}\ntrailer\n",
            block_with_content("demo", "line_a\nline_b")
        );
        assert!(has_block(&src, "demo"));
        assert_eq!(read_block(&src, "demo").unwrap(), "line_a\nline_b");

        let rewritten = replace_block(&src, "demo", "line_c").unwrap();
        assert!(rewritten.contains("line_c"));
        assert!(!rewritten.contains("line_a"));
    }
}
