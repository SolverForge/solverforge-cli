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

/// Keeps the symlink-dependent tests typechecking on every platform; they only
/// execute where the POSIX installer runs.
fn make_symlink(target: &Path, link: &Path) {
    #[cfg(unix)]
    std::os::unix::fs::symlink(target, link).expect("create symlink");
    #[cfg(windows)]
    std::os::windows::fs::symlink_dir(target, link).expect("create symlink");
    #[cfg(not(any(unix, windows)))]
    {
        let _ = (target, link);
        panic!("symlinks are unsupported on this platform");
    }
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

#[test]
fn three_harness_per_harness_requires_force_and_force_installs_all_copies() {
    let tmp = tempfile::tempdir().expect("temp dir");
    let home = tmp.path();

    let refused = run(
        home,
        &[
            "--agent", "opencode", "--agent", "claude", "--agent", "codex",
        ],
    );
    assert!(
        !refused.status.success(),
        "duplicate discovery must be refused without --force"
    );
    assert!(
        !home.join(".claude/skills").exists() && !home.join(".config/opencode/skills").exists(),
        "a refused install must not create anything"
    );

    let forced = run(
        home,
        &[
            "--agent",
            "opencode",
            "--agent",
            "claude",
            "--agent",
            "codex",
            "--layout",
            "per-harness",
            "--force",
        ],
    );
    assert!(
        forced.status.success(),
        "{}",
        String::from_utf8_lossy(&forced.stderr)
    );
    assert!(
        String::from_utf8_lossy(&forced.stderr).contains("warning: opencode scans"),
        "forced duplicate discovery must warn: {}",
        String::from_utf8_lossy(&forced.stderr)
    );
    for dir in [
        home.join(".config/opencode/skills"),
        home.join(".claude/skills"),
        home.join(".agents/skills"),
    ] {
        assert!(
            dir.join("solverforge-modeling/SKILL.md").is_file()
                && dir.join("solverforge-ui/SKILL.md").is_file(),
            "forced per-harness install must populate {}",
            dir.display()
        );
    }
}

#[test]
fn three_harness_covering_is_refused_with_guidance() {
    let tmp = tempfile::tempdir().expect("temp dir");
    let home = tmp.path();

    let refused = run(
        home,
        &[
            "--agent", "opencode", "--agent", "claude", "--agent", "codex", "--layout", "covering",
        ],
    );
    assert!(!refused.status.success());
    let stderr = String::from_utf8_lossy(&refused.stderr);
    assert!(
        stderr.contains("no duplicate-free placement")
            && stderr.contains("--layout per-harness --force"),
        "the refusal must guide to per-harness --force: {stderr}"
    );
    assert!(
        !home.join(".claude/skills").exists(),
        "a refused install must not create anything"
    );
}

#[test]
fn only_flag_is_rejected() {
    let tmp = tempfile::tempdir().expect("temp dir");
    let home = tmp.path();

    let attempt = run(home, &["--only", "opencode"]);
    assert!(!attempt.status.success(), "--only must no longer exist");
    let stderr = String::from_utf8_lossy(&attempt.stderr);
    assert!(
        stderr.contains("unknown option '--only'"),
        "unexpected error: {stderr}"
    );
    assert!(
        !home.join(".config/opencode/skills").exists(),
        "a rejected invocation must not create anything"
    );
}

#[test]
fn no_agent_without_tty_fails_without_creating_directories() {
    let tmp = tempfile::tempdir().expect("temp dir");
    let home = tmp.path();

    let attempt = run(home, &[]);
    assert!(!attempt.status.success());
    let stderr = String::from_utf8_lossy(&attempt.stderr);
    assert!(
        stderr.contains("no --agent given"),
        "unexpected error: {stderr}"
    );
    assert!(
        !home.join(".config/opencode/skills").exists()
            && !home.join(".claude/skills").exists()
            && !home.join(".agents/skills").exists(),
        "a refused invocation must not create anything"
    );
}

fn link_receipt(home: &Path, skill: &str) -> PathBuf {
    home.join(format!(".claude/skills/.solverforge-skill-link-{skill}"))
}

#[test]
fn replaced_link_with_stale_receipt_is_preserved() {
    let tmp = tempfile::tempdir().expect("temp dir");
    let home = tmp.path();
    let foreign_target = tmp.path().join("foreign");
    fs::create_dir_all(&foreign_target).expect("create foreign target");

    let linked = run(home, &["--agent", "claude", "--link"]);
    assert!(linked.status.success());
    let entry = home.join(".claude/skills/solverforge-modeling");
    fs::remove_file(&entry).expect("remove managed link");
    make_symlink(&foreign_target, &entry);

    let listed = run(home, &["--agent", "claude", "--list"]);
    assert!(
        String::from_utf8_lossy(&listed.stdout).contains("stale-rcpt"),
        "a replaced link must list as stale, not managed"
    );

    let attempted = run(home, &["--agent", "claude"]);
    assert!(
        !attempted.status.success(),
        "a replaced link must not be reinstalled over"
    );

    let removed = run(home, &["--agent", "claude", "--uninstall"]);
    assert!(removed.status.success());
    assert!(
        fs::symlink_metadata(&entry)
            .expect("replacement link survives")
            .file_type()
            .is_symlink()
            && fs::read_link(&entry).expect("readlink") == foreign_target,
        "uninstall must not delete a link it no longer owns"
    );
    assert!(
        !link_receipt(home, "solverforge-modeling").exists(),
        "the stale receipt must be removed on uninstall"
    );
}

#[test]
fn altered_receipt_is_not_trusted() {
    let tmp = tempfile::tempdir().expect("temp dir");
    let home = tmp.path();

    let linked = run(home, &["--agent", "claude", "--link"]);
    assert!(linked.status.success());
    let entry = home.join(".claude/skills/solverforge-modeling");
    let receipt = link_receipt(home, "solverforge-modeling");
    let tampered = fs::read_to_string(&receipt)
        .expect("read receipt")
        .lines()
        .map(|line| {
            if line.starts_with("target=") {
                "target=/somewhere/else".to_string()
            } else {
                line.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n");
    fs::write(&receipt, tampered).expect("tamper receipt");

    let removed = run(home, &["--agent", "claude", "--uninstall"]);
    assert!(removed.status.success());
    assert!(
        fs::symlink_metadata(&entry).is_ok(),
        "a link whose receipt target was altered must survive uninstall"
    );
}

#[test]
fn symlinked_receipt_is_not_trusted() {
    let tmp = tempfile::tempdir().expect("temp dir");
    let home = tmp.path();

    let linked = run(home, &["--agent", "claude", "--link"]);
    assert!(linked.status.success());
    let entry = home.join(".claude/skills/solverforge-modeling");
    let receipt = link_receipt(home, "solverforge-modeling");
    fs::remove_file(&receipt).expect("remove receipt");
    make_symlink(&tmp.path().join("elsewhere"), &receipt);

    let removed = run(home, &["--agent", "claude", "--uninstall"]);
    assert!(removed.status.success());
    assert!(
        fs::symlink_metadata(&entry).is_ok(),
        "a link with a symlinked receipt must survive uninstall"
    );
}

#[test]
fn owned_dangling_link_is_still_removed() {
    let tmp = tempfile::tempdir().expect("temp dir");
    let home = tmp.path();

    let linked = run(home, &["--agent", "claude", "--link"]);
    assert!(linked.status.success());
    let entry = home.join(".claude/skills/solverforge-modeling");
    let dangling = tmp.path().join("nowhere-at-all");
    let receipt = link_receipt(home, "solverforge-modeling");
    fs::remove_file(&entry).expect("remove link");
    make_symlink(&dangling, &entry);
    let rewritten = fs::read_to_string(&receipt)
        .expect("read receipt")
        .lines()
        .map(|line| {
            if line.starts_with("target=") {
                format!("target={}", dangling.display())
            } else {
                line.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n");
    fs::write(&receipt, rewritten).expect("align receipt with the dangling link");

    let removed = run(home, &["--agent", "claude", "--uninstall"]);
    assert!(
        removed.status.success() && String::from_utf8_lossy(&removed.stdout).contains("removed"),
        "a self-consistent managed link must stay owned even while dangling"
    );
    assert!(fs::symlink_metadata(&entry).is_err());
}

#[test]
fn uninstall_after_link_deletion_removes_only_the_receipt() {
    let tmp = tempfile::tempdir().expect("temp dir");
    let home = tmp.path();

    let linked = run(home, &["--agent", "claude", "--link"]);
    assert!(linked.status.success());
    let entry = home.join(".claude/skills/solverforge-modeling");
    fs::remove_file(&entry).expect("remove link");

    let listed = run(home, &["--agent", "claude", "--list"]);
    assert!(
        String::from_utf8_lossy(&listed.stdout).contains("stale-rcpt"),
        "a deleted link must list as stale"
    );

    let removed = run(home, &["--agent", "claude", "--uninstall"]);
    assert!(removed.status.success());
    assert!(fs::symlink_metadata(&entry).is_err());
    assert!(
        !link_receipt(home, "solverforge-modeling").exists(),
        "only the installer-owned receipt may be removed"
    );
}

#[test]
fn explicit_dir_with_spaces_works() {
    let tmp = tempfile::tempdir().expect("temp dir");
    let home = tmp.path();
    let dir = home.join("my skills dir");

    let result = run(
        home,
        &[
            "--dir",
            dir.to_str().unwrap(),
            "--skill",
            "solverforge-modeling",
        ],
    );
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(dir.join("solverforge-modeling/SKILL.md").is_file());
}

#[test]
fn make_wrapper_forwards_args() {
    let tmp = tempfile::tempdir().expect("temp dir");
    let home = tmp.path();

    let output = Command::new("make")
        .arg("install-skill")
        .arg("ARGS=--agent claude --skill solverforge-modeling")
        .env("HOME", home)
        .env("XDG_CONFIG_HOME", home.join(".config"))
        .current_dir(repo_root())
        .output()
        .expect("run make install-skill");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        home.join(".claude/skills/solverforge-modeling/SKILL.md")
            .is_file(),
        "make must forward ARGS to the installer"
    );
    assert!(
        !home.join(".claude/skills/solverforge-ui").exists(),
        "skill selection must survive the make wrapper"
    );

    let refused = Command::new("make")
        .arg("install-skill")
        .env("HOME", home)
        .env("XDG_CONFIG_HOME", home.join(".config"))
        .current_dir(repo_root())
        .output()
        .expect("run make install-skill without args");
    assert!(
        !refused.status.success(),
        "make without ARGS must surface the installer's explicit-agent refusal in non-TTY runs"
    );
}
