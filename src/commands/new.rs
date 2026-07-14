use include_dir::{include_dir, Dir};
use std::fmt;
use std::fs;
use std::path::Path;
use std::process::Command;

use crate::error::{is_rust_keyword, CliError, CliResult};
use crate::output;
use crate::scaffold_target::{
    MAPS_CRATE_VERSION, MAPS_SOURCE_PATH, MAPS_TARGET_LABEL, RUNTIME_CRATE_VERSION,
    RUNTIME_SOURCE_PATH, RUNTIME_TARGET_DISPLAY, RUNTIME_TARGET_LABEL, UI_CRATE_VERSION,
    UI_SOURCE_PATH, UI_TARGET_LABEL,
};
use crate::template;

// Keep the neutral scaffold embedded so generated apps are self-contained at build time.
static UNIFIED_TEMPLATE: Dir = include_dir!("$CARGO_MANIFEST_DIR/templates/scalar/generic");

#[derive(Clone, Copy, Debug, Eq, PartialEq, clap::ValueEnum)]
pub enum ScaffoldShell {
    Web,
    Api,
    Cli,
}

impl ScaffoldShell {
    fn as_str(self) -> &'static str {
        match self {
            Self::Web => "web",
            Self::Api => "api",
            Self::Cli => "cli",
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Web => "neutral web scaffold",
            Self::Api => "neutral API scaffold",
            Self::Cli => "neutral CLI scaffold",
        }
    }
}

impl fmt::Display for ScaffoldShell {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

pub fn run(
    name: &str,
    shell: ScaffoldShell,
    skip_git: bool,
    skip_readme: bool,
    quiet: bool,
) -> CliResult {
    let crate_name = to_crate_name(name);

    // Validate project name
    validate_project_name(name, &crate_name)?;

    scaffold(
        name,
        &crate_name,
        &UNIFIED_TEMPLATE,
        shell,
        skip_git,
        skip_readme,
        quiet,
    )
}

fn validate_project_name(name: &str, crate_name: &str) -> CliResult {
    if name.is_empty() {
        return Err(CliError::InvalidProjectName {
            name: name.to_string(),
            reason: "name cannot be empty",
        });
    }

    // Must start with a letter
    if !name.chars().next().is_some_and(|c| c.is_ascii_alphabetic()) {
        return Err(CliError::InvalidProjectName {
            name: name.to_string(),
            reason: "must start with a letter",
        });
    }

    // Only valid chars
    if !name
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return Err(CliError::InvalidProjectName {
            name: name.to_string(),
            reason: "may only contain letters, digits, hyphens, and underscores",
        });
    }

    // Check Rust keyword
    if is_rust_keyword(crate_name) {
        return Err(CliError::ReservedKeyword {
            name: crate_name.to_string(),
        });
    }

    Ok(())
}

