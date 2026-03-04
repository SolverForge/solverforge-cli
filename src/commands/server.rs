use owo_colors::OwoColorize;
use std::process::Command;

/// Runs `cargo run --release` — the `solverforge server` command.
pub fn run() -> Result<(), String> {
    println!("{} Starting SolverForge server...", "▸".bright_green());
    println!(
        "{} Running {}",
        "▸".bright_green(),
        "cargo run --release".bright_black()
    );

    let status = Command::new("cargo")
        .args(["run", "--release"])
        .status()
        .map_err(|e| format!("failed to run cargo: {}", e))?;

    if status.success() {
        Ok(())
    } else {
        Err("cargo run --release failed".to_string())
    }
}
