// Integration tests for project scaffolding.
//
// Some tests invoke `cargo check` inside a temp directory and therefore require a full Rust
// toolchain plus dependency resolution access.

use std::path::PathBuf;
use std::process::Command;

const LOCAL_SOLVERFORGE_PATH: &str = "/srv/lab/dev/solverforge/solverforge-rs/crates/solverforge";
const LOCAL_SOLVERFORGE_UI_PATH: &str = "/srv/lab/dev/solverforge/solverforge-ui";
const CLI_VERSION: &str = env!("CARGO_PKG_VERSION");

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
    let minimal_replacement = format!(
        "solverforge = {{ path = {:?}, features = [\"serde\"] }}",
        solverforge_path
    );
    let updated = if manifest.contains(&standard_replacement)
        || manifest.contains(&minimal_replacement)
    {
        manifest.clone()
    } else {
        manifest
            .replacen(
                "solverforge = { path = \"/srv/lab/dev/solverforge/solverforge-rs/crates/solverforge\", features = [\"serde\", \"console\", \"verbose-logging\"] }",
                &standard_replacement,
                1,
            )
            .replacen(
                "solverforge = { path = \"/srv/lab/dev/solverforge/solverforge-rs/crates/solverforge\", features = [\"serde\"] }",
                &minimal_replacement,
                1,
            )
    };
    if manifest != updated {
        std::fs::write(&cargo_toml, updated).expect("failed to update scaffold Cargo.toml");
    }
}

fn legacy_data_loader_stub() -> &'static str {
    "/* Data loading module.\n\n   Replace `load()` with code that reads your real inputs and constructs the\n   domain objects your API or solver layer needs. */\n\npub fn load() -> Result<(), Box<dyn std::error::Error>> {\n    Ok(())\n}\n"
}

#[test]
fn test_version_output_distinguishes_cli_from_runtime_target() {
    let output = cli_command()
        .arg("--version")
        .output()
        .expect("failed to run solverforge --version");

    assert!(output.status.success(), "--version failed");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains(&format!("solverforge-cli {}", CLI_VERSION))
            && stdout.contains(&format!("CLI version: {}", CLI_VERSION))
            && stdout.contains(
                "Scaffold runtime target: SolverForge local checkout (pre-0.7.0 release)"
            )
            && stdout.contains(LOCAL_SOLVERFORGE_PATH)
            && stdout.contains(LOCAL_SOLVERFORGE_UI_PATH),
        "version output should distinguish the CLI version from the scaffold runtime target: {}",
        stdout
    );
}