fn scaffold(
    project_name: &str,
    crate_name: &str,
    template_dir: &Dir,
    shell: ScaffoldShell,
    skip_git: bool,
    skip_readme: bool,
    quiet: bool,
) -> CliResult {
    let start = std::time::Instant::now();
    let dest = Path::new(project_name);
    if dest.exists() {
        return Err(CliError::DirectoryExists {
            name: project_name.to_string(),
        });
    }

    output::print_heading(&format!(
        "Creating {} project '{}'",
        shell.label(),
        project_name
    ));

    let vars: &[(&str, &str)] = &[
        ("solverforge_dep", &solverforge_dep_spec()),
        ("solverforge_ui_dep", &solverforge_ui_dep_spec()),
        ("solverforge_maps_dep", &solverforge_maps_dep_spec()),
        ("scaffold_shell", shell.as_str()),
        ("project_name", project_name),
        ("crate_name", crate_name),
        ("solverforge_cli_version", env!("CARGO_PKG_VERSION")),
        ("solverforge_runtime_target", RUNTIME_TARGET_LABEL),
        ("solverforge_runtime_source", RUNTIME_SOURCE_PATH),
        ("solverforge_ui_source", UI_SOURCE_PATH),
    ];

    template::render(template_dir, dest, vars)?;
    materialize_shell(dest, project_name, crate_name, shell)?;

    // Write .gitignore
    let gitignore_content = "/target\n**/*.rs.bk\n";
    fs::write(dest.join(".gitignore"), gitignore_content).map_err(|e| CliError::IoError {
        context: "failed to write .gitignore".to_string(),
        source: e,
    })?;
    output::print_create(".gitignore");

    if !skip_readme {
        // Write README.md
        let readme = generate_readme(project_name, crate_name, shell);
        fs::write(dest.join("README.md"), readme).map_err(|e| CliError::IoError {
            context: "failed to write README.md".to_string(),
            source: e,
        })?;
        output::print_create("README.md");
    }

    // Print file listing
    print_file_tree(dest, dest)?;

    if !skip_git {
        // git init
        let git_ok = Command::new("git")
            .args(["init", "--quiet"])
            .current_dir(dest)
            .status()
            .map(|s| s.success())
            .unwrap_or(false);

        if git_ok {
            // Initial commit
            let add_ok = Command::new("git")
                .args(["add", "."])
                .current_dir(dest)
                .status()
                .map(|s| s.success())
                .unwrap_or(false);

            if add_ok {
                let _ = Command::new("git")
                    .args([
                        "commit",
                        "--quiet",
                        "-m",
                        "Initial commit from solverforge new",
                    ])
                    .current_dir(dest)
                    .status();
            }

            output::print_invoke("git init");
        }
    }

    println!();
    output::print_success(&format!(
        "  Project created in {} ({})",
        project_name,
        output::format_elapsed(start)
    ));
    println!();

    print_template_guidance(project_name, shell);

    // Optional cargo check prompt (skipped in quiet mode)
    if !quiet {
        run_cargo_check_prompt(dest)?;
    }

    Ok(())
}

fn materialize_shell(
    dest: &Path,
    project_name: &str,
    crate_name: &str,
    shell: ScaffoldShell,
) -> CliResult {
    match shell {
        ScaffoldShell::Web => Ok(()),
        ScaffoldShell::Api => {
            remove_dir_if_exists(&dest.join("static"))?;
            write_generated(
                dest.join("solverforge.app.toml"),
                &app_spec_toml(project_name, shell),
            )?;
            write_generated(
                dest.join("Cargo.toml"),
                &api_cargo_toml(project_name, crate_name),
            )?;
            write_generated(
                dest.join("src/main.rs"),
                &api_main_rs(project_name, crate_name),
            )?;
            Ok(())
        }
        ScaffoldShell::Cli => {
            remove_dir_if_exists(&dest.join("static"))?;
            remove_file_if_exists(&dest.join("src/api/routes.rs"))?;
            remove_file_if_exists(&dest.join("src/api/sse.rs"))?;
            write_generated(
                dest.join("solverforge.app.toml"),
                &app_spec_toml(project_name, shell),
            )?;
            write_generated(
                dest.join("Cargo.toml"),
                &cli_cargo_toml(project_name, crate_name),
            )?;
            write_generated(dest.join("src/api/mod.rs"), cli_api_mod_rs())?;
            write_generated(dest.join("src/lib.rs"), &cli_lib_rs(project_name))?;
            write_generated(
                dest.join("src/main.rs"),
                &cli_main_rs(project_name, crate_name),
            )?;
            Ok(())
        }
    }
}

fn remove_dir_if_exists(path: &Path) -> CliResult {
    if path.exists() {
        fs::remove_dir_all(path).map_err(|e| CliError::IoError {
            context: format!("failed to remove {}", path.display()),
            source: e,
        })?;
    }
    Ok(())
}

fn remove_file_if_exists(path: &Path) -> CliResult {
    if path.exists() {
        fs::remove_file(path).map_err(|e| CliError::IoError {
            context: format!("failed to remove {}", path.display()),
            source: e,
        })?;
    }
    Ok(())
}

