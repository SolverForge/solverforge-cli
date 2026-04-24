use std::fs;
use std::path::{Path, PathBuf};

pub const USE_LOCAL_PATCHES_ENV: &str = "SF_USE_LOCAL_PATCHES";
pub const ECOSYSTEM_ROOT_ENV: &str = "SF_ECOSYSTEM_ROOT";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DependencyOverrideMode {
    CratesIo,
    LocalPatches,
}

pub fn apply_generated_project_dependency_overrides(project_dir: &Path) -> DependencyOverrideMode {
    if !use_local_patches() {
        return DependencyOverrideMode::CratesIo;
    }

    let paths = LocalSolverforgePaths::resolve();
    let cargo_config = format!(
        "[patch.crates-io]\nsolverforge = {{ path = \"{}\" }}\nsolverforge-ui = {{ path = \"{}\" }}\nsolverforge-maps = {{ path = \"{}\" }}\n",
        toml_path(&paths.runtime_path),
        toml_path(&paths.ui_path),
        toml_path(&paths.maps_path)
    );
    let cargo_dir = project_dir.join(".cargo");
    fs::create_dir_all(&cargo_dir).expect("failed to create generated app .cargo dir");
    let cargo_config_path = cargo_dir.join("config.toml");
    if cargo_config_path.exists() {
        let existing = fs::read_to_string(&cargo_config_path)
            .expect("failed to read generated .cargo/config.toml");
        assert_eq!(
            existing, cargo_config,
            "generated app already has a different .cargo/config.toml; refusing to overwrite local Cargo patch config"
        );
    } else {
        fs::write(&cargo_config_path, cargo_config)
            .expect("failed to write generated app Cargo patch config");
    }

    DependencyOverrideMode::LocalPatches
}

fn use_local_patches() -> bool {
    matches!(
        std::env::var(USE_LOCAL_PATCHES_ENV),
        Ok(value) if matches!(value.to_ascii_lowercase().as_str(), "1" | "true" | "yes" | "on")
    )
}

struct LocalSolverforgePaths {
    runtime_path: PathBuf,
    ui_path: PathBuf,
    maps_path: PathBuf,
}

impl LocalSolverforgePaths {
    fn resolve() -> Self {
        let ecosystem_root = std::env::var(ECOSYSTEM_ROOT_ENV)
            .map(PathBuf::from)
            .unwrap_or_else(|_| {
                PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .parent()
                    .expect("CLI repo should have an ecosystem parent")
                    .to_path_buf()
            });

        Self {
            runtime_path: required_path(
                "solverforge",
                ecosystem_root
                    .join("solverforge-rs")
                    .join("crates")
                    .join("solverforge"),
            ),
            ui_path: required_path("solverforge-ui", ecosystem_root.join("solverforge-ui")),
            maps_path: required_path("solverforge-maps", ecosystem_root.join("solverforge-maps")),
        }
    }
}

fn required_path(label: &str, path: PathBuf) -> PathBuf {
    if !path.exists() {
        panic!(
            "{USE_LOCAL_PATCHES_ENV}=1 requested local Cargo patches, but {label} was not found at {}. Set {ECOSYSTEM_ROOT_ENV} or check out the sibling repo.",
            path.display()
        );
    }
    path.canonicalize().unwrap_or_else(|err| {
        panic!(
            "failed to canonicalize {} at {}: {err}",
            label,
            path.display()
        )
    })
}

fn toml_path(path: &Path) -> String {
    path.display().to_string().replace('\\', "/")
}
