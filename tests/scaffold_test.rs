// Integration tests for project scaffolding.
//
// Some tests invoke `cargo check` inside a temp directory and therefore require a full Rust
// toolchain plus dependency resolution access.

use std::path::Path;
use std::process::Command;

#[path = "support/dependency_overrides.rs"]
mod dependency_overrides;
#[path = "support/scaffold_generated_app.rs"]
mod scaffold_generated_app;

use dependency_overrides::{
    apply_generated_project_dependency_overrides, DependencyOverrideMode, USE_LOCAL_PATCHES_ENV,
};
use scaffold_generated_app::ScaffoldGeneratedApp;

const RUNTIME_DEP_LABEL: &str = "crates.io: solverforge 0.10.0";
const UI_DEP_LABEL: &str = "crates.io: solverforge-ui 0.6.5";
const MAPS_DEP_LABEL: &str = "crates.io: solverforge-maps 2.1.4";
const SOLVERFORGE_DEP_SPEC: &str =
    r#"{ version = "0.10.0", features = ["serde", "console", "verbose-logging"] }"#;
const SOLVERFORGE_UI_DEP_SPEC: &str = r#"{ version = "0.6.5" }"#;
const SOLVERFORGE_MAPS_DEP_SPEC: &str = r#"{ version = "2.1.4" }"#;
const GENERATED_RUST_VERSION_SPEC: &str = r#"rust-version = "1.95""#;
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

fn write_override_template(project_dir: &Path, name: &str, contents: &str) {
    let template_dir = project_dir.join(".solverforge").join("templates");
    std::fs::create_dir_all(&template_dir).expect("failed to create override template dir");
    std::fs::write(template_dir.join(format!("{name}.rs.tmpl")), contents)
        .expect("failed to write override template");
}

fn seed_list_template_domain(project_dir: &Path) {
    let list_template = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("templates")
        .join("list")
        .join("generic")
        .join("src")
        .join("domain");

    for file in ["mod.rs", "plan.rs", "container.rs", "item.rs"] {
        std::fs::write(
            project_dir.join("src").join("domain").join(file),
            std::fs::read_to_string(list_template.join(file))
                .expect("failed to read list template"),
        )
        .expect("failed to seed list template domain file");
    }
}

fn overlay_list_template(project_dir: &Path, project_name: &str) {
    for path in [
        "src/api",
        "src/constraints",
        "src/data",
        "src/domain",
        "src/solver",
        "static",
    ] {
        let target = project_dir.join(path);
        if target.exists() {
            std::fs::remove_dir_all(&target).expect("failed to remove stale scaffold directory");
        }
    }

    let template_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("templates")
        .join("list")
        .join("generic");
    copy_template_tree(&template_root, project_dir, project_name);
}

fn copy_template_tree(src: &Path, dst: &Path, project_name: &str) {
    std::fs::create_dir_all(dst).expect("failed to create template destination");
    let crate_name = project_name
        .chars()
        .map(|ch| {
            if ch == '-' {
                '_'
            } else {
                ch.to_ascii_lowercase()
            }
        })
        .collect::<String>();
    for entry in std::fs::read_dir(src).expect("failed to read template directory") {
        let entry = entry.expect("failed to read template entry");
        let path = entry.path();
        let file_type = entry
            .file_type()
            .expect("failed to read template entry type");
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if file_type.is_dir() {
            copy_template_tree(&path, &dst.join(name.as_ref()), project_name);
        } else if file_type.is_file() {
            let mut target_name = name.to_string();
            if let Some(stem) = target_name.strip_suffix(".tmpl") {
                target_name = stem.to_string();
            }
            let contents = std::fs::read_to_string(&path)
                .expect("failed to read text template")
                .replace("{{project_name}}", project_name)
                .replace("{{crate_name}}", &crate_name)
                .replace("{{solverforge_dep}}", SOLVERFORGE_DEP_SPEC)
                .replace("{{solverforge_ui_dep}}", SOLVERFORGE_UI_DEP_SPEC)
                .replace("{{solverforge_maps_dep}}", SOLVERFORGE_MAPS_DEP_SPEC);
            std::fs::write(dst.join(target_name), contents).expect("failed to write template file");
        }
    }
}

fn assert_template_retained_lifecycle_contract(template_kind: &str) {
    let template_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("templates")
        .join(template_kind)
        .join("generic");
    let solver_service =
        std::fs::read_to_string(template_root.join("src").join("solver").join("service.rs"))
            .unwrap_or_else(|err| panic!("failed to read {template_kind} solver service: {err}"));
    let sse_rs = std::fs::read_to_string(template_root.join("src").join("api").join("sse.rs"))
        .unwrap_or_else(|err| panic!("failed to read {template_kind} SSE API: {err}"));
    let app_js = std::fs::read_to_string(template_root.join("static").join("app.js"))
        .unwrap_or_else(|err| panic!("failed to read {template_kind} app.js: {err}"));

    for stale in ["last_event", "lastEvent", "sse_snapshot", "sseSnapshot"] {
        assert!(
            !solver_service.contains(stale) && !sse_rs.contains(stale) && !app_js.contains(stale),
            "{template_kind} template should not carry cached SSE fallback state `{stale}`"
        );
    }

    assert!(
        solver_service.contains("pub fn bootstrap_event(&self, id: &str)")
            && solver_service.contains("MANAGER.get_status(job_id)")
            && solver_service.contains("MANAGER.get_snapshot(job_id, Some(revision))")
            && solver_service.contains("snapshot_status_event_payload")
            && solver_service.contains("bootstrap_snapshot_event_type")
            && solver_service.contains("\"best_solution\""),
        "{template_kind} solver service should derive reconnect bootstrap from status plus retained snapshot: {solver_service}"
    );
    assert!(
        sse_rs.contains(".bootstrap_event(&id)")
            && sse_rs.contains("bootstrap_event_sequence")
            && sse_rs.contains("event_sequence <= bootstrap_event_sequence"),
        "{template_kind} SSE stream should emit bootstrap first and filter older live frames: {sse_rs}"
    );
    assert!(
        app_js.contains("SF.createSolver")
            && app_js.contains("cleanupTerminalJob()")
            && app_js.contains("solver.delete()")
            && app_js.contains("throw err;")
            && app_js.contains("solver.isRunning()")
            && app_js.contains("solver.getLifecycleState() === 'PAUSED'"),
        "{template_kind} app should stay thin around stock solver lifecycle controls: {app_js}"
    );
}

fn assert_template_timeline_contract(template_kind: &str) {
    let template_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("templates")
        .join(template_kind)
        .join("generic");
    let app_js = std::fs::read_to_string(template_root.join("static").join("app.js"))
        .unwrap_or_else(|err| panic!("failed to read {template_kind} app.js: {err}"));

    assert!(
        app_js.contains("SF.rail.createTimeline(timelineConfig)")
            && app_js.contains("SLOT_MINUTES = 60")
            && app_js.contains("startMinute: slotIndex * SLOT_MINUTES")
            && app_js.contains("endMinute: (slotIndex + 1) * SLOT_MINUTES")
            && app_js.contains("startMinute: 0")
            && app_js.contains("endMinute: normalizedSlots * SLOT_MINUTES")
            && app_js.contains("initialViewport"),
        "{template_kind} app should feed solverforge-ui timelines normalized integer minute models: {app_js}"
    );
    assert!(
        !app_js.contains("new Date(")
            && !app_js.contains("Date.parse")
            && !app_js.contains("toISOString")
            && !app_js.contains("parseFloat"),
        "{template_kind} app should not parse or coerce timestamps locally before calling SF.rail.createTimeline: {app_js}"
    );
}

