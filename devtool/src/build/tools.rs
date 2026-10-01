//! Build tools (tools/<name>/) — equivalent of build_tools() in build.sh:
//! compiles every cargo tool found under tools/, then runs it from the
//! project root.

use anyhow::{bail, Result};
use std::process::Command;

use crate::paths::Paths;
use crate::proc::{capture, run, run_quiet};
use crate::ui;

/// Name of the binary produced by a crate, via `cargo metadata` (first
/// package, first binary) — avoids depending on python3 as build.sh did.
fn binary_name(manifest: &std::path::Path) -> Result<String> {
    let out = capture(Command::new("cargo").args([
        "metadata",
        "--format-version",
        "1",
        "--no-deps",
        "--manifest-path",
    ]).arg(manifest))?;

    let json: serde_json::Value = serde_json::from_str(&out)?;
    let packages = json["packages"].as_array().cloned().unwrap_or_default();
    for package in packages {
        if let Some(targets) = package["targets"].as_array() {
            for target in targets {
                let kinds = target["kind"].as_array().cloned().unwrap_or_default();
                if kinds.iter().any(|k| k == "bin") {
                    if let Some(name) = target["name"].as_str() {
                        return Ok(name.to_string());
                    }
                }
            }
        }
    }
    bail!("unable to determine compiled binary name for {}", manifest.display())
}

pub fn build(p: &Paths, jobs: usize) -> Result<()> {
    ui::info("Searching for Senbit build tools...");

    let tools_dir = p.tools_dir();
    if !tools_dir.is_dir() {
        ui::info("No tools directory found.");
        return Ok(());
    }

    let mut tool_dirs: Vec<_> = std::fs::read_dir(&tools_dir)?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|d| d.is_dir() && d.join("Cargo.toml").is_file())
        .collect();
    tool_dirs.sort();

    for tool_dir in &tool_dirs {
        let name = tool_dir.file_name().unwrap().to_string_lossy().to_string();
        let manifest = tool_dir.join("Cargo.toml");

        ui::section(&format!("Tool: {name}"));
        ui::info("Compiling...");

        let spin = ui::spinner(&format!("Compiling tool '{name}'..."));
        let result = run_quiet(Command::new("cargo")
            .current_dir(&p.root)
            .args(["build", "--release", "--manifest-path"])
            .arg(&manifest)
            .arg("-j")
            .arg(jobs.to_string()));
        spin.finish_and_clear();
        result?;

        let binary_name = binary_name(&manifest)?;
        let binary_path = tool_dir.join("target/release").join(&binary_name);

        if !binary_path.is_file() {
            bail!("compiled tool binary not found:\n  {}", binary_path.display());
        }

        ui::info("Running...");
        ui::detail(binary_path.display().to_string());
        run(Command::new(&binary_path).current_dir(&p.root))?;
    }

    println!();
    ui::ok("All Senbit build tools completed.");
    Ok(())
}