use std::ffi::OsStr;
use std::fs;
use std::path::Path;

use super::{project_edition, SourceSnapshot};

const UNFORMATTED: &str = "use crate::domain::{Plan, Shift, Employee};\nfn f() { let x = 1; }\n";
const FORMATTED: &str =
    "use crate::domain::{Employee, Plan, Shift};\nfn f() {\n    let x = 1;\n}\n";

fn write_project(root: &Path) {
    fs::create_dir_all(root.join("src/constraints")).unwrap();
    fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"demo\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    )
    .unwrap();
}

#[test]
fn format_changed_formats_only_sources_written_after_the_snapshot() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    write_project(root);
    let untouched = root.join("src/lib.rs");
    fs::write(&untouched, UNFORMATTED).unwrap();
    let rewritten = root.join("src/constraints/mod.rs");
    fs::write(&rewritten, "mod b;\n").unwrap();

    let snapshot = SourceSnapshot::capture(root);
    let created = root.join("src/constraints/new_rule.rs");
    fs::write(&created, UNFORMATTED).unwrap();
    fs::write(&rewritten, "mod b;\nmod a;\n").unwrap();
    snapshot.format_changed_with(OsStr::new("rustfmt"));

    assert_eq!(fs::read_to_string(&created).unwrap(), FORMATTED);
    assert_eq!(fs::read_to_string(&rewritten).unwrap(), "mod a;\nmod b;\n");
    assert_eq!(
        fs::read_to_string(&untouched).unwrap(),
        UNFORMATTED,
        "sources the command did not write must stay as the user left them"
    );
}

#[test]
fn format_changed_leaves_files_alone_when_rustfmt_is_missing() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    write_project(root);
    let created = root.join("src/lib.rs");

    let snapshot = SourceSnapshot::empty(root);
    fs::write(&created, UNFORMATTED).unwrap();
    snapshot.format_changed_with(OsStr::new("/nonexistent/solverforge-test-rustfmt"));

    assert_eq!(fs::read_to_string(&created).unwrap(), UNFORMATTED);
}

#[test]
fn format_changed_leaves_unparseable_sources_unchanged() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    write_project(root);
    let broken = root.join("src/lib.rs");

    let snapshot = SourceSnapshot::empty(root);
    fs::write(&broken, "fn f( {\n").unwrap();
    snapshot.format_changed_with(OsStr::new("rustfmt"));

    assert_eq!(fs::read_to_string(&broken).unwrap(), "fn f( {\n");
}

#[test]
fn project_edition_follows_the_manifest() {
    let tmp = tempfile::tempdir().unwrap();
    assert_eq!(project_edition(tmp.path()), "2021");
    fs::write(
        tmp.path().join("Cargo.toml"),
        "[package]\nname = \"demo\"\nedition = \"2024\"\n",
    )
    .unwrap();
    assert_eq!(project_edition(tmp.path()), "2024");
}