fn write_generated(path: impl AsRef<Path>, content: &str) -> CliResult {
    let path = path.as_ref();
    fs::write(path, content).map_err(|e| CliError::IoError {
        context: format!("failed to write {}", path.display()),
        source: e,
    })
}

fn solverforge_dep_spec() -> String {
    format!(
        "{{ version = \"{RUNTIME_CRATE_VERSION}\", features = [\"serde\", \"console\", \"verbose-logging\"] }}"
    )
}

fn solverforge_ui_dep_spec() -> String {
    format!("{{ version = \"{UI_CRATE_VERSION}\" }}")
}

fn solverforge_maps_dep_spec() -> String {
    format!("{{ version = \"{MAPS_CRATE_VERSION}\" }}")
}

fn app_spec_toml(project_name: &str, shell: ScaffoldShell) -> String {
    let ui_source = if shell == ScaffoldShell::Web {
        format!("ui_source = \"{UI_SOURCE_PATH}\"\n")
    } else {
        String::new()
    };
    format!(
        r#"[app]
name = "{project_name}"
starter = "neutral-shell"
shell = "{shell}"
cli_version = "{cli_version}"

[runtime]
target = "{runtime_target}"
runtime_source = "{runtime_source}"
{ui_source}
[demo]
default_size = "standard"
available_sizes = ["small", "standard", "large"]

[solution]
name = "Plan"
score = "HardSoftScore"
"#,
        cli_version = env!("CARGO_PKG_VERSION"),
        runtime_target = RUNTIME_TARGET_LABEL,
        runtime_source = RUNTIME_SOURCE_PATH,
    )
}

fn base_package_toml(project_name: &str, crate_name: &str) -> String {
    format!(
        r#"[package]
name = "{project_name}"
version = "0.1.0"
edition = "2021"
rust-version = "1.95"
description = "Constraint optimizer built with SolverForge"

[[bin]]
name = "{crate_name}"
path = "src/main.rs"

[dependencies]
solverforge = {solverforge_dep}
"#,
        solverforge_dep = solverforge_dep_spec(),
    )
}

fn api_cargo_toml(project_name: &str, crate_name: &str) -> String {
    format!(
        r#"{base}
# HTTP API server
axum = "0.8.9"
tokio = {{ version = "1.52.3", features = ["full"] }}
tokio-stream = {{ version = "0.1.18", features = ["sync"] }}
tower-http = {{ version = "0.6.11", features = ["cors"] }}

# Serialization
serde = {{ version = "1.0.228", features = ["derive"] }}
serde_json = "1.0.150"

# Utilities
parking_lot = "0.12.5"
"#,
        base = base_package_toml(project_name, crate_name),
    )
}

fn cli_cargo_toml(project_name: &str, crate_name: &str) -> String {
    format!(
        r#"{base}
# Command-line shell
clap = {{ version = "4.6.1", features = ["derive"] }}
tokio = {{ version = "1.52.3", features = ["full"] }}

# Serialization
serde = {{ version = "1.0.228", features = ["derive"] }}
serde_json = "1.0.150"

# Utilities
parking_lot = "0.12.5"
"#,
        base = base_package_toml(project_name, crate_name),
    )
}

