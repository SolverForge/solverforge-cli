//! Every skill's `SKILL.md` must link every reference file it ships, and must
//! not link a reference that does not exist. This guards the `references/`
//! trees as skills are added or moved.

use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;

fn skills_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("skills")
}

fn linked_references(source: &str) -> BTreeSet<String> {
    let mut linked = BTreeSet::new();
    for token in source.split("references/").skip(1) {
        let mut name: String = token
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
            .collect();
        if name.ends_with('.') {
            name.pop();
        }
        if name.ends_with(".md") {
            linked.insert(name);
        }
    }
    linked
}

fn present_references(dir: &std::path::Path) -> BTreeSet<String> {
    let mut present = BTreeSet::new();
    let Ok(entries) = fs::read_dir(dir) else {
        return present;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) == Some("md") {
            present.insert(path.file_name().unwrap().to_string_lossy().into_owned());
        }
    }
    present
}

#[test]
fn every_skill_reference_resolves_and_is_linked() {
    let root = skills_root();
    let mut saw_skill = false;

    for entry in fs::read_dir(&root).expect("skills dir") {
        let dir = entry.expect("entry").path();
        let skill = dir.join("SKILL.md");
        if !skill.is_file() {
            continue;
        }
        saw_skill = true;
        let source = fs::read_to_string(&skill).expect("read SKILL.md");
        let linked = linked_references(&source);
        let present = present_references(&dir.join("references"));

        for name in &linked {
            assert!(
                present.contains(name),
                "{} links missing reference {name}",
                skill.display()
            );
        }
        for name in &present {
            assert!(
                linked.contains(name),
                "{} is not linked from {}",
                dir.join("references").join(name).display(),
                skill.display()
            );
        }
    }

    assert!(saw_skill, "no skills found under {}", root.display());
}
