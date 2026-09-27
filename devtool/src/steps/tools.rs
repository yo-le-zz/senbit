use std::process::Command;

use anyhow::{bail, Result};

use crate::common::*;

pub fn build_tools(p: &Paths) -> Result<()> {
    let tools_dir = p.join("tools");
    if !tools_dir.is_dir() {
        info("No tools directory found.");
        return Ok(());
    }

    info("Searching for Senbit build tools...");

    let mut tool_dirs: Vec<_> = std::fs::read_dir(&tools_dir)?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.is_dir() && p.join("Cargo.toml").is_file())
        .collect();
    tool_dirs.sort();

    if tool_dirs.is_empty() {
        info("No tools found.");
        return Ok(());
    }

    for tool_dir in tool_dirs {
        let name = tool_dir.file_name().unwrap().to_string_lossy().to_string();

        section(&format!("Tool: {name}"));

        info("Compiling...");
        let spin = spinner(&format!("Compiling tool '{name}'..."));
        let result = run(Command::new("cargo").current_dir(&tool_dir).args(["build", "--release"]));
        spin.finish_and_clear();
        result?;

        let metadata = capture(Command::new("cargo").current_dir(&tool_dir).args([
            "metadata",
            "--format-version",
            "1",
            "--no-deps",
        ]))?;

        let json: serde_json::Value = serde_json::from_str(&metadata)?;
        let package_name = json["packages"][0]["name"]
            .as_str()
            .unwrap_or(&name)
            .to_string();

        let binary = tool_dir.join("target/release").join(&package_name);
        if !binary.is_file() {
            bail!("compiled tool binary not found:\n  {}", binary.display());
        }

        println!();
        info(format!("Running {package_name}..."));
        run(Command::new(&binary).current_dir(&p.root))?;

        println!();
        ok(format!("Tool {package_name} completed."));
    }

    Ok(())
}