#[test]
fn test_new_creates_neutral_project_files() {
    let tmp = tempfile::tempdir().expect("failed to create temp dir");
    let project_name = "test_unified_project";

    let status = cli_command()
        .args([
            "new",
            project_name,
            "--skip-git",
            "--skip-readme",
            "--quiet",
        ])
        .current_dir(tmp.path())
        .status()
        .expect("failed to run solverforge new");

    assert!(status.success(), "solverforge new failed");

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
        project_dir.join("solverforge.app.toml").exists(),
        "solverforge.app.toml missing"
    );

    let app_spec =
        std::fs::read_to_string(project_dir.join("solverforge.app.toml")).expect("read app spec");
    let sf_config =
        std::fs::read_to_string(project_dir.join("static").join("sf-config.json")).unwrap();
    let app_js = std::fs::read_to_string(project_dir.join("static").join("app.js")).unwrap();
    let cargo_toml = std::fs::read_to_string(project_dir.join("Cargo.toml")).unwrap();
    let plan_rs =
        std::fs::read_to_string(project_dir.join("src").join("domain").join("plan.rs")).unwrap();
    let main_rs = std::fs::read_to_string(project_dir.join("src").join("main.rs")).unwrap();
    let ui_model = std::fs::read_to_string(
        project_dir
            .join("static")
            .join("generated")
            .join("ui-model.json"),
    )
    .unwrap();
    let solver_service =
        std::fs::read_to_string(project_dir.join("src").join("solver").join("service.rs")).unwrap();
    let routes_rs =
        std::fs::read_to_string(project_dir.join("src").join("api").join("routes.rs")).unwrap();
    let sse_rs =
        std::fs::read_to_string(project_dir.join("src").join("api").join("sse.rs")).unwrap();

    assert!(
        app_spec.contains("starter = \"neutral-shell\"") && !app_spec.contains("[[variables]]"),
        "neutral scaffold should ship an empty app spec: {}",
        app_spec
    );
    assert!(
        sf_config.contains("\"title\"") && sf_config.contains("\"subtitle\""),
        "neutral scaffold should keep only the shell config in sf-config.json: {}",
        sf_config
    );
    assert!(
        ui_model.contains("\"views\": []")
            && ui_model.contains("\"constraints\": []")
            && ui_model.contains("\"entities\": []")
            && ui_model.contains("\"facts\": []"),
        "neutral scaffold should start with an empty UI model: {}",
        ui_model
    );
    assert!(
        app_js.contains("SF.createHeader")
            && app_js.contains("SF.createStatusBar")
            && app_js.contains("SF.createSolver")
            && app_js.contains("generated/ui-model.json")
            && app_js.contains("renderStandardView")
            && app_js.contains("renderListView")
            && app_js.contains("No planning variables are declared yet")
            && app_js.contains("onProgress")
            && app_js.contains("onSolution")
            && app_js.contains("onComplete"),
        "neutral scaffold should render variable-driven views from solverforge-ui primitives: {}",
        app_js
    );
    assert!(
        cargo_toml.contains(
            "solverforge = { path = \"/srv/lab/dev/solverforge/solverforge-rs/crates/solverforge\", features = [\"serde\", \"console\", \"verbose-logging\"] }"
        ) && cargo_toml.contains("solverforge-ui = { path = \"/srv/lab/dev/solverforge/solverforge-ui\" }"),
        "unified scaffold should point at the local SolverForge and solverforge-ui checkouts until the releases are published: {}",
        cargo_toml
    );
    assert!(
        !plan_rs.contains("#[planning_entity_collection]")
            && !plan_rs.contains("#[problem_fact_collection]")
            && plan_rs.contains("pub score: Option<HardSoftScore>")
            && plan_rs.contains("pub fn new() -> Self"),
        "neutral scaffold should start with a score-only plan shell: {plan_rs}"
    );
    assert!(
        main_rs.contains("std::env::var(\"PORT\")") && main_rs.contains("unwrap_or(7860)"),
        "neutral scaffold should honor PORT when binding the generated app: {}",
        main_rs
    );
    assert!(
        solver_service.contains("mpsc::UnboundedReceiver<SolverEvent<Plan>>")
            && solver_service.contains("event_type: &'static str")
            && solver_service.contains("current_score: Option<String>")
            && solver_service.contains("best_score: Option<String>")
            && solver_service.contains("solution: Option<PlanDto>")
            && solver_service.contains("fn snapshot_event(state: &JobState)")
            && solver_service.contains("\"best_solution\"")
            && solver_service.contains("\"finished\"")
            && solver_service.contains("let (event_type, solution) = snapshot_event(state);"),
        "neutral scaffold should align solver SSE payloads with the current backend contract: {}",
        solver_service
    );
    assert!(
        routes_rs.contains("Json<CreateScheduleResponse>")
            && routes_rs.contains("Json(CreateScheduleResponse { id })"),
        "neutral scaffold should return create schedule responses as JSON ids: {}",
        routes_rs
    );
    assert!(
        sse_rs.contains("\"eventType\":\"progress\"")
            && sse_rs.contains("\"currentScore\":null")
            && sse_rs.contains("\"bestScore\":null"),
        "neutral scaffold SSE bootstrap should use the typed progress payload shape: {}",
        sse_rs
    );
}

