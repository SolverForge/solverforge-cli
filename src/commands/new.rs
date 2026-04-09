use include_dir::{include_dir, Dir};
use std::path::Path;
use std::process::Command;

use crate::error::{is_rust_keyword, CliError, CliResult};
use crate::output;
use crate::scaffold_target::{
    RUNTIME_SOURCE_PATH, RUNTIME_TARGET_DISPLAY, RUNTIME_TARGET_LABEL, UI_SOURCE_PATH,
};
use crate::template;

// Keep the neutral scaffold embedded so generated apps are self-contained at build time.
static UNIFIED_TEMPLATE: Dir = include_dir!("$CARGO_MANIFEST_DIR/templates/standard/generic");

pub fn run(name: &str, skip_git: bool, skip_readme: bool, quiet: bool) -> CliResult {
    let crate_name = to_crate_name(name);

    // Validate project name
    validate_project_name(name, &crate_name)?;

    scaffold(
        name,
        &crate_name,
        &UNIFIED_TEMPLATE,
        "neutral shell",
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
    label: &str,
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

    output::print_heading(&format!("Creating {} project '{}'", label, project_name));

    let vars: &[(&str, &str)] = &[
        ("solverforge_dep", &solverforge_dep_spec()),
        ("solverforge_ui_dep", &solverforge_ui_dep_spec()),
        ("project_name", project_name),
        ("crate_name", crate_name),
        ("solverforge_cli_version", env!("CARGO_PKG_VERSION")),
        ("solverforge_runtime_target", RUNTIME_TARGET_LABEL),
        ("solverforge_runtime_source", RUNTIME_SOURCE_PATH),
        ("solverforge_ui_source", UI_SOURCE_PATH),
    ];

    template::render(template_dir, dest, vars)?;

    // Write .gitignore
    let gitignore_content = "/target\n**/*.rs.bk\nCargo.lock\n";
    std::fs::write(dest.join(".gitignore"), gitignore_content).map_err(|e| CliError::IoError {
        context: "failed to write .gitignore".to_string(),
        source: e,
    })?;
    output::print_create(".gitignore");

    if !skip_readme {
        // Write README.md
        let readme = generate_readme(project_name, crate_name, label);
        std::fs::write(dest.join("README.md"), readme).map_err(|e| CliError::IoError {
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

    print_template_guidance(project_name);

    // Optional cargo check prompt (skipped in quiet mode)
    if !quiet {
        run_cargo_check_prompt(dest)?;
    }

    Ok(())
}

fn solverforge_dep_spec() -> String {
    "{ version = \"0.8.1\", features = [\"serde\", \"console\", \"verbose-logging\"] }".to_string()
}

fn solverforge_ui_dep_spec() -> String {
    "{ version = \"0.4.2\" }".to_string()
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

fn print_template_guidance(project_name: &str) {
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

    println!("    solverforge server");
    println!();
    println!("  This starter includes:");
    println!("    - One neutral app shell for standard, list, or mixed modeling");
    println!("    - Variable-driven views generated from solverforge.app.toml");
    println!("    - Retained job lifecycle with pause, resume, cancel, and delete");
    println!("    - Typed SSE lifecycle events and snapshot-bound score analysis");
    println!("    - solverforge.app.toml for the scaffolded domain contract");
    println!("    - solver.toml as the search-strategy layer");
    println!("    solverforge generate entity task");
    println!("    solverforge generate fact resource");
    println!("    solverforge generate variable resource_idx --entity Task --kind standard --range resources --allows-unassigned");

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

fn generate_readme(project_name: &str, _crate_name: &str, label: &str) -> String {
    let mut readme = format!("# {}\n\n", project_name);
    readme.push_str(&format!(
        "A SolverForge constraint optimization project (starter: `{}`).\n\n",
        label
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
    readme.push_str(&format!(
        "- Runtime dependency currently wired into `Cargo.toml`: `{}`\n\n",
        RUNTIME_SOURCE_PATH
    ));
    readme.push_str(&format!(
        "- Frontend UI dependency currently wired into `Cargo.toml`: `{}`\n\n",
        UI_SOURCE_PATH
    ));
    readme.push_str(&format!(
        "This project was scaffolded by `solverforge-cli`, and it currently targets `{}` through the configured crate dependency targets.\n\n",
        RUNTIME_TARGET_DISPLAY
    ));
    readme.push_str("## Quick Start\n\n");
    readme.push_str("```bash\n");
    readme.push_str("# Start the solver server\n");
    readme.push_str("solverforge server\n\n");
    readme.push_str("# Or run directly\n");
    readme.push_str("cargo run --release\n");
    readme.push_str("```\n\n");
    readme.push_str("## Development\n\n");
    readme.push_str("```bash\n");
    readme.push_str("# Add a new constraint\n");
    readme.push_str("solverforge generate constraint my_rule --unary --hard\n\n");
    readme.push_str("# Add a domain entity\n");
    readme.push_str("solverforge generate entity worker --planning-variable shift_idx\n\n");
    readme.push_str("# Add a problem fact\n");
    readme.push_str("solverforge generate fact location\n\n");
    readme.push_str("# Remove a resource\n");
    readme.push_str("solverforge destroy constraint my_rule\n");
    readme.push_str("```\n\n");
    readme.push_str("## Project Structure\n\n");
    readme.push_str("| Directory | Purpose |\n");
    readme.push_str("|-----------|--------|\n");
    readme.push_str("| `src/domain/` | Planning entities, facts, and solution struct |\n");
    readme.push_str("| `src/constraints/` | Constraint definitions (scored by the solver) |\n");
    readme.push_str("| `src/solver/` | Solver service and configuration |\n");
    readme.push_str("| `src/api/` | HTTP routes and DTOs |\n");
    readme.push_str("| `src/data/` | Data loading and generation |\n");
    readme.push_str("| `solverforge.app.toml` | Scaffolded app/domain contract |\n");
    readme.push_str("| `solver.toml` | Solver configuration (termination, phases) |\n");
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