fn assert_template_modal_body_contract(template_kind: &str) {
    let template_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("templates")
        .join(template_kind)
        .join("generic");
    let app_js = std::fs::read_to_string(template_root.join("static").join("app.js"))
        .unwrap_or_else(|err| panic!("failed to read {template_kind} app.js: {err}"));

    assert!(
        app_js.contains("analysisModal.setBody(buildAnalysisBody(analysis))")
            && app_js.contains("function buildAnalysisBody(analysis)")
            && app_js.contains("var container = SF.el('div'")
            && app_js.contains("SF.createTable({"),
        "{template_kind} app should build modal analysis content as DOM nodes before passing it to solverforge-ui: {app_js}"
    );
    assert!(
        !app_js.contains("buildAnalysisHtml")
            && !app_js.contains("analysisModal.setBody('<")
            && !app_js.contains("analysisModal.setBody(buildAnalysisHtml")
            && !app_js.contains("!solver.getJobId()")
            && !app_js.contains("unsafeBody"),
        "{template_kind} app should not pass generated HTML strings or unsafe modal bodies for analysis content: {app_js}"
    );
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
            && stdout.contains("Scaffold runtime target: SolverForge crate target 0.10.0")
            && stdout.contains("Scaffold UI target: solverforge-ui 0.6.5")
            && stdout.contains("Scaffold maps target: solverforge-maps 2.1.4")
            && stdout.contains(RUNTIME_DEP_LABEL)
            && stdout.contains(UI_DEP_LABEL)
            && stdout.contains(MAPS_DEP_LABEL),
        "version output should distinguish the CLI version from the scaffold dependency targets: {}",
        stdout
    );
}

#[test]
fn test_generate_scaffold_is_not_a_public_command() {
    let help = cli_command()
        .args(["generate", "--help"])
        .output()
        .expect("failed to run solverforge generate --help");

    assert!(help.status.success(), "generate --help failed");

    let stdout = String::from_utf8_lossy(&help.stdout);
    assert!(
        !stdout.contains("generate scaffold") && !stdout.contains("\n  scaffold"),
        "generate help should not advertise a scaffold subcommand: {stdout}"
    );

    let rejected = cli_command()
        .args(["generate", "scaffold", "legacy_app"])
        .output()
        .expect("failed to run solverforge generate scaffold");

    assert!(
        !rejected.status.success(),
        "removed generate scaffold command should fail"
    );
    let stderr = String::from_utf8_lossy(&rejected.stderr);
    assert!(
        stderr.contains("error:") && stderr.contains("scaffold"),
        "removed generate scaffold command should be rejected by Clap: {stderr}"
    );
}

#[test]
fn test_scalar_and_list_templates_use_status_snapshot_reconnect_bootstrap() {
    assert_template_retained_lifecycle_contract("scalar");
    assert_template_retained_lifecycle_contract("list");
}

#[test]
fn test_scalar_and_list_templates_emit_numeric_timeline_models() {
    assert_template_timeline_contract("scalar");
    assert_template_timeline_contract("list");
}

#[test]
fn test_scalar_and_list_templates_emit_safe_modal_bodies() {
    assert_template_modal_body_contract("scalar");
    assert_template_modal_body_contract("list");
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
    let gitignore = std::fs::read_to_string(project_dir.join(".gitignore")).unwrap();
    let plan_rs =
        std::fs::read_to_string(project_dir.join("src").join("domain").join("plan.rs")).unwrap();
    let main_rs = std::fs::read_to_string(project_dir.join("src").join("main.rs")).unwrap();
    let index_html =
        std::fs::read_to_string(project_dir.join("static").join("index.html")).unwrap();
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
    let data_mod =
        std::fs::read_to_string(project_dir.join("src").join("data").join("mod.rs")).unwrap();
    let data_seed =
        std::fs::read_to_string(project_dir.join("src").join("data").join("data_seed.rs")).unwrap();

    assert!(
        app_spec.contains("starter = \"neutral-shell\"")
            && app_spec.contains("[demo]")
            && app_spec.contains("default_size = \"standard\"")
            && app_spec.contains("available_sizes = [\"small\", \"standard\", \"large\"]")
            && !app_spec.contains("[[variables]]"),
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
            && app_js.contains("SF.rail.createTimeline")
            && app_js.contains("generated/ui-model.json")
            && app_js.contains("requestJson('/demo-data', 'demo data catalog')")
            && app_js.contains("buildScalarViewPayload")
            && app_js.contains("buildListViewPayload")
            && app_js.contains("defaultId")
            && app_js.contains("availableIds")
            && app_js.contains("No planning variables are declared yet")
            && app_js.contains("onPauseRequested")
            && app_js.contains("onSolution")
            && app_js.contains("onPaused")
            && app_js.contains("onResume")
            && app_js.contains("onCancel")
            && app_js.contains("onComplete")
            && app_js.contains("window.location.origin")
            && app_js.contains("throw err;")
            && !app_js.contains("/demo-data/STANDARD")
            && !app_js.contains("localhost:7860"),
        "neutral scaffold should render variable-driven views from solverforge-ui primitives: {}",
        app_js
    );
    assert!(
        cargo_toml.contains(
            "solverforge = { version = \"0.10.0\", features = [\"serde\", \"console\", \"verbose-logging\"] }"
        ) && cargo_toml.contains("solverforge-ui = { version = \"0.6.5\" }")
            && cargo_toml.contains("solverforge-maps = { version = \"2.1.4\" }")
            && cargo_toml.contains(GENERATED_RUST_VERSION_SPEC)
            && cargo_toml.contains("axum = \"0.8.9\"")
            && cargo_toml.contains("tokio = { version = \"1.52.1\", features = [\"full\"] }")
            && cargo_toml.contains(
                "tower-http = { version = \"0.6.8\", features = [\"fs\", \"cors\"] }"
            )
            && cargo_toml.contains("uuid = { version = \"1.23.1\", features = [\"v4\", \"serde\"] }"),
        "unified scaffold should point at the current SolverForge, solverforge-ui, and solverforge-maps crate targets: {}",
        cargo_toml
    );
    assert!(
        gitignore.lines().any(|line| line == "/target")
            && gitignore.lines().any(|line| line == "**/*.rs.bk")
            && !gitignore.lines().any(|line| line == "Cargo.lock"),
        "generated app .gitignore should keep Cargo.lock trackable: {}",
        gitignore
    );
    assert!(
        !plan_rs.contains("#[planning_entity_collection]")
            && !plan_rs.contains("#[problem_fact_collection]")
            && plan_rs.contains("@solverforge:neutral-solution")
            && plan_rs.contains("constraints = \"crate::constraints::create_constraints\"")
            && plan_rs.contains("solver_toml = \"../../solver.toml\"")
            && plan_rs.contains("pub score: Option<HardSoftScore>")
            && plan_rs.contains("@solverforge:begin solution-collections")
            && plan_rs.contains("@solverforge:begin solution-constructor-params")
            && plan_rs.contains("@solverforge:begin solution-constructor-init"),
        "neutral scaffold should start with an empty managed planning solution: {plan_rs}"
    );
    assert!(
        main_rs.contains("solverforge::console::init()")
            && !main_rs.contains("solverforge::__internal::init_console()")
            && main_rs.contains("std::env::var(\"PORT\")")
            && main_rs.contains("unwrap_or(7860)")
            && !main_rs.contains("Then open: http://localhost:7860")
            && main_rs.contains("default port 7860"),
        "neutral scaffold should honor PORT when binding the generated app: {}",
        main_rs
    );
    assert!(
        index_html.contains("/sf/img/ouroboros.svg")
            && !index_html.contains("solverforge-favicon.svg"),
        "neutral scaffold should only reference shipped solverforge-ui assets: {}",
        index_html
    );
    assert!(
        data_mod.contains("mod data_seed;")
            && data_mod.contains(
                "pub use data_seed::{available_demo_data, default_demo_data, generate, DemoData};"
            ),
        "neutral scaffold should keep data/mod.rs as the stable wrapper: {}",
        data_mod
    );
    assert!(
        data_seed.contains("@generated by solverforge-cli: data v1")
            && data_seed.contains("pub enum DemoData")
            && data_seed.contains("pub fn id(self) -> &'static str")
            && data_seed.contains("pub fn default_demo_data() -> DemoData")
            && data_seed.contains("pub fn available_demo_data() -> &'static [DemoData]")
            && data_seed.contains("pub fn generate(demo: DemoData) -> Plan"),
        "neutral scaffold should keep generated sample builders in data_seed.rs: {}",
        data_seed
    );
    assert!(
        solver_service.contains("mpsc::UnboundedReceiver<SolverEvent<Plan>>")
            && solver_service.contains("event_type: &'static str")
            && solver_service.contains("snapshot_revision: Option<u64>")
            && solver_service.contains("snapshot_status_event_payload")
            && solver_service.contains("MANAGER.get_snapshot(job_id, Some(revision))")
            && solver_service.contains("\"pause_requested\"")
            && solver_service.contains("\"paused\"")
            && solver_service.contains("\"resumed\"")
            && solver_service.contains("\"completed\"")
            && solver_service.contains("\"cancelled\"")
            && solver_service.contains("\"failed\"")
            && solver_service.contains("pub fn start_job(&self, plan: Plan)")
            && solver_service.contains("\"best_solution\"")
            && solver_service.contains("MANAGER.pause(parse_job_id(id)?)")
            && solver_service.contains("MANAGER.resume(parse_job_id(id)?)")
            && !solver_service.contains("last_event"),
        "neutral scaffold should align solver SSE payloads with the retained job lifecycle contract: {}",
        solver_service
    );
    assert!(
        routes_rs.contains("Json<CreateJobResponse>")
            && routes_rs.contains("Json(CreateJobResponse { id })")
            && routes_rs.contains("struct DemoDataCatalogResponse")
            && routes_rs.contains("DemoData::default_demo_data().id()")
            && routes_rs.contains("DemoData::available_demo_data()")
            && routes_rs.contains(".route(\"/jobs\", post(create_job))")
            && routes_rs.contains(".route(\"/jobs/{id}/snapshot\", get(get_snapshot))")
            && routes_rs.contains(".route(\"/jobs/{id}/pause\", post(pause_job))")
            && routes_rs.contains(".route(\"/jobs/{id}/resume\", post(resume_job))")
            && routes_rs.contains(".route(\"/jobs/{id}/cancel\", post(cancel_job))")
            && routes_rs.contains("StatusCode::BAD_REQUEST"),
        "neutral scaffold should expose the retained /jobs lifecycle routes: {}",
        routes_rs
    );
    assert!(
        sse_rs.contains(".bootstrap_event(&id)")
            && !sse_rs.contains("\"currentScore\":null")
            && !sse_rs.contains("\"bestScore\":null"),
        "neutral scaffold SSE bootstrap should derive the retained lifecycle payload from live solver status: {}",
        sse_rs
    );
}