fn api_main_rs(project_name: &str, crate_name: &str) -> String {
    format!(
        r#"/* {project_name} — SolverForge HTTP API
   Run with: solverforge server */

use {crate_name}::api;

use std::net::SocketAddr;
use std::sync::Arc;
use tower_http::cors::{{Any, CorsLayer}};

#[tokio::main]
async fn main() {{
    solverforge::console::init();

    let state = Arc::new(api::AppState::new());

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let app = api::router(state).layer(cors);

    let port = std::env::var("PORT")
        .ok()
        .and_then(|value| value.parse::<u16>().ok())
        .unwrap_or(7860);
    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    println!("▸ {project_name} API listening on http://{{}}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}}
"#
    )
}

fn cli_api_mod_rs() -> &'static str {
    r#"mod dto;
mod telemetry;

pub use dto::PlanDto;
pub use telemetry::{CandidateTraceDto, TelemetryDto};
"#
}

fn cli_lib_rs(project_name: &str) -> String {
    format!(
        r#"/* {project_name} — neutral command-line optimizer built with SolverForge

Structure:
  domain/      — Plan (solution) plus CLI-generated entities and facts
  constraints/ — Scoring rules
  solver/      — Engine, service, termination config
  data/        — Demo data / data loading */

pub mod api;
pub mod constraints;
pub mod data;
pub mod domain;
pub mod solver;
"#
    )
}

fn cli_main_rs(project_name: &str, crate_name: &str) -> String {
    format!(
        r#"/* {project_name} — SolverForge command-line shell */

use clap::{{Parser, Subcommand}};
use {crate_name}::api::PlanDto;
use {crate_name}::data::{{default_demo_data, generate, DemoData}};

#[derive(Parser)]
#[command(name = "{project_name}")]
#[command(about = "SolverForge command-line optimizer")]
struct Cli {{
    #[command(subcommand)]
    command: Option<Command>,
}}

#[derive(Subcommand)]
enum Command {{
    /// Print generated demo data as JSON
    DemoData {{
        /// Demo data size: small, standard, or large
        #[arg(long)]
        size: Option<String>,
    }},
}}

fn main() -> Result<(), Box<dyn std::error::Error>> {{
    solverforge::console::init();
    let cli = Cli::parse();

    match cli.command.unwrap_or(Command::DemoData {{ size: None }}) {{
        Command::DemoData {{ size }} => {{
            let demo = match size {{
                Some(size) => size
                    .parse::<DemoData>()
                    .map_err(|_| std::io::Error::new(
                        std::io::ErrorKind::InvalidInput,
                        format!("unknown demo data size '{{size}}'"),
                    ))?,
                None => default_demo_data(),
            }};
            let dto = PlanDto::from_plan(&generate(demo));
            println!("{{}}", serde_json::to_string_pretty(&dto)?);
        }}
    }}

    Ok(())
}}
"#
    )
}

fn run_cargo_check_prompt(dest: &Path) -> CliResult {
    use dialoguer::Confirm;

    let run_check = Confirm::new()
        .with_prompt("Run `cargo check` to verify the project compiles?")
        .default(true)
        .interact()
        .unwrap_or(false);

    if run_check {
        output::print_invoke("cargo check");
        let status = Command::new("cargo")
            .arg("check")
            .current_dir(dest)
            .status()
            .map_err(|e| CliError::IoError {
                context: "failed to run cargo check".to_string(),
                source: e,
            })?;

        if status.success() {
            output::print_success("  cargo check passed");
        } else {
            output::print_error("cargo check failed — the project may need fixes");
        }
    }

    Ok(())
}

fn print_template_guidance(project_name: &str, shell: ScaffoldShell) {
    if output::is_quiet() {
        return;
    }

    println!("  Next steps:");
    println!("    cd {}", project_name);
    println!(
        "    # CLI {} targeting SolverForge {}",
        env!("CARGO_PKG_VERSION"),
        RUNTIME_TARGET_LABEL
    );

    match shell {
        ScaffoldShell::Web | ScaffoldShell::Api => println!("    solverforge server"),
        ScaffoldShell::Cli => println!("    cargo run -- demo-data"),
    }
    println!();
    println!("  This scaffold includes:");
    println!(
        "    - One neutral {} shell for scalar, list, or mixed modeling",
        shell
    );
    match shell {
        ScaffoldShell::Web => {
            println!(
                "    - Variable-driven timeline and data views generated from solverforge.app.toml"
            );
            println!("    - Retained job lifecycle with pause, resume, cancel, and delete");
            println!("    - Typed SSE lifecycle events and snapshot-bound score analysis");
            println!("    - Full compact telemetry plus opt-in candidate-trace diagnostics");
        }
        ScaffoldShell::Api => {
            println!("    - HTTP JSON and SSE endpoints without frontend assets");
            println!("    - Retained job lifecycle with pause, resume, cancel, and delete");
            println!("    - Full compact telemetry plus opt-in candidate-trace diagnostics");
        }
        ScaffoldShell::Cli => {
            println!("    - A Clap command-line entry point without Axum or frontend assets");
            println!("    - Demo-data JSON output backed by the same domain contract");
        }
    }
    println!("    - solverforge.app.toml for the scaffolded domain contract");
    println!("    - solver.toml as the search-strategy layer");
    println!("    solverforge generate entity task");
    println!("    solverforge generate fact resource");
    println!("    solverforge generate variable resource_idx --entity Task --kind scalar --range resources --allows-unassigned");
    println!("    solverforge generate variable visit_order --entity Route --kind list --elements visits");
    println!("    # Or use --countable-range 0..24 for an integer-valued scalar domain");

    println!();
}

fn print_file_tree(root: &Path, dir: &Path) -> CliResult {
    let mut entries: Vec<_> = std::fs::read_dir(dir)
        .map_err(|e| CliError::IoError {
            context: format!("failed to read directory {:?}", dir),
            source: e,
        })?
        .filter_map(|e| e.ok())
        .collect();

    entries.sort_by_key(|e| e.file_name());

    for entry in entries {
        let path = entry.path();
        let relative = path.strip_prefix(root).unwrap_or(&path);

        if path.is_dir() {
            // Skip .git directory
            if path.file_name().is_some_and(|n| n == ".git") {
                continue;
            }
            print_file_tree(root, &path)?;
        } else {
            output::print_create(&relative.display().to_string());
        }
    }

    Ok(())
}

fn generate_readme(project_name: &str, _crate_name: &str, shell: ScaffoldShell) -> String {
    let mut readme = format!("# {}\n\n", project_name);
    readme.push_str(&format!(
        "A SolverForge constraint optimization project (scaffold: `{}`).\n\n",
        shell.label()
    ));
    readme.push_str("## Versioning\n\n");
    readme.push_str(&format!(
        "- CLI version used to scaffold this project: `{}`\n",
        env!("CARGO_PKG_VERSION")
    ));
    readme.push_str(&format!(
        "- SolverForge runtime target for this scaffold: `{}`\n",
        RUNTIME_TARGET_LABEL
    ));
    if shell == ScaffoldShell::Web {
        readme.push_str(&format!(
            "- SolverForge UI target for this scaffold: `{}`\n",
            UI_TARGET_LABEL
        ));
        readme.push_str(&format!(
            "- SolverForge maps target for this scaffold: `{}`\n",
            MAPS_TARGET_LABEL
        ));
    }
    readme.push_str(&format!(
        "- Runtime dependency currently wired into `Cargo.toml`: `{}`\n",
        RUNTIME_SOURCE_PATH
    ));
    if shell == ScaffoldShell::Web {
        readme.push_str(&format!(
            "- Frontend UI dependency currently wired into `Cargo.toml`: `{}`\n",
            UI_SOURCE_PATH
        ));
        readme.push_str(&format!(
            "- Maps dependency currently wired into `Cargo.toml`: `{}`\n",
            MAPS_SOURCE_PATH
        ));
    }
    readme.push_str(&format!("- Scaffold shell: `{}`\n\n", shell.as_str()));
    readme.push_str(&format!(
        "This project was scaffolded by `solverforge-cli`, and it currently targets `{}` through the configured crate dependency targets.\n\n",
        RUNTIME_TARGET_DISPLAY
    ));
    readme.push_str("## Quick Start\n\n");
    readme.push_str("```bash\n");
    match shell {
        ScaffoldShell::Web | ScaffoldShell::Api => {
            readme.push_str("# Start the solver server\n");
            readme.push_str("solverforge server\n\n");
            readme.push_str("# Or run directly\n");
            readme.push_str("cargo run --release\n");
        }
        ScaffoldShell::Cli => {
            readme.push_str("# Print generated demo data\n");
            readme.push_str("cargo run -- demo-data\n");
        }
    }
    readme.push_str("```\n\n");
    readme.push_str("## Development\n\n");
    readme.push_str("```bash\n");
    readme.push_str("# Add a new constraint\n");
    readme.push_str("solverforge generate constraint my_rule --unary --hard\n\n");
    readme.push_str("# Add a problem fact\n");
    readme.push_str(
        "solverforge generate fact resource --field category:String --field load:i32\n\n",
    );
    readme.push_str("# Add a domain entity\n");
    readme
        .push_str("solverforge generate entity task --field label:String --field priority:i32\n\n");
    readme.push_str("# Add a scalar planning variable\n");
    readme.push_str("solverforge generate variable resource_idx --entity Task --kind scalar --range resources --allows-unassigned\n\n");
    readme.push_str("# Or add a scalar over a non-negative half-open integer range\n");
    readme.push_str("# solverforge generate variable hour --entity Task --kind scalar --countable-range 0..24\n\n");
    readme.push_str("# Or add scalar hook metadata when your domain owns the hook functions\n");
    readme.push_str("# solverforge generate variable resource_idx --entity Task --kind scalar --range resources --candidate-values resource_candidates\n\n");
    readme.push_str("# Add an ordered list variable with optional current SolverForge metadata\n");
    readme.push_str("# solverforge generate variable visit_order --entity Route --kind list --elements visits --domain cvrp\n\n");
    readme.push_str("# Enable bounded candidate-pull diagnostics when needed\n");
    readme.push_str("# solverforge config set candidate_trace.max_entries 100000\n\n");
    readme.push_str("# Remove a resource\n");
    readme.push_str("solverforge destroy constraint my_rule\n");
    readme.push_str("```\n\n");
    readme.push_str("## Project Structure\n\n");
    readme.push_str("| Directory | Purpose |\n");
    readme.push_str("|-----------|--------|\n");
    readme.push_str("| `src/domain/` | Planning entities, facts, and solution struct |\n");
    readme.push_str("| `src/constraints/` | Constraint definitions (scored by the solver) |\n");
    readme.push_str("| `src/solver/` | Solver service and configuration |\n");
    match shell {
        ScaffoldShell::Web | ScaffoldShell::Api => {
            readme.push_str("| `src/api/` | HTTP routes and DTOs |\n");
        }
        ScaffoldShell::Cli => {
            readme.push_str("| `src/api/dto.rs` | Shared JSON DTOs used by the CLI shell |\n");
        }
    }
    readme.push_str("| `src/data/` | Data loading and generation |\n");
    readme.push_str("| `solverforge.app.toml` | Scaffolded app/domain contract |\n");
    readme.push_str("| `solver.toml` | Solver configuration (termination, phases) |\n");
    if matches!(shell, ScaffoldShell::Web | ScaffoldShell::Api) {
        readme.push_str("\n## Runtime Diagnostics\n\n");
        readme.push_str("Status, snapshot, and SSE payloads expose the complete compact SolverForge telemetry surface. Candidate pulls remain separate from ordinary control-plane traffic. After enabling `candidate_trace.max_entries`, use `GET /jobs/{id}/telemetry` for the atomically retained bounded trace and `POST /jobs/qualified` for externally attested qualified trace jobs.\n");
    }
    readme
}

/// Converts a project name to a valid Rust crate name (underscores, lowercase).
fn to_crate_name(name: &str) -> String {
    name.chars()
        .map(|c| {
            if c == '-' {
                '_'
            } else {
                c.to_ascii_lowercase()
            }
        })
        .collect()
}