#[test]
fn test_new_readme_records_cli_and_runtime_versions_separately() {
    let tmp = tempfile::tempdir().expect("failed to create temp dir");
    let project_name = "test_readme_versions";

    let status = cli_command()
        .args(["new", project_name, "--skip-git", "--quiet"])
        .current_dir(tmp.path())
        .status()
        .expect("failed to run solverforge new");

    assert!(status.success(), "solverforge new failed");

    let readme = std::fs::read_to_string(tmp.path().join(project_name).join("README.md"))
        .expect("failed to read scaffold README.md");
    assert!(
        readme.contains(&format!(
            "CLI version used to scaffold this project: `{}`",
            CLI_VERSION
        )) && readme.contains(
            "SolverForge runtime target for this scaffold: `local checkout (pre-0.7.0 release)`"
        ) && readme.contains(LOCAL_SOLVERFORGE_PATH)
            && readme.contains(LOCAL_SOLVERFORGE_UI_PATH)
            && readme.contains("hardcoded local path dependencies")
            && readme.contains("solverforge.app.toml"),
        "scaffold README should distinguish CLI version from runtime target: {}",
        readme
    );
}

#[test]
fn test_new_removed_template_flags_fail() {
    let tmp = tempfile::tempdir().expect("failed to create temp dir");

    let output = cli_command()
        .args(["new", "test_removed_template", "--standard"])
        .current_dir(tmp.path())
        .output()
        .expect("failed to run solverforge new");

    assert!(
        !output.status.success(),
        "solverforge new --standard unexpectedly succeeded"
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("unexpected argument '--standard' found"),
        "unexpected stderr: {stderr}"
    );
}