#[test]
fn test_list_template_api_docs_follow_runtime_origin() {
    let list_app_js = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/templates/list/generic/static/app.js"
    ))
    .expect("failed to read list template app.js");
    let list_main_rs = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/templates/list/generic/src/main.rs.tmpl"
    ))
    .expect("failed to read list template main.rs");

    assert!(
        list_app_js.contains("window.location.origin")
            && !list_app_js.contains("localhost:7860")
            && list_app_js.contains("buildApiGuideEndpoints")
            && list_app_js.contains("requestJson('/demo-data', 'demo data catalog')")
            && list_app_js.contains("defaultId")
            && list_app_js.contains("availableIds")
            && list_app_js.contains("throw err;")
            && !list_app_js.contains("/demo-data/STANDARD"),
        "list template API guide should derive curl examples from the runtime origin: {}",
        list_app_js
    );
    assert!(
        !list_main_rs.contains("Then open: http://localhost:7860")
            && list_main_rs.contains("default port 7860"),
        "list template startup comment should not hard-code a fixed localhost URL: {}",
        list_main_rs
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
        )) && readme.contains("SolverForge runtime target for this scaffold: `solverforge 0.10.0`")
            && readme.contains("SolverForge UI target for this scaffold: `solverforge-ui 0.6.5`")
            && readme
                .contains("SolverForge maps target for this scaffold: `solverforge-maps 2.1.4`")
            && readme.contains(RUNTIME_DEP_LABEL)
            && readme.contains(UI_DEP_LABEL)
            && readme.contains(MAPS_DEP_LABEL)
            && readme.contains("configured crate dependency targets")
            && readme.contains("solverforge.app.toml")
            && readme.contains("solverforge generate fact resource")
            && readme.contains("solverforge generate entity task")
            && readme.contains("solverforge generate variable resource_idx --entity Task --kind scalar --range resources --allows-unassigned")
            && !readme.contains("solverforge generate entity worker --planning-variable shift_idx"),
        "scaffold README should distinguish CLI version from runtime target and show the current modeling flow: {}",
        readme
    );
}

