//! Behavioral tests for the agent-centric skill installer. They run the real
//! `scripts/install-skill` under an isolated HOME so no user state is touched.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn run(home: &Path, args: &[&str]) -> std::process::Output {
    Command::new("sh")
        .arg(repo_root().join("scripts/install-skill"))
        .args(args)
        .env("HOME", home)
        .env("XDG_CONFIG_HOME", home.join(".config"))
        .current_dir(repo_root())
        .output()
        .expect("run installer")
}

fn copy_tree(from: &Path, to: &Path) {
    if let Some(parent) = to.parent() {
        fs::create_dir_all(parent).expect("create parent");
    }
    let status = Command::new("cp")
        .arg("-R")
        .arg(from)
        .arg(to)
        .status()
        .expect("run cp");
    assert!(status.success(), "cp -R {}", from.display());
}

#[test]
fn default_installs_one_copy_per_selected_harness_and_refuses_duplicates() {
    let tmp = tempfile::tempdir().expect("temp dir");
    let home = tmp.path();

    let opencode = run(home, &["--agent", "opencode"]);
    assert!(
        opencode.status.success(),
        "{}",
        String::from_utf8_lossy(&opencode.stderr)
    );
    assert!(home
        .join(".config/opencode/skills/solverforge-modeling/SKILL.md")
        .is_file());
    assert!(home
        .join(".config/opencode/skills/solverforge-ui/SKILL.md")
        .is_file());
    assert!(
        !home.join(".agents/skills").exists(),
        "the installer must not assume ~/.agents"
    );

    let refused = run(home, &["--agent", "opencode", "--agent", "claude"]);
    assert!(
        !refused.status.success(),
        "per-harness opencode+claude duplicates discovery and must be refused"
    );

    let covering = run(
        home,
        &[
            "--agent", "opencode", "--agent", "claude", "--layout", "covering",
        ],
    );
    assert!(
        covering.status.success(),
        "{}",
        String::from_utf8_lossy(&covering.stderr)
    );
    assert!(home
        .join(".claude/skills/solverforge-modeling/SKILL.md")
        .is_file());
}

#[test]
fn link_and_uninstall_round_trip() {
    let tmp = tempfile::tempdir().expect("temp dir");
    let home = tmp.path();

    let linked = run(home, &["--agent", "claude", "--link"]);
    assert!(
        linked.status.success(),
        "{}",
        String::from_utf8_lossy(&linked.stderr)
    );
    let entry = home.join(".claude/skills/solverforge-modeling");
    assert!(
        fs::symlink_metadata(&entry)
            .expect("link exists")
            .file_type()
            .is_symlink(),
        "expected a symlink at {}",
        entry.display()
    );

    let removed = run(home, &["--agent", "claude", "--uninstall"]);
    assert!(
        removed.status.success(),
        "{}",
        String::from_utf8_lossy(&removed.stderr)
    );
    assert!(!entry.exists());
}

#[test]
fn skill_name_traversal_is_rejected() {
    let tmp = tempfile::tempdir().expect("temp dir");
    let home = tmp.path();
    let dest = home.join("dest");
    let escaped = home.join("skills/solverforge-modeling");

    let attempt = run(
        home,
        &[
            "--dir",
            dest.to_str().unwrap(),
            "--skill",
            "../skills/solverforge-modeling",
        ],
    );
    assert!(!attempt.status.success(), "traversal must be rejected");
    assert!(
        !escaped.exists(),
        "installer escaped the requested skills directory"
    );
}

#[test]
fn unknown_skill_name_is_rejected() {
    let tmp = tempfile::tempdir().expect("temp dir");
    let home = tmp.path();

    let attempt = run(home, &["--agent", "claude", "--skill", "does-not-exist"]);
    assert!(!attempt.status.success(), "unknown skill must be rejected");
    assert!(
        !home.join(".claude/skills").exists(),
        "a rejected install must not create anything"
    );
}

#[test]
fn explicit_skill_selection_installs_only_that_skill() {
    let tmp = tempfile::tempdir().expect("temp dir");
    let home = tmp.path();

    let result = run(
        home,
        &["--agent", "claude", "--skill", "solverforge-modeling"],
    );
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(home
        .join(".claude/skills/solverforge-modeling/SKILL.md")
        .is_file());
    assert!(
        !home.join(".claude/skills/solverforge-ui").exists(),
        "only the named skill should be installed"
    );
}

#[test]
fn manually_copied_skill_is_not_owned_by_the_installer() {
    let tmp = tempfile::tempdir().expect("temp dir");
    let home = tmp.path();
    let manual = home.join(".claude/skills/solverforge-modeling");
    copy_tree(&repo_root().join("skills/solverforge-modeling"), &manual);

    let listed = run(home, &["--agent", "claude", "--list"]);
    let listing = String::from_utf8_lossy(&listed.stdout);
    assert!(
        listing.contains("foreign"),
        "a hand-copied skill must list as foreign: {listing}"
    );

    let removed = run(home, &["--agent", "claude", "--uninstall"]);
    assert!(
        removed.status.success(),
        "{}",
        String::from_utf8_lossy(&removed.stderr)
    );
    assert!(
        manual.join("SKILL.md").is_file(),
        "uninstall must not delete a hand-copied skill"
    );
}

#[test]
fn copied_install_is_owned_and_uninstalls() {
    let tmp = tempfile::tempdir().expect("temp dir");
    let home = tmp.path();

    let installed = run(home, &["--agent", "claude"]);
    assert!(
        installed.status.success(),
        "{}",
        String::from_utf8_lossy(&installed.stderr)
    );
    let entry = home.join(".claude/skills/solverforge-modeling");
    assert!(
        entry.join(".solverforge-skill").is_file(),
        "install must record ownership in the installed copy"
    );

    let removed = run(home, &["--agent", "claude", "--uninstall"]);
    assert!(
        removed.status.success(),
        "{}",
        String::from_utf8_lossy(&removed.stderr)
    );
    assert!(!entry.exists(), "an owned install must be removed");
}

#[test]
fn reinstalling_an_owned_copy_reports_current() {
    let tmp = tempfile::tempdir().expect("temp dir");
    let home = tmp.path();

    let first = run(
        home,
        &["--agent", "claude", "--skill", "solverforge-modeling"],
    );
    assert!(
        first.status.success(),
        "{}",
        String::from_utf8_lossy(&first.stderr)
    );
    let second = run(
        home,
        &["--agent", "claude", "--skill", "solverforge-modeling"],
    );
    assert!(
        second.status.success(),
        "{}",
        String::from_utf8_lossy(&second.stderr)
    );
    let output = String::from_utf8_lossy(&second.stdout);
    assert!(
        output.contains("current"),
        "reinstalling an unchanged owned copy must be a no-op: {output}"
    );
}

#[test]
fn foreign_entries_are_left_untouched() {
    let tmp = tempfile::tempdir().expect("temp dir");
    let home = tmp.path();
    let foreign = home.join(".claude/skills/solverforge-modeling");
    fs::create_dir_all(&foreign).expect("create foreign dir");
    fs::write(foreign.join("SKILL.md"), "user file").expect("write user file");

    let attempt = run(home, &["--agent", "claude"]);
    assert!(
        !attempt.status.success(),
        "must refuse to overwrite an entry it did not install"
    );
    assert_eq!(
        fs::read_to_string(foreign.join("SKILL.md")).unwrap(),
        "user file"
    );
}