#[test]
fn test_new_neutral_cargo_check_passes() {
    let tmp = tempfile::tempdir().expect("failed to create temp dir");
    let project_name = "test_cargo_check_unified";

    let scaffold_status = cli_command()
        .args([
            "new",
            project_name,
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
        "cargo check failed on scaffolded neutral project"
    );
}

#[test]
fn test_generate_variable_updates_app_spec_and_ui_model() {
    let tmp = tempfile::tempdir().expect("failed to create temp dir");
    let project_name = "test_generated_variable_ui_model";

    let scaffold_status = cli_command()
        .args([
            "new",
            project_name,
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

    let fact_status = cli_command()
        .args(["generate", "fact", "resource"])
        .current_dir(&project_dir)
        .status()
        .expect("failed to generate fact");
    assert!(fact_status.success(), "fact generation failed");

    let entity_status = cli_command()
        .args(["generate", "entity", "task"])
        .current_dir(&project_dir)
        .status()
        .expect("failed to generate entity");
    assert!(entity_status.success(), "entity generation failed");

    let variable_status = cli_command()
        .args([
            "generate",
            "variable",
            "resource_idx",
            "--entity",
            "Task",
            "--kind",
            "standard",
            "--range",
            "resources",
            "--allows-unassigned",
        ])
        .current_dir(&project_dir)
        .status()
        .expect("failed to generate variable");
    assert!(variable_status.success(), "variable generation failed");

    let spec = std::fs::read_to_string(project_dir.join("solverforge.app.toml")).unwrap();
    let ui_model = std::fs::read_to_string(
        project_dir
            .join("static")
            .join("generated")
            .join("ui-model.json"),
    )
    .unwrap();

    assert!(spec.contains("field = \"resource_idx\"") && spec.contains("kind = \"standard\""));
    assert!(
        ui_model.contains("\"kind\": \"standard\"")
            && ui_model.contains("\"entityPlural\": \"tasks\"")
            && ui_model.contains("\"sourcePlural\": \"resources\"")
    );
}

#[test]
fn test_generate_constraint_workflow_cargo_check_passes() {
    let tmp = tempfile::tempdir().expect("failed to create temp dir");
    let project_name = "test_generated_constraint_workflow";

    let scaffold_status = cli_command()
        .args([
            "new",
            project_name,
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

    let fact_status = cli_command()
        .args(["generate", "fact", "resource"])
        .current_dir(&project_dir)
        .status()
        .expect("failed to run solverforge generate fact");
    assert!(fact_status.success(), "fact generation failed");

    let entity_status = cli_command()
        .args(["generate", "entity", "task"])
        .current_dir(&project_dir)
        .status()
        .expect("failed to run solverforge generate entity");
    assert!(entity_status.success(), "entity generation failed");

    let variable_status = cli_command()
        .args([
            "generate",
            "variable",
            "resource_idx",
            "--entity",
            "Task",
            "--kind",
            "standard",
            "--range",
            "resources",
            "--allows-unassigned",
        ])
        .current_dir(&project_dir)
        .status()
        .expect("failed to run solverforge generate variable");
    assert!(variable_status.success(), "variable generation failed");

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

#[test]
fn test_generate_data_creates_compiler_owned_seed_and_preserves_wrapper() {
    let tmp = tempfile::tempdir().expect("failed to create temp dir");
    let project_name = "test_generate_data";

    let scaffold_status = cli_command()
        .args([
            "new",
            project_name,
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

    let fact_status = cli_command()
        .args(["generate", "fact", "resource", "--field", "capacity:usize"])
        .current_dir(&project_dir)
        .status()
        .expect("failed to generate fact");
    assert!(fact_status.success(), "fact generation failed");

    let entity_status = cli_command()
        .args(["generate", "entity", "task", "--field", "priority:i32"])
        .current_dir(&project_dir)
        .status()
        .expect("failed to generate entity");
    assert!(entity_status.success(), "entity generation failed");

    let variable_status = cli_command()
        .args([
            "generate",
            "variable",
            "resource_idx",
            "--entity",
            "Task",
            "--kind",
            "standard",
            "--range",
            "resources",
            "--allows-unassigned",
        ])
        .current_dir(&project_dir)
        .status()
        .expect("failed to generate variable");
    assert!(variable_status.success(), "variable generation failed");

    let data_status = cli_command()
        .args(["generate", "data"])
        .current_dir(&project_dir)
        .status()
        .expect("failed to generate data");
    assert!(data_status.success(), "data generation failed");

    let data_wrapper =
        std::fs::read_to_string(project_dir.join("src").join("data").join("mod.rs")).unwrap();
    let generated_mod =
        std::fs::read_to_string(project_dir.join("src").join("generated").join("mod.rs")).unwrap();
    let generated_seed = std::fs::read_to_string(
        project_dir
            .join("src")
            .join("generated")
            .join("data_seed.rs"),
    )
    .unwrap();

    assert!(
        data_wrapper.contains("pub use crate::generated::data_seed::DemoData;")
            && data_wrapper.contains("crate::generated::data_seed::generate(demo)"),
        "data wrapper should remain a stable shim over the compiler-owned generated seed: {}",
        data_wrapper
    );
    assert_eq!(
        generated_mod, "pub mod data_seed;\n",
        "generated module should expose the compiler-owned data seed"
    );
    assert!(
        generated_seed.contains("pub enum DemoData")
            && generated_seed.contains("DemoData::Small => generate_plan(")
            && generated_seed.contains("DemoData::Standard => generate_plan(")
            && generated_seed.contains("let resources =")
            && generated_seed.contains("let tasks =")
            && generated_seed.contains("Plan::new(resources, tasks)"),
        "generated data seed should be rebuilt from the current project model: {}",
        generated_seed
    );

    let check_status = Command::new("cargo")
        .arg("check")
        .current_dir(&project_dir)
        .status()
        .expect("failed to run cargo check");

    assert!(
        check_status.success(),
        "cargo check failed after generate data workflow"
    );
}

#[test]
fn test_generate_data_migrates_legacy_owned_loader_and_backfills_lib_export() {
    let tmp = tempfile::tempdir().expect("failed to create temp dir");
    let project_name = "test_generate_data_legacy_migration";

    let scaffold_status = cli_command()
        .args([
            "new",
            project_name,
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

    std::fs::write(
        project_dir.join("src").join("data").join("mod.rs"),
        legacy_data_loader_stub(),
    )
    .unwrap();

    let lib_path = project_dir.join("src").join("lib.rs");
    let legacy_lib = std::fs::read_to_string(&lib_path)
        .unwrap()
        .replace("pub mod generated;\n", "");
    std::fs::write(&lib_path, legacy_lib).unwrap();

    let fact_status = cli_command()
        .args(["generate", "fact", "resource"])
        .current_dir(&project_dir)
        .status()
        .expect("failed to generate fact");
    assert!(fact_status.success(), "fact generation failed");

    let data_wrapper =
        std::fs::read_to_string(project_dir.join("src").join("data").join("mod.rs")).unwrap();
    let lib_rs = std::fs::read_to_string(&lib_path).unwrap();

    assert!(
        data_wrapper.contains("@generated by solverforge-cli: data-wrapper v1")
            && data_wrapper.contains("crate::generated::data_seed::generate(demo)"),
        "legacy owned data loader should be migrated to the generated wrapper: {}",
        data_wrapper
    );
    assert!(
        lib_rs.contains("pub mod generated;"),
        "legacy app migration should backfill pub mod generated; into src/lib.rs: {}",
        lib_rs
    );

    let check_status = Command::new("cargo")
        .arg("check")
        .current_dir(&project_dir)
        .status()
        .expect("failed to run cargo check");

    assert!(
        check_status.success(),
        "cargo check failed after migrating legacy data loader"
    );
}

#[test]
fn test_generate_data_preserves_customized_legacy_loader() {
    let tmp = tempfile::tempdir().expect("failed to create temp dir");
    let project_name = "test_generate_data_preserve_custom_legacy_loader";

    let scaffold_status = cli_command()
        .args([
            "new",
            project_name,
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

    let custom_loader = "/* Data loading module.\n\n   Replace `load()` with code that reads your real inputs and constructs the\n   domain objects your API or solver layer needs. */\n\nuse std::str::FromStr;\n\nuse crate::domain::{Plan, Resource};\n\n#[derive(Debug, Clone, Copy)]\npub enum DemoData {\n    Standard,\n}\n\nimpl FromStr for DemoData {\n    type Err = ();\n\n    fn from_str(s: &str) -> Result<Self, Self::Err> {\n        match s.to_uppercase().as_str() {\n            \"STANDARD\" => Ok(DemoData::Standard),\n            _ => Err(()),\n        }\n    }\n}\n\npub fn generate(_demo: DemoData) -> Plan {\n    let resources = vec![Resource::new(0, \"custom-resource\")];\n    Plan::new(resources)\n}\n\npub fn custom_source() -> &'static str { \"csv\" }\n"
        .to_string();
    std::fs::write(
        project_dir.join("src").join("data").join("mod.rs"),
        &custom_loader,
    )
    .unwrap();

    let lib_path = project_dir.join("src").join("lib.rs");
    let legacy_lib = std::fs::read_to_string(&lib_path)
        .unwrap()
        .replace("pub mod generated;\n", "");
    std::fs::write(&lib_path, legacy_lib).unwrap();

    let fact_status = cli_command()
        .args(["generate", "fact", "resource"])
        .current_dir(&project_dir)
        .status()
        .expect("failed to generate fact");
    assert!(fact_status.success(), "fact generation failed");

    let data_mod =
        std::fs::read_to_string(project_dir.join("src").join("data").join("mod.rs")).unwrap();
    let lib_rs = std::fs::read_to_string(&lib_path).unwrap();

    assert_eq!(
        data_mod, custom_loader,
        "customized legacy data loader should not be overwritten"
    );
    assert!(
        !lib_rs.contains("pub mod generated;"),
        "customized legacy app should not be rewritten to depend on generated data wiring"
    );

    let check_status = Command::new("cargo")
        .arg("check")
        .current_dir(&project_dir)
        .status()
        .expect("failed to run cargo check");

    assert!(
        check_status.success(),
        "cargo check failed after preserving customized legacy data loader"
    );
}