#[test]
fn test_new_removed_template_flags_fail() {
    let tmp = tempfile::tempdir().expect("failed to create temp dir");

    let output = cli_command()
        .args(["new", "test_removed_template", "--scalar"])
        .current_dir(tmp.path())
        .output()
        .expect("failed to run solverforge new");

    assert!(
        !output.status.success(),
        "solverforge new --scalar unexpectedly succeeded"
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("unexpected argument '--scalar' found"),
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
    match apply_generated_project_dependency_overrides(&project_dir) {
        DependencyOverrideMode::CratesIo => {
            eprintln!(
                "using published SolverForge crate targets for scaffold validation; set {}=1 to apply explicit local Cargo patches",
                USE_LOCAL_PATCHES_ENV
            );
        }
        DependencyOverrideMode::LocalPatches => {
            eprintln!("using explicit local Cargo patches for scaffold validation");
        }
    }
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
fn test_list_template_direct_check_and_boot_passes() {
    let project_name = "test_list_template_direct";
    let mut app = ScaffoldGeneratedApp::new("list-template-direct", project_name);

    app.scaffold_neutral();
    overlay_list_template(app.project_dir(), project_name);
    apply_generated_project_dependency_overrides(app.project_dir());

    let solver_toml = std::fs::read_to_string(app.project_dir().join("solver.toml")).unwrap();
    let cargo_toml = std::fs::read_to_string(app.project_dir().join("Cargo.toml")).unwrap();
    let domain_mod =
        std::fs::read_to_string(app.project_dir().join("src").join("domain").join("mod.rs"))
            .unwrap();
    let data_seed = std::fs::read_to_string(
        app.project_dir()
            .join("src")
            .join("data")
            .join("data_seed.rs"),
    )
    .unwrap();
    let routes_rs =
        std::fs::read_to_string(app.project_dir().join("src").join("api").join("routes.rs"))
            .unwrap();
    let app_js = std::fs::read_to_string(app.project_dir().join("static").join("app.js")).unwrap();

    assert!(
        !cargo_toml.contains("{{")
            && cargo_toml.contains(&format!("solverforge = {SOLVERFORGE_DEP_SPEC}"))
            && cargo_toml.contains(&format!("solverforge-ui = {SOLVERFORGE_UI_DEP_SPEC}"))
            && cargo_toml.contains(&format!("solverforge-maps = {SOLVERFORGE_MAPS_DEP_SPEC}"))
            && cargo_toml.contains(GENERATED_RUST_VERSION_SPEC)
            && cargo_toml.contains("axum = \"0.8.9\"")
            && cargo_toml
                .contains("tokio-stream = { version = \"0.1.18\", features = [\"sync\"] }")
            && cargo_toml.contains("tower = \"0.5.3\"")
            && cargo_toml.contains("parking_lot = \"0.12.5\""),
        "list template Cargo.toml should render registry dependency specs: {cargo_toml}"
    );
    assert!(
        solver_toml.contains("seconds_spent_limit")
            && domain_mod.contains("solverforge::planning_model!")
            && domain_mod.contains("mod container;")
            && domain_mod.contains("mod item;")
            && domain_mod.contains("mod plan;")
            && data_seed.contains("pub enum DemoData")
            && data_seed.contains("available_demo_data()")
            && routes_rs.contains(".route(\"/jobs\", post(create_job))")
            && routes_rs.contains(".route(\"/jobs/{id}/events\", get(sse::events))")
            && routes_rs.contains(".route(\"/jobs/{id}/snapshot\", get(get_snapshot))")
            && routes_rs.contains(".route(\"/jobs/{id}/cancel\", post(cancel_job))")
            && app_js.contains("SF.createSolver")
            && app_js.contains("SF.rail.createTimeline"),
        "list template files should expose the retained lifecycle and sequence-board surface"
    );

    app.cargo_check("List template cargo check");
    app.cargo_build("List template cargo build");
    let port = app.start_server();
    let client = app.client();
    let base_url = app.base_url(port);
    let health: serde_json::Value = client
        .get(format!("{base_url}/health"))
        .send()
        .expect("health request failed")
        .error_for_status()
        .expect("health should succeed")
        .json()
        .expect("health should return JSON");
    assert_eq!(health["status"], "UP");

    let catalog: serde_json::Value = client
        .get(format!("{base_url}/demo-data"))
        .send()
        .expect("demo catalog request failed")
        .error_for_status()
        .expect("demo catalog should succeed")
        .json()
        .expect("demo catalog should return JSON");
    assert_eq!(catalog["defaultId"], "STANDARD");
    assert!(catalog["availableIds"]
        .as_array()
        .is_some_and(|ids| ids.iter().any(|id| id == "LARGE")));

    let served_app_js = client
        .get(format!("{base_url}/app.js"))
        .send()
        .expect("static app.js request failed")
        .error_for_status()
        .expect("static app.js should succeed")
        .text()
        .expect("static app.js should be text");
    assert!(served_app_js.contains("SF.createSolver"));

    app.mark_success();
}

#[test]
fn test_generate_solution_replaces_neutral_scaffold_and_cargo_check_passes() {
    let tmp = tempfile::tempdir().expect("failed to create temp dir");
    let project_name = "test_generate_solution_replaces_neutral";

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
    apply_generated_project_dependency_overrides(&project_dir);

    let solution_status = cli_command()
        .args([
            "generate",
            "solution",
            "schedule",
            "--score",
            "HardSoftDecimalScore",
        ])
        .current_dir(&project_dir)
        .status()
        .expect("failed to generate solution");

    assert!(
        solution_status.success(),
        "generate solution should replace a fresh neutral scaffold"
    );

    let domain_mod =
        std::fs::read_to_string(project_dir.join("src").join("domain").join("mod.rs")).unwrap();
    let constraints_mod =
        std::fs::read_to_string(project_dir.join("src").join("constraints").join("mod.rs"))
            .unwrap();
    let data_seed =
        std::fs::read_to_string(project_dir.join("src").join("data").join("data_seed.rs")).unwrap();
    let dto_rs =
        std::fs::read_to_string(project_dir.join("src").join("api").join("dto.rs")).unwrap();
    let solver_service =
        std::fs::read_to_string(project_dir.join("src").join("solver").join("service.rs")).unwrap();

    assert!(
        !project_dir
            .join("src")
            .join("domain")
            .join("plan.rs")
            .exists()
            && project_dir
                .join("src")
                .join("domain")
                .join("schedule.rs")
                .exists(),
        "generate solution should remove the neutral Plan and create Schedule"
    );
    assert!(
        domain_mod.contains("mod schedule;")
            && domain_mod.contains("pub use schedule::Schedule;")
            && !domain_mod.contains("Plan"),
        "domain exports should point at the generated solution: {}",
        domain_mod
    );
    assert!(
        constraints_mod.contains("use crate::domain::Schedule;")
            && constraints_mod.contains("ConstraintSet<Schedule, HardSoftDecimalScore>")
            && !constraints_mod.contains("ConstraintSet<Plan"),
        "constraint assembly should target the generated solution: {}",
        constraints_mod
    );
    assert!(
        data_seed.contains("use crate::domain::Schedule;")
            && data_seed.contains("pub fn default_demo_data() -> DemoData")
            && data_seed.contains("pub fn available_demo_data() -> &'static [DemoData]")
            && data_seed.contains("pub fn generate(demo: DemoData) -> Schedule")
            && data_seed.contains("Schedule::new()")
            && !data_seed.contains("Plan::new()"),
        "generated data should target the generated solution: {}",
        data_seed
    );
    assert!(
        dto_rs.contains("use crate::domain::Schedule;")
            && dto_rs.contains("from_plan(plan: &Schedule)")
            && dto_rs.contains("Result<Schedule, serde_json::Error>")
            && dto_rs.contains("SolverSnapshot<Schedule>")
            && dto_rs.contains("SolverStatus<HardSoftDecimalScore>")
            && dto_rs.contains("SolverSnapshotAnalysis<HardSoftDecimalScore>")
            && dto_rs.contains("ScoreAnalysis<HardSoftDecimalScore>")
            && !dto_rs.contains("HardSoftScore,")
            && !dto_rs.contains("<HardSoftScore>")
            && dto_rs.contains("PlanDto"),
        "DTOs should target the generated solution without renaming the DTO surface: {}",
        dto_rs
    );
    assert!(
        solver_service.contains("use crate::domain::Schedule;")
            && solver_service.contains("SolverManager<Schedule>")
            && solver_service.contains("pub fn start_job(&self, plan: Schedule)")
            && solver_service.contains("mpsc::UnboundedReceiver<SolverEvent<Schedule>>")
            && solver_service.contains("SolverStatus<HardSoftDecimalScore>")
            && solver_service.contains("SolverSnapshotAnalysis<HardSoftDecimalScore>")
            && solver_service.contains("SolverEventMetadata<HardSoftDecimalScore>")
            && !solver_service.contains("HardSoftScore,")
            && !solver_service.contains("<HardSoftScore>")
            && !solver_service.contains("crate::domain::Plan"),
        "solver service should target the generated solution: {}",
        solver_service
    );

    let check_status = Command::new("cargo")
        .arg("check")
        .current_dir(&project_dir)
        .status()
        .expect("failed to run cargo check");

    assert!(
        check_status.success(),
        "cargo check failed after replacing the neutral solution"
    );
}

#[test]
fn test_generate_solution_refuses_after_domain_shape_exists() {
    let tmp = tempfile::tempdir().expect("failed to create temp dir");
    let project_name = "test_generate_solution_refuses_shaped_project";

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
    let fact_status = cli_command()
        .args(["generate", "fact", "resource"])
        .current_dir(&project_dir)
        .status()
        .expect("failed to generate fact");

    assert!(fact_status.success(), "fact generation failed");

    let output = cli_command()
        .args([
            "generate",
            "solution",
            "schedule",
            "--score",
            "HardSoftScore",
        ])
        .current_dir(&project_dir)
        .output()
        .expect("failed to run generate solution");

    assert!(
        !output.status.success(),
        "generate solution should not replace a shaped project"
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("a planning solution 'Plan' already exists")
            && stderr.contains("solverforge destroy solution"),
        "unexpected generate solution error: {}",
        stderr
    );
    assert!(project_dir
        .join("src")
        .join("domain")
        .join("plan.rs")
        .exists());
    assert!(!project_dir
        .join("src")
        .join("domain")
        .join("schedule.rs")
        .exists());
}

#[test]
fn test_generate_solution_preflights_invalid_custom_override_before_neutral_removal() {
    let tmp = tempfile::tempdir().expect("failed to create temp dir");
    let project_name = "test_generate_solution_invalid_override";

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
    let domain_mod_before =
        std::fs::read_to_string(project_dir.join("src").join("domain").join("mod.rs")).unwrap();
    let plan_before =
        std::fs::read_to_string(project_dir.join("src").join("domain").join("plan.rs")).unwrap();
    write_override_template(
        &project_dir,
        "solution",
        "pub struct {{NAME}} {\n    pub score: Option<{{FIELDS}}>,\n}\n",
    );

    let output = cli_command()
        .args([
            "generate",
            "solution",
            "schedule",
            "--score",
            "HardSoftScore",
        ])
        .current_dir(&project_dir)
        .output()
        .expect("failed to run generate solution");

    assert!(
        !output.status.success(),
        "invalid custom solution override should fail"
    );
    assert!(
        String::from_utf8_lossy(&output.stderr).contains(
            ".solverforge/templates/solution.rs.tmpl is invalid: missing or duplicated managed block markers for 'solution-imports'"
        ),
        "unexpected stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        project_dir
            .join("src")
            .join("domain")
            .join("plan.rs")
            .exists(),
        "failed preflight must leave the neutral solution in place"
    );
    assert!(
        !project_dir
            .join("src")
            .join("domain")
            .join("schedule.rs")
            .exists(),
        "failed preflight must not create the replacement solution"
    );
    assert_eq!(
        std::fs::read_to_string(project_dir.join("src").join("domain").join("mod.rs")).unwrap(),
        domain_mod_before
    );
    assert_eq!(
        std::fs::read_to_string(project_dir.join("src").join("domain").join("plan.rs")).unwrap(),
        plan_before
    );
}

#[test]
fn test_generate_solution_preflights_existing_target_file_before_neutral_removal() {
    let tmp = tempfile::tempdir().expect("failed to create temp dir");
    let project_name = "test_generate_solution_existing_target";

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
    let domain_dir = project_dir.join("src").join("domain");
    let domain_mod_before =
        std::fs::read_to_string(domain_dir.join("mod.rs")).expect("failed to read domain mod");
    let plan_before =
        std::fs::read_to_string(domain_dir.join("plan.rs")).expect("failed to read neutral plan");
    let constraints_before =
        std::fs::read_to_string(project_dir.join("src").join("constraints").join("mod.rs"))
            .expect("failed to read constraints mod");
    let dto_before = std::fs::read_to_string(project_dir.join("src").join("api").join("dto.rs"))
        .expect("failed to read dto");
    let service_before =
        std::fs::read_to_string(project_dir.join("src").join("solver").join("service.rs"))
            .expect("failed to read solver service");
    let lib_before = std::fs::read_to_string(project_dir.join("src").join("lib.rs"))
        .expect("failed to read lib");
    let data_seed_before =
        std::fs::read_to_string(project_dir.join("src").join("data").join("data_seed.rs"))
            .expect("failed to read data seed");
    let existing_schedule = "// user-owned schedule module\n";
    std::fs::write(domain_dir.join("schedule.rs"), existing_schedule)
        .expect("failed to write existing target solution");

    let output = cli_command()
        .args([
            "generate",
            "solution",
            "schedule",
            "--score",
            "HardSoftScore",
        ])
        .current_dir(&project_dir)
        .output()
        .expect("failed to run generate solution");

    assert!(
        !output.status.success(),
        "pre-existing target solution file should fail"
    );
    assert!(
        String::from_utf8_lossy(&output.stderr)
            .contains("solution file 'src/domain/schedule.rs' already exists"),
        "unexpected stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        std::fs::read_to_string(domain_dir.join("mod.rs")).expect("failed to read domain mod"),
        domain_mod_before
    );
    assert_eq!(
        std::fs::read_to_string(domain_dir.join("plan.rs")).expect("failed to read neutral plan"),
        plan_before
    );
    assert_eq!(
        std::fs::read_to_string(domain_dir.join("schedule.rs"))
            .expect("failed to read existing target solution"),
        existing_schedule
    );
    assert_eq!(
        std::fs::read_to_string(project_dir.join("src").join("constraints").join("mod.rs"))
            .expect("failed to read constraints mod"),
        constraints_before
    );
    assert_eq!(
        std::fs::read_to_string(project_dir.join("src").join("api").join("dto.rs"))
            .expect("failed to read dto"),
        dto_before
    );
    assert_eq!(
        std::fs::read_to_string(project_dir.join("src").join("solver").join("service.rs"))
            .expect("failed to read solver service"),
        service_before
    );
    assert_eq!(
        std::fs::read_to_string(project_dir.join("src").join("lib.rs"))
            .expect("failed to read lib"),
        lib_before
    );
    assert_eq!(
        std::fs::read_to_string(project_dir.join("src").join("data").join("data_seed.rs"))
            .expect("failed to read data seed"),
        data_seed_before
    );
}

#[test]
fn test_generate_solution_existing_target_error_wins_over_invalid_custom_override() {
    let tmp = tempfile::tempdir().expect("failed to create temp dir");
    let project_name = "test_generate_solution_existing_target_beats_override";

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
    let domain_dir = project_dir.join("src").join("domain");
    let domain_mod_before =
        std::fs::read_to_string(domain_dir.join("mod.rs")).expect("failed to read domain mod");
    let plan_before =
        std::fs::read_to_string(domain_dir.join("plan.rs")).expect("failed to read neutral plan");
    let constraints_before =
        std::fs::read_to_string(project_dir.join("src").join("constraints").join("mod.rs"))
            .expect("failed to read constraints mod");
    let dto_before = std::fs::read_to_string(project_dir.join("src").join("api").join("dto.rs"))
        .expect("failed to read dto");
    let service_before =
        std::fs::read_to_string(project_dir.join("src").join("solver").join("service.rs"))
            .expect("failed to read solver service");
    let existing_schedule = "// user-owned schedule module\n";
    std::fs::write(domain_dir.join("schedule.rs"), existing_schedule)
        .expect("failed to write existing target solution");
    write_override_template(
        &project_dir,
        "solution",
        "pub struct {{NAME}} {\n    pub score: Option<{{FIELDS}}>,\n}\n",
    );

    let output = cli_command()
        .args([
            "generate",
            "solution",
            "schedule",
            "--score",
            "HardSoftScore",
        ])
        .current_dir(&project_dir)
        .output()
        .expect("failed to run generate solution");

    assert!(
        !output.status.success(),
        "pre-existing target solution file should fail"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("solution file 'src/domain/schedule.rs' already exists")
            && !stderr.contains("solution.rs.tmpl is invalid"),
        "existing target file should be reported before rendering invalid custom templates: {stderr}"
    );
    assert_eq!(
        std::fs::read_to_string(domain_dir.join("mod.rs")).expect("failed to read domain mod"),
        domain_mod_before
    );
    assert_eq!(
        std::fs::read_to_string(domain_dir.join("plan.rs")).expect("failed to read neutral plan"),
        plan_before
    );
    assert_eq!(
        std::fs::read_to_string(domain_dir.join("schedule.rs"))
            .expect("failed to read existing target solution"),
        existing_schedule
    );
    assert_eq!(
        std::fs::read_to_string(project_dir.join("src").join("constraints").join("mod.rs"))
            .expect("failed to read constraints mod"),
        constraints_before
    );
    assert_eq!(
        std::fs::read_to_string(project_dir.join("src").join("api").join("dto.rs"))
            .expect("failed to read dto"),
        dto_before
    );
    assert_eq!(
        std::fs::read_to_string(project_dir.join("src").join("solver").join("service.rs"))
            .expect("failed to read solver service"),
        service_before
    );
}

#[test]
fn test_generate_entity_preflights_invalid_custom_override_before_writes() {
    let tmp = tempfile::tempdir().expect("failed to create temp dir");
    let project_name = "test_generate_entity_invalid_override";

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
    let domain_mod_before =
        std::fs::read_to_string(project_dir.join("src").join("domain").join("mod.rs")).unwrap();
    let plan_before =
        std::fs::read_to_string(project_dir.join("src").join("domain").join("plan.rs")).unwrap();
    write_override_template(
        &project_dir,
        "entity",
        "pub struct {{NAME}} {\n    pub id: String,\n}\n",
    );

    let output = cli_command()
        .args(["generate", "entity", "task"])
        .current_dir(&project_dir)
        .output()
        .expect("failed to run generate entity");

    assert!(
        !output.status.success(),
        "invalid custom entity override should fail"
    );
    assert!(
        String::from_utf8_lossy(&output.stderr).contains(
            ".solverforge/templates/entity.rs.tmpl is invalid: missing or duplicated managed block markers for 'entity-variables'"
        ),
        "unexpected stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        !project_dir
            .join("src")
            .join("domain")
            .join("task.rs")
            .exists(),
        "failed preflight must not create an orphan entity file"
    );
    assert_eq!(
        std::fs::read_to_string(project_dir.join("src").join("domain").join("mod.rs")).unwrap(),
        domain_mod_before
    );
    assert_eq!(
        std::fs::read_to_string(project_dir.join("src").join("domain").join("plan.rs")).unwrap(),
        plan_before
    );
}

#[test]
fn test_generate_variable_accepts_valid_canonical_custom_entity_override() {
    let tmp = tempfile::tempdir().expect("failed to create temp dir");
    let project_name = "test_generate_variable_custom_entity_override";

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
    write_override_template(
        &project_dir,
        "entity",
        r#"use serde::{Deserialize, Serialize};
use solverforge::prelude::*;

#[planning_entity]
#[derive(Serialize, Deserialize)]
pub struct {{NAME}} {
    #[planning_id]
    pub id: String,
    pub label: String,
    // @solverforge:begin entity-variables
    // @solverforge:end entity-variables
}

impl {{NAME}} {
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            // @solverforge:begin entity-variable-init
            // @solverforge:end entity-variable-init
        }
    }
}
"#,
    );

    assert!(
        cli_command()
            .args(["generate", "fact", "resource"])
            .current_dir(&project_dir)
            .status()
            .expect("failed to generate fact")
            .success(),
        "fact generation failed"
    );
    assert!(
        cli_command()
            .args(["generate", "entity", "task"])
            .current_dir(&project_dir)
            .status()
            .expect("failed to generate entity")
            .success(),
        "entity generation failed"
    );
    assert!(
        cli_command()
            .args([
                "generate",
                "variable",
                "resource_idx",
                "--entity",
                "Task",
                "--kind",
                "scalar",
                "--range",
                "resources",
                "--allows-unassigned",
            ])
            .current_dir(&project_dir)
            .status()
            .expect("failed to generate variable")
            .success(),
        "variable generation failed"
    );

    let task_rs =
        std::fs::read_to_string(project_dir.join("src").join("domain").join("task.rs")).unwrap();
    assert!(
        task_rs.contains("pub label: String"),
        "custom field missing: {task_rs}"
    );
    assert!(
        task_rs.contains(
            "#[planning_variable(value_range_provider = \"resources\", allows_unassigned = true)]"
        ) && task_rs.contains("pub resource_idx: Option<usize>,")
            && task_rs.contains("resource_idx: None,"),
        "managed block rewrites should still work on canonical custom entity overrides: {task_rs}"
    );
}

