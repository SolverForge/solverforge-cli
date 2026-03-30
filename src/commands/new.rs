use include_dir::{include_dir, Dir};
use std::path::Path;
use std::process::Command;

use crate::error::{is_rust_keyword, CliError, CliResult};
use crate::output;
use crate::template;

static STANDARD_TEMPLATE: Dir = include_dir!("$CARGO_MANIFEST_DIR/templates/standard/generic");

static LIST_GENERIC_TEMPLATE: Dir = include_dir!("$CARGO_MANIFEST_DIR/templates/list/generic");

const AVAILABLE_TEMPLATES: &str = "
  Standard Starter (sample app with standard variables):
    --standard                      — generic standard starter skeleton

  List Starter (sample app with list variables):
    --list                          — generic list starter skeleton";

pub fn run(
    name: &str,
    template: Template,
    skip_git: bool,
    skip_readme: bool,
    quiet: bool,
) -> CliResult {
    let crate_name = to_crate_name(name);

    // Validate project name
    validate_project_name(name, &crate_name)?;

    match template {
        Template::Standard => scaffold(
            name,
            &crate_name,
            &STANDARD_TEMPLATE,
            "standard",
            skip_git,
            skip_readme,
            quiet,
        ),
        Template::List => scaffold(
            name,
            &crate_name,
            &LIST_GENERIC_TEMPLATE,
            "list",
            skip_git,
            skip_readme,
            quiet,
        ),
    }
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
        ("solverforge_version", env!("CARGO_PKG_VERSION")),
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

    // Template-specific guidance
    print_template_guidance(project_name, label);

    // Optional cargo check prompt (skipped in quiet mode)
    if !quiet {
        run_cargo_check_prompt(dest)?;
    }

    Ok(())
}

fn solverforge_dep_spec() -> String {
    format!(
        "{{ version = \"{}\", features = [\"serde\", \"console\", \"verbose-logging\"] }}",
        env!("CARGO_PKG_VERSION")
    )
}

fn solverforge_ui_dep_spec() -> String {
    "\"0.3.1\"".to_string()
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

fn print_template_guidance(project_name: &str, label: &str) {
    if output::is_quiet() {
        return;
    }

    println!("  Next steps:");
    println!("    cd {}", project_name);

    match label {
        "standard" => {
            println!("    solverforge server");
            println!();
            println!("  This template includes:");
            println!("    - Standard starter domain skeleton with field-declared variables");
            println!("    - Assignment-board UI composed from solverforge-ui primitives");
            println!("    - REST API with SSE live updates and score analysis");
            println!("    - solver.toml as the search-strategy layer");
            println!("    solverforge generate entity task --planning-variable resource_idx");
            println!("    solverforge generate fact resource");
            println!("    solverforge generate constraint all_assigned --unary --hard");
        }
        "list" => {
            println!("    solverforge server");
            println!();
            println!("  This template includes:");
            println!("    - List starter domain skeleton with field-declared list variables");
            println!("    - Balanced load constraint (soft)");
            println!("    - Sequence view composed from solverforge-ui primitives");
            println!("    - REST API with SSE live updates and score analysis");
            println!("    - solver.toml as the search-strategy layer");
        }
        _ => {
            println!("    solverforge server");
        }
    }

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
        "A SolverForge constraint optimization project (template: `{}`).\n\n",
        label
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
    readme.push_str("| `solver.toml` | Solver configuration (termination, phases) |\n");
    readme
}

pub enum Template {
    Standard,
    List,
}

impl Template {
    pub fn parse(standard: bool, list: bool) -> CliResult<Self> {
        match (standard, list) {
            (true, false) => Ok(Template::Standard),
            (false, true) => Ok(Template::List),
            (false, false) => Err(CliError::with_hint(
                "specify a template flag",
                format!("Available templates:{AVAILABLE_TEMPLATES}"),
            )),
            (true, true) => Err(CliError::general(
                "--standard and --list are mutually exclusive",
            )),
        }
    }
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
