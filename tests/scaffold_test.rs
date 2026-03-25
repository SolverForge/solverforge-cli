// Integration tests for project scaffolding.
//
// Some tests invoke `cargo check` inside a temp directory and therefore require a full Rust
// toolchain plus dependency resolution access.

use std::path::PathBuf;
use std::process::Command;

fn cli_command() -> Command {
    let mut command = Command::new("cargo");
    command.args([
        "run",
        "--quiet",
        "--manifest-path",
        concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.toml"),
        "--bin",
        "solverforge",
        "--",
    ]);
    command
}

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("crate manifest dir should have a parent")
        .to_path_buf()
}

fn pin_generated_project_to_local_solverforge(project_dir: &std::path::Path) {
    let cargo_toml = project_dir.join("Cargo.toml");
    let manifest =
        std::fs::read_to_string(&cargo_toml).expect("failed to read scaffold Cargo.toml");
    let solverforge_path = workspace_root()
        .join("solverforge-rs")
        .join("crates")
        .join("solverforge");
    let standard_replacement = format!(
        "solverforge = {{ path = {:?}, features = [\"serde\", \"console\", \"verbose-logging\"] }}",
        solverforge_path
    );
    let basic_replacement = format!(
        "solverforge = {{ path = {:?}, features = [\"serde\"] }}",
        solverforge_path
    );
    let updated = if manifest.contains(
        "solverforge = { version = \"0.6.0\", features = [\"serde\", \"console\", \"verbose-logging\"] }",
    ) {
        manifest.replacen(
            "solverforge = { version = \"0.6.0\", features = [\"serde\", \"console\", \"verbose-logging\"] }",
            &standard_replacement,
            1,
        )
    } else {
        manifest.replacen(
            "solverforge = { version = \"0.6.0\", features = [\"serde\"] }",
            &basic_replacement,
            1,
        )
    };
    assert_ne!(
        manifest, updated,
        "failed to rewrite scaffold dependency to local solverforge path"
    );
    std::fs::write(&cargo_toml, updated).expect("failed to update scaffold Cargo.toml");
}

#[test]
fn test_new_standard_creates_project_files() {
    let tmp = tempfile::tempdir().expect("failed to create temp dir");
    let project_name = "test_standard_project";

    let status = cli_command()
        .args([
            "new",
            project_name,
            "--standard",
            "--skip-git",
            "--skip-readme",
            "--quiet",
        ])
        .current_dir(tmp.path())
        .status()
        .expect("failed to run solverforge new");

    assert!(status.success(), "solverforge new --standard failed");

    let project_dir = tmp.path().join(project_name);
    assert!(project_dir.exists(), "project directory not created");
    assert!(
        project_dir.join("Cargo.toml").exists(),
        "Cargo.toml missing"
    );
    assert!(project_dir.join("src").exists(), "src/ directory missing");
    assert!(
        project_dir.join(".gitignore").exists(),
        ".gitignore missing"
    );
    assert!(
        project_dir.join("solver.toml").exists(),
        "solver.toml missing"
    );
    assert!(
        project_dir.join("static").join("sf-config.json").exists(),
        "static/sf-config.json missing"
    );

    let sf_config =
        std::fs::read_to_string(project_dir.join("static").join("sf-config.json")).unwrap();
    assert!(
        sf_config.contains("\"type\": \"assignment_board\""),
        "standard scaffold should default to assignment_board view: {}",
        sf_config
    );
    assert!(
        sf_config.contains("\"balanced_load\""),
        "standard scaffold should wire balanced_load into sf-config.json: {}",
        sf_config
    );
    assert!(
        sf_config.contains("\"capacity_limit\"") && sf_config.contains("\"affinity_match\""),
        "standard scaffold should wire enriched demo constraints into sf-config.json: {}",
        sf_config
    );

    let app_js = std::fs::read_to_string(project_dir.join("static").join("app.js")).unwrap();
    let cargo_toml = std::fs::read_to_string(project_dir.join("Cargo.toml")).unwrap();
    let solver_service =
        std::fs::read_to_string(project_dir.join("src").join("solver").join("service.rs")).unwrap();
    let routes_rs =
        std::fs::read_to_string(project_dir.join("src").join("api").join("routes.rs")).unwrap();
    assert!(
        app_js.contains("SF.createHeader")
            && app_js.contains("SF.createStatusBar")
            && app_js.contains("SF.createSolver"),
        "standard scaffold should compose the app from solverforge-ui primitives: {}",
        app_js
    );
    assert!(
        cargo_toml.contains("solverforge-ui = \"0.3.0\""),
        "standard scaffold should pin solverforge-ui 0.3.0: {}",
        cargo_toml
    );
    assert!(
        solver_service.contains("mpsc::UnboundedReceiver<(Plan, HardSoftScore)>")
            && solver_service.contains("\"movesPerSecond\"")
            && solver_service.contains("\"id\""),
        "standard scaffold should align solver SSE payloads with the current backend contract: {}",
        solver_service
    );
    assert!(
        routes_rs.contains("Json<CreateScheduleResponse>")
            && routes_rs.contains("Json(CreateScheduleResponse { id })"),
        "standard scaffold should return create schedule responses as JSON ids: {}",
        routes_rs
    );
    assert!(
        app_js.contains("renderAssignmentBoard"),
        "standard scaffold should keep assignment-board-specific rendering: {}",
        app_js
    );
}