#[test]
fn test_generate_fact_accepts_valid_canonical_custom_solution_override() {
    let tmp = tempfile::tempdir().expect("failed to create temp dir");
    let project_name = "test_generate_fact_custom_solution_override";

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
    write_override_template(
        &project_dir,
        "solution",
        r#"use serde::{Deserialize, Serialize};
use solverforge::prelude::*;

// @solverforge:begin solution-imports
// @solverforge:end solution-imports

#[planning_solution(
    constraints = "crate::constraints::create_constraints",
    solver_toml = "../../solver.toml"
)]
#[derive(Serialize, Deserialize)]
pub struct {{NAME}} {
    pub title: String,
    // @solverforge:begin solution-collections
    // @solverforge:end solution-collections
    #[planning_score]
    pub score: Option<{{FIELDS}}>,
}

impl {{NAME}} {
    pub fn new(
        // @solverforge:begin solution-constructor-params
        // @solverforge:end solution-constructor-params
    ) -> Self {
        Self {
            title: "custom".to_string(),
            // @solverforge:begin solution-constructor-init
            // @solverforge:end solution-constructor-init
            score: None,
        }
    }
}
"#,
    );

    assert!(
        cli_command()
            .args([
                "generate",
                "solution",
                "schedule",
                "--score",
                "HardSoftScore",
            ])
            .current_dir(&project_dir)
            .status()
            .expect("failed to generate solution")
            .success(),
        "solution generation failed"
    );
    assert!(
        cli_command()
            .args(["generate", "fact", "resource"])
            .current_dir(&project_dir)
            .status()
            .expect("failed to generate fact")
            .success(),
        "fact generation failed"
    );

    let schedule_rs =
        std::fs::read_to_string(project_dir.join("src").join("domain").join("schedule.rs"))
            .unwrap();
    assert!(
        schedule_rs.contains("title: \"custom\".to_string()")
            && schedule_rs.contains("use super::Resource;")
            && schedule_rs.contains("pub resources: Vec<Resource>,")
            && schedule_rs.contains("resources,"),
        "managed block rewrites should still work on canonical custom solution overrides: {schedule_rs}"
    );
}

