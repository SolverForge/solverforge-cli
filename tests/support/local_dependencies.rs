use std::fs;
use std::path::{Path, PathBuf};

pub const USE_PUBLISHED_DEPS_ENV: &str = "SF_USE_PUBLISHED_DEPS";

pub fn pin_generated_project_to_local_solverforge(project_dir: &Path) -> bool {
    if use_published_deps() {
        return false;
    }

    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("CLI repo should have a workspace parent")
        .to_path_buf();
    let runtime_path = find_existing_path(&[workspace_root
        .join("solverforge-rs")
        .join("crates")
        .join("solverforge")]);
    let ui_path = find_existing_path(&[workspace_root.join("solverforge-ui")]);

    let (Some(runtime_path), Some(ui_path)) = (runtime_path, ui_path) else {
        return false;
    };

    let cargo_toml_path = project_dir.join("Cargo.toml");
    let cargo_toml = fs::read_to_string(&cargo_toml_path).expect("read generated Cargo.toml");
    let runtime_line = format!(
        "solverforge = {{ path = \"{}\", features = [\"serde\", \"console\", \"verbose-logging\"] }}",
        toml_path(&runtime_path)
    );
    let ui_line = format!("solverforge-ui = {{ path = \"{}\" }}", toml_path(&ui_path));

    let rewritten = cargo_toml
        .lines()
        .map(|line| {
            let trimmed = line.trim_start();
            if trimmed.starts_with("solverforge = ") {
                runtime_line.clone()
            } else if trimmed.starts_with("solverforge-ui = ") {
                ui_line.clone()
            } else {
                line.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n");

    fs::write(cargo_toml_path, rewritten + "\n").expect("write pinned generated Cargo.toml");
    true
}

fn use_published_deps() -> bool {
    matches!(
        std::env::var(USE_PUBLISHED_DEPS_ENV),
        Ok(value) if matches!(value.to_ascii_lowercase().as_str(), "1" | "true" | "yes" | "on")
    )
}

fn find_existing_path(candidates: &[PathBuf]) -> Option<PathBuf> {
    candidates.iter().find(|path| path.exists()).cloned()
}

fn toml_path(path: &Path) -> String {
    path.display().to_string().replace('\\', "/")
}