#[test]
fn test_new_list_creates_project_files() {
    let tmp = tempfile::tempdir().expect("failed to create temp dir");
    let project_name = "test_list_project";

    let status = cli_command()
        .args([
            "new",
            project_name,
            "--list",
            "--skip-git",
            "--skip-readme",
            "--quiet",
        ])
        .current_dir(tmp.path())
        .status()
        .expect("failed to run solverforge new");

    assert!(status.success(), "solverforge new --list failed");

    let project_dir = tmp.path().join(project_name);
    assert!(
        project_dir.join("Cargo.toml").exists(),
        "Cargo.toml missing"
    );
    assert!(
        project_dir.join("static").join("sf-config.json").exists(),
        "static/sf-config.json missing"
    );

    let app_js = std::fs::read_to_string(project_dir.join("static").join("app.js")).unwrap();
    let cargo_toml = std::fs::read_to_string(project_dir.join("Cargo.toml")).unwrap();
    let solver_service =
        std::fs::read_to_string(project_dir.join("src").join("solver").join("service.rs")).unwrap();
    let routes_rs =
        std::fs::read_to_string(project_dir.join("src").join("api").join("routes.rs")).unwrap();
    assert!(
        app_js.contains("SF.createHeader")
            && app_js.contains("SF.createStatusBar")
            && app_js.contains("SF.createSolver"),
        "list scaffold should compose the app from solverforge-ui primitives: {}",
        app_js
    );
    assert!(
        cargo_toml.contains("solverforge-ui = \"0.3.0\""),
        "list scaffold should pin solverforge-ui 0.3.0: {}",
        cargo_toml
    );
    assert!(
        solver_service.contains("mpsc::UnboundedReceiver<(Plan, HardSoftScore)>")
            && solver_service.contains("\"movesPerSecond\"")
            && solver_service.contains("\"id\""),
        "list scaffold should align solver SSE payloads with the current backend contract: {}",
        solver_service
    );
    assert!(
        routes_rs.contains("Json<CreateScheduleResponse>")
            && routes_rs.contains("Json(CreateScheduleResponse { id })"),
        "list scaffold should return create schedule responses as JSON ids: {}",
        routes_rs
    );
    assert!(
        app_js.contains("renderSequences"),
        "list scaffold should keep sequence-specific rendering: {}",
        app_js
    );
}

#[test]
fn test_new_removed_specializations_fail_with_guidance() {
    let tmp = tempfile::tempdir().expect("failed to create temp dir");

    let output = cli_command()
        .args([
            "new",
            "test_removed_template",
            "--standard=employee-scheduling",
        ])
        .current_dir(tmp.path())
        .output()
        .expect("failed to run solverforge new");

    assert!(
        !output.status.success(),
        "solverforge new --standard=employee-scheduling unexpectedly succeeded"
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("unexpected argument '--standard=employee-scheduling' found")
            || stderr.contains("unexpected value"),
        "unexpected stderr: {stderr}"
    );
}

#[test]
fn test_new_standard_cargo_check_passes() {
    let tmp = tempfile::tempdir().expect("failed to create temp dir");
    let project_name = "test_cargo_check_standard";

    let scaffold_status = cli_command()
        .args([
            "new",
            project_name,
            "--standard",
            "--skip-git",
            "--skip-readme",
            "--quiet",
        ])
        .current_dir(tmp.path())
        .status()
        .expect("failed to run solverforge new");

    assert!(scaffold_status.success(), "scaffolding failed");

    let project_dir = tmp.path().join(project_name);
    pin_generated_project_to_local_solverforge(&project_dir);
    let check_status = Command::new("cargo")
        .arg("check")
        .current_dir(&project_dir)
        .status()
        .expect("failed to run cargo check");

    assert!(
        check_status.success(),
        "cargo check failed on scaffolded standard project"
    );
}

#[test]
fn test_new_list_cargo_check_passes() {
    let tmp = tempfile::tempdir().expect("failed to create temp dir");
    let project_name = "test_cargo_check_list";

    let scaffold_status = cli_command()
        .args([
            "new",
            project_name,
            "--list",
            "--skip-git",
            "--skip-readme",
            "--quiet",
        ])
        .current_dir(tmp.path())
        .status()
        .expect("failed to run solverforge new");

    assert!(scaffold_status.success(), "scaffolding failed");

    let project_dir = tmp.path().join(project_name);
    pin_generated_project_to_local_solverforge(&project_dir);
    let check_status = Command::new("cargo")
        .arg("check")
        .current_dir(&project_dir)
        .status()
        .expect("failed to run cargo check");

    assert!(check_status.success(), "cargo check failed on list project");
}

#[test]
fn test_generate_constraint_workflow_cargo_check_passes() {
    let tmp = tempfile::tempdir().expect("failed to create temp dir");
    let project_name = "test_generated_constraint_workflow";

    let scaffold_status = cli_command()
        .args([
            "new",
            project_name,
            "--standard",
            "--skip-git",
            "--skip-readme",
            "--quiet",
        ])
        .current_dir(tmp.path())
        .status()
        .expect("failed to run solverforge new");

    assert!(scaffold_status.success(), "scaffolding failed");

    let project_dir = tmp.path().join(project_name);
    pin_generated_project_to_local_solverforge(&project_dir);
    let generate_status = cli_command()
        .args(["generate", "constraint", "coverage_gap", "--join", "--hard"])
        .current_dir(&project_dir)
        .status()
        .expect("failed to run solverforge generate constraint");

    assert!(generate_status.success(), "constraint generation failed");

    let check_status = Command::new("cargo")
        .arg("check")
        .current_dir(&project_dir)
        .status()
        .expect("failed to run cargo check");

    assert!(
        check_status.success(),
        "cargo check failed after generate constraint workflow"
    );
}