#[test]
fn test_check_rejects_invalid_override_templates() {
    let tmp = tempfile::tempdir().expect("failed to create temp dir");
    let project_name = "test_check_invalid_override_templates";

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
    write_override_template(
        &project_dir,
        "entity",
        "pub struct {{NAME}} {\n    pub id: String,\n}\n",
    );
    write_override_template(
        &project_dir,
        "solution",
        "pub struct {{NAME}} {\n    pub score: Option<{{FIELDS}}>,\n}\n",
    );

    let output = cli_command()
        .arg("check")
        .current_dir(&project_dir)
        .output()
        .expect("failed to run solverforge check");

    assert!(
        !output.status.success(),
        "invalid override templates should fail solverforge check"
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains(
            ".solverforge/templates/entity.rs.tmpl: missing or duplicated managed block markers for 'entity-variables'"
        ) && stdout.contains(
            ".solverforge/templates/solution.rs.tmpl: missing or duplicated managed block markers for 'solution-imports'"
        ),
        "unexpected stdout: {stdout}"
    );
}

#[test]
fn test_check_rejects_malformed_current_managed_surfaces() {
    let tmp = tempfile::tempdir().expect("failed to create temp dir");
    let project_name = "test_check_invalid_current_managed_surfaces";

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
    assert!(
        cli_command()
            .args(["generate", "entity", "task"])
            .current_dir(&project_dir)
            .status()
            .expect("failed to generate entity")
            .success(),
        "entity generation failed"
    );

    std::fs::write(
        project_dir.join("src").join("domain").join("mod.rs"),
        r#"mod task;
mod plan;

pub use task::Task;
pub use plan::Plan;
"#,
    )
    .unwrap();
    std::fs::write(
        project_dir.join("src").join("domain").join("task.rs"),
        r#"use serde::{Deserialize, Serialize};
use solverforge::prelude::*;

#[planning_entity]
#[derive(Serialize, Deserialize)]
pub struct Task {
    #[planning_id]
    pub id: String,
}

impl Task {
    pub fn new(id: impl Into<String>) -> Self {
        Self { id: id.into() }
    }
}
"#,
    )
    .unwrap();
    let plan_path = project_dir.join("src").join("domain").join("plan.rs");
    let invalid_plan = std::fs::read_to_string(&plan_path)
        .unwrap()
        .replace("    // @solverforge:begin solution-collections\n", "")
        .replace("    // @solverforge:end solution-collections\n", "");
    std::fs::write(plan_path, invalid_plan).unwrap();
    std::fs::write(
        project_dir.join("src").join("constraints").join("mod.rs"),
        r#"/* Constraint definitions.

Add constraint modules with `solverforge generate constraint ...`.
The neutral shell starts with an empty constraint set. */

use crate::domain::Plan;
use solverforge::prelude::*;

pub use self::assemble::create_constraints;

mod assemble {
    use super::*;

    pub fn create_constraints() -> impl ConstraintSet<Plan, HardSoftScore> {
        ()
    }
}
"#,
    )
    .unwrap();

    let output = cli_command()
        .arg("check")
        .current_dir(&project_dir)
        .output()
        .expect("failed to run solverforge check");

    assert!(
        !output.status.success(),
        "malformed managed surfaces should fail solverforge check"
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("src/domain/mod.rs: src/domain/mod.rs must declare solverforge::planning_model! { ... }")
            && stdout.contains("src/constraints/mod.rs: missing or duplicated managed block markers for 'constraint-modules'"),
        "unexpected stdout: {stdout}"
    );
    assert!(
        !stdout.contains("src/domain/plan.rs:")
            && !stdout.contains("src/domain/task.rs:"),
        "domain member files should not be checked after the canonical manifest is invalid: {stdout}"
    );
}

