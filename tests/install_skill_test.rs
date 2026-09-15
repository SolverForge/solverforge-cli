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
