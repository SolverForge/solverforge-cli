use include_dir::{include_dir, Dir};
use owo_colors::OwoColorize;
use std::path::Path;

use crate::template;

static BASIC_GENERIC_TEMPLATE: Dir = include_dir!("$CARGO_MANIFEST_DIR/templates/basic/generic");

static EMPLOYEE_SCHEDULING_TEMPLATE: Dir =
    include_dir!("$CARGO_MANIFEST_DIR/templates/basic/employee-scheduling");

static VEHICLE_ROUTING_TEMPLATE: Dir =
    include_dir!("$CARGO_MANIFEST_DIR/templates/list/vehicle-routing");

const AVAILABLE_TEMPLATES: &str = "
  Standard Variable (each entity holds one value):
    --basic                         — generic standard-variable skeleton
    --basic/employee-scheduling     — assign employees to shifts

  List Variable (each entity owns an ordered sequence):
    --list                          — generic list-variable skeleton  (coming soon)
    --list/vehicle-routing          — capacitated vehicle routing (CVRP)";

/// Scaffolds a new SolverForge project.
pub fn run(name: &str, template: Template) -> Result<(), String> {
    let crate_name = to_crate_name(name);
    match template {
        Template::Basic => scaffold(name, &crate_name, &BASIC_GENERIC_TEMPLATE, "basic"),
        Template::BasicEmployeeScheduling => {
            scaffold(name, &crate_name, &EMPLOYEE_SCHEDULING_TEMPLATE, "basic/employee-scheduling")
        }
        Template::List => Err(format!(
            "the generic list-variable skeleton is not yet available\n\nAvailable templates:{AVAILABLE_TEMPLATES}"
        )),
        Template::ListVehicleRouting => {
            scaffold(name, &crate_name, &VEHICLE_ROUTING_TEMPLATE, "list/vehicle-routing")
        }
    }
}

fn scaffold(
    project_name: &str,
    crate_name: &str,
    template_dir: &Dir,
    label: &str,
) -> Result<(), String> {
    let dest = Path::new(project_name);
    if dest.exists() {
        return Err(format!("directory '{}' already exists", project_name));
    }

    println!(
        "{} Creating {} project {}",
        "▸".bright_green(),
        label.bright_cyan(),
        project_name.bright_white().bold()
    );

    let vars: &[(&str, &str)] = &[
        ("project_name", project_name),
        ("crate_name", crate_name),
        ("solverforge_version", "0.5.2"),
    ];

    template::render(template_dir, dest, vars)?;

    println!();
    println!("{}", "  Done! Your project is ready.".bright_green().bold());
    println!();
    println!("  Next steps:");
    println!("    {} {}", "cd".bright_black(), project_name.bright_cyan());
    println!(
        "    {} {}  {}",
        "$".bright_black(),
        "solverforge server".bright_cyan(),
        "# start the solver".bright_black()
    );
    println!(
        "    {}",
        "Open http://localhost:7860 in your browser".bright_black()
    );
    println!();

    Ok(())
}

pub enum Template {
    Basic,
    BasicEmployeeScheduling,
    List,
    ListVehicleRouting,
}

impl Template {
    pub fn parse(basic: bool, list: bool, specialization: Option<&str>) -> Result<Self, String> {
        match (basic, list, specialization) {
            (true, false, None) => Ok(Template::Basic),
            (true, false, Some("employee-scheduling")) => Ok(Template::BasicEmployeeScheduling),
            (false, true, None) => Ok(Template::List),
            (false, true, Some("vehicle-routing")) => Ok(Template::ListVehicleRouting),
            (false, false, None) => Err(format!(
                "specify a template flag\n\nAvailable templates:{AVAILABLE_TEMPLATES}"
            )),
            (true, true, _) => Err("--basic and --list are mutually exclusive".to_string()),
            (_, _, Some(s)) => Err(format!(
                "unknown specialization: '{}'\n\nAvailable templates:{AVAILABLE_TEMPLATES}",
                s
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