#[test]
fn test_destroy_solution_removes_list_template_plan_re_exports() {
    let tmp = tempfile::tempdir().expect("failed to create temp dir");
    let project_name = "test_destroy_solution_list_template_exports";

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
    seed_list_template_domain(&project_dir);

    let output = cli_command()
        .args(["destroy", "--yes", "solution"])
        .current_dir(&project_dir)
        .output()
        .expect("failed to run solverforge destroy solution");

    assert!(output.status.success(), "destroy solution failed");
    let domain_mod =
        std::fs::read_to_string(project_dir.join("src").join("domain").join("mod.rs")).unwrap();
    assert!(
        !project_dir
            .join("src")
            .join("domain")
            .join("plan.rs")
            .exists(),
        "destroy solution should remove plan.rs"
    );
    assert!(
        !domain_mod.contains("pub use plan::Plan;")
            && domain_mod.contains("pub use container::Container;")
            && domain_mod.contains("pub use item::Item;"),
        "destroy solution should remove all plan-owned re-exports from the managed block: {domain_mod}"
    );
}

#[test]
fn test_destroy_entity_container_on_list_template_keeps_project_buildable() {
    let tmp = tempfile::tempdir().expect("failed to create temp dir");
    let project_name = "test_destroy_list_template_entity";

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
    apply_generated_project_dependency_overrides(&project_dir);
    seed_list_template_domain(&project_dir);

    let output = cli_command()
        .args(["destroy", "--yes", "entity", "container"])
        .current_dir(&project_dir)
        .output()
        .expect("failed to run solverforge destroy entity");

    assert!(
        output.status.success(),
        "destroy entity should succeed on the list template: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let plan_rs =
        std::fs::read_to_string(project_dir.join("src").join("domain").join("plan.rs")).unwrap();
    assert!(
        !plan_rs.contains("use super::Container;")
            && !plan_rs.contains("pub containers: Vec<Container>,"),
        "destroy entity should remove container-owned solution wiring: {plan_rs}"
    );

    let data_status = cli_command()
        .args(["generate", "data"])
        .current_dir(&project_dir)
        .status()
        .expect("failed to run solverforge generate data");
    assert!(
        data_status.success(),
        "generate data should resync compiler-owned sample data after destroy"
    );

    let check_status = Command::new("cargo")
        .arg("check")
        .current_dir(&project_dir)
        .status()
        .expect("failed to run cargo check");

    assert!(
        check_status.success(),
        "cargo check failed after destroying the list-template entity"
    );
}

#[test]
fn test_destroy_fact_item_on_list_template_rejects_element_collection_dependency() {
    let tmp = tempfile::tempdir().expect("failed to create temp dir");
    let project_name = "test_destroy_list_template_fact_dependency";

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
    seed_list_template_domain(&project_dir);
    let plan_before =
        std::fs::read_to_string(project_dir.join("src").join("domain").join("plan.rs")).unwrap();
    let item_before =
        std::fs::read_to_string(project_dir.join("src").join("domain").join("item.rs")).unwrap();

    let output = cli_command()
        .args(["destroy", "--yes", "fact", "item"])
        .current_dir(&project_dir)
        .output()
        .expect("failed to run solverforge destroy fact");

    assert!(
        !output.status.success(),
        "destroy fact should reject still-referenced list element collections"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains(
            "cannot destroy fact 'Item' because managed planning variables still reference collection 'item_facts': Container.items (element_collection = \"item_facts\")"
        ),
        "unexpected stderr: {stderr}"
    );
    assert!(
        project_dir
            .join("src")
            .join("domain")
            .join("item.rs")
            .exists(),
        "failed dependency check must leave the fact file in place"
    );
    assert_eq!(
        std::fs::read_to_string(project_dir.join("src").join("domain").join("plan.rs")).unwrap(),
        plan_before
    );
    assert_eq!(
        std::fs::read_to_string(project_dir.join("src").join("domain").join("item.rs")).unwrap(),
        item_before
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
            "scalar",
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

    assert!(spec.contains("field = \"resource_idx\"") && spec.contains("kind = \"scalar\""));
    assert!(
        ui_model.contains("\"kind\": \"scalar\"")
            && ui_model.contains("\"entityPlural\": \"tasks\"")
            && ui_model.contains("\"sourcePlural\": \"resources\"")
    );
}

#[test]
fn test_generate_variable_rejects_noncanonical_kind() {
    let tmp = tempfile::tempdir().expect("failed to create temp dir");
    let project_name = "test_generated_variable_rejects_unknown_kind";

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
    assert!(
        cli_command()
            .args(["generate", "fact", "resource"])
            .current_dir(&project_dir)
            .status()
            .expect("failed to generate fact")
            .success(),
        "fact generation failed"
    );
    assert!(
        cli_command()
            .args(["generate", "entity", "task"])
            .current_dir(&project_dir)
            .status()
            .expect("failed to generate entity")
            .success(),
        "entity generation failed"
    );
    let output = cli_command()
        .args([
            "generate",
            "variable",
            "resource_idx",
            "--entity",
            "Task",
            "--kind",
            "not_a_variable_kind",
            "--range",
            "resources",
            "--allows-unassigned",
        ])
        .current_dir(&project_dir)
        .output()
        .expect("failed to generate variable");

    assert!(
        !output.status.success(),
        "noncanonical kind should be rejected"
    );
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("valid values: scalar, list"),
        "stderr should explain the canonical kind set: {}",
        String::from_utf8_lossy(&output.stderr)
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
    apply_generated_project_dependency_overrides(&project_dir);

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
            "scalar",
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
fn test_generate_data_creates_direct_compiler_owned_data_module() {
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
    apply_generated_project_dependency_overrides(&project_dir);

    let fact_status = cli_command()
        .args([
            "generate",
            "fact",
            "resource",
            "--field",
            "category:String",
            "--field",
            "load:i32",
        ])
        .current_dir(&project_dir)
        .status()
        .expect("failed to generate fact");
    assert!(fact_status.success(), "fact generation failed");

    let entity_status = cli_command()
        .args([
            "generate",
            "entity",
            "task",
            "--field",
            "label:String",
            "--field",
            "priority:i32",
        ])
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
            "scalar",
            "--range",
            "resources",
            "--allows-unassigned",
        ])
        .current_dir(&project_dir)
        .status()
        .expect("failed to generate variable");
    assert!(variable_status.success(), "variable generation failed");

    let data_status = cli_command()
        .args(["generate", "data", "--size", "large"])
        .current_dir(&project_dir)
        .status()
        .expect("failed to generate data");
    assert!(data_status.success(), "data generation failed");

    let data_mod =
        std::fs::read_to_string(project_dir.join("src").join("data").join("mod.rs")).unwrap();
    let data_seed =
        std::fs::read_to_string(project_dir.join("src").join("data").join("data_seed.rs")).unwrap();
    let spec = std::fs::read_to_string(project_dir.join("solverforge.app.toml")).unwrap();

    assert!(
        data_mod.contains("mod data_seed;")
            && data_mod.contains(
                "pub use data_seed::{available_demo_data, default_demo_data, generate, DemoData};"
            ),
        "data module should be the stable wrapper: {}",
        data_mod
    );
    assert!(
        data_seed.contains("@generated by solverforge-cli: data v1")
            && data_seed.contains("pub enum DemoData")
            && data_seed.contains("const DEFAULT_DEMO_DATA: DemoData = DemoData::Large;")
            && data_seed.contains("pub fn id(self) -> &'static str")
            && data_seed.contains("pub fn default_demo_data() -> DemoData")
            && data_seed.contains("pub fn available_demo_data() -> &'static [DemoData]")
            && data_seed.contains("DemoData::Small => generate_plan(")
            && data_seed.contains("DemoData::Standard => generate_plan(")
            && data_seed.contains("DemoData::Large => generate_plan(")
            && data_seed.contains("let resources =")
            && data_seed.contains("let tasks =")
            && data_seed.contains("category: format!(\"resource-category-{idx}\")")
            && data_seed.contains("load: ((idx % 7) as i32) - 2")
            && data_seed.contains("label: format!(\"task-label-{idx}\")")
            && data_seed.contains("priority: ((idx % 7) as i32) - 2")
            && data_seed.contains("Plan::new(resources, tasks)")
            && !data_seed.contains("crate::generated"),
        "data seed should be rebuilt from the current project model: {}",
        data_seed
    );
    assert!(
        !project_dir.join("src").join("generated").exists(),
        "generated module should not be part of the data pipeline"
    );
    assert!(
        spec.contains("default_size = \"large\"")
            && spec.contains("[demo]")
            && spec.contains("\"small\"")
            && spec.contains("\"standard\"")
            && spec.contains("\"large\""),
        "generate data --size should persist the project default in solverforge.app.toml: {}",
        spec
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
fn test_generate_flows_preserve_custom_data_wrapper() {
    let tmp = tempfile::tempdir().expect("failed to create temp dir");
    let project_name = "test_generate_flows_preserve_data_module";

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
    apply_generated_project_dependency_overrides(&project_dir);

    let custom_data_wrapper = r#"mod data_seed;

pub use data_seed::{generate, DemoData};

pub fn custom_source() -> &'static str {
    "csv"
}
"#;
    std::fs::write(
        project_dir.join("src").join("data").join("mod.rs"),
        custom_data_wrapper,
    )
    .unwrap();

    let lib_path = project_dir.join("src").join("lib.rs");
    let fact_status = cli_command()
        .args(["generate", "fact", "resource"])
        .current_dir(&project_dir)
        .status()
        .expect("failed to generate fact");
    assert!(fact_status.success(), "fact generation failed");

    let data_status = cli_command()
        .args(["generate", "data"])
        .current_dir(&project_dir)
        .status()
        .expect("failed to generate data");
    assert!(data_status.success(), "data generation failed");

    let data_mod =
        std::fs::read_to_string(project_dir.join("src").join("data").join("mod.rs")).unwrap();
    let data_seed =
        std::fs::read_to_string(project_dir.join("src").join("data").join("data_seed.rs")).unwrap();
    let lib_rs = std::fs::read_to_string(&lib_path).unwrap();

    assert_eq!(data_mod, custom_data_wrapper);
    assert!(
        data_seed.contains("@generated by solverforge-cli: data v1")
            && data_seed.contains("pub enum DemoData")
            && data_seed.contains("pub fn default_demo_data() -> DemoData")
            && data_seed.contains("pub fn available_demo_data() -> &'static [DemoData]")
            && data_seed.contains("pub fn generate(demo: DemoData) -> Plan")
            && !data_seed.contains("crate::generated"),
        "generate flows should rebuild the canonical generated data seed: {}",
        data_seed
    );
    assert!(
        !lib_rs.contains("pub mod generated;"),
        "generate flows should not restore the removed generated module export: {}",
        lib_rs
    );

    let check_status = Command::new("cargo")
        .arg("check")
        .current_dir(&project_dir)
        .status()
        .expect("failed to run cargo check");

    assert!(
        check_status.success(),
        "cargo check failed after preserving the custom data wrapper and regenerating the seed"
    );
}
