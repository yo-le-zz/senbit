mod build;
mod config;
mod download;
mod extract;
mod stage;

use anyhow::{Context, Result};
use colored::Colorize;
use std::path::Path;

const CONFIG_PATH: &str = "tools/grub/config.toml";
const OUTPUT_PATH: &str = "build/tools/generated/grub";

fn fix_missing_files(source: &Path) -> Result<()> {
    let extra_deps = source.join("grub-core/extra_deps.lst");

    if !extra_deps.exists() {
        std::fs::write(
            &extra_deps,
            "depends bli part_gpt\n",
        )
        .with_context(|| {
            format!(
                "Failed to create {}",
                extra_deps.display()
            )
        })?;

        println!(
            "{} {}",
            "Created missing GRUB file:".green(),
            extra_deps.display()
        );
    } else {
        println!(
            "{} {}",
            "GRUB file already exists:".cyan(),
            extra_deps.display()
        );
    }

    Ok(())
}

fn run() -> Result<()> {
    let config = config::Config::load(CONFIG_PATH)?;

    let output = Path::new(OUTPUT_PATH);

    std::fs::create_dir_all(output)
        .context("Failed to create GRUB output directory")?;

    println!(
        "{}",
        format!(
            "Preparing GRUB {}...",
            config.grub.version
        )
        .cyan()
        .bold()
    );

    download::download(&config, output)?;

    let archive = output.join(format!(
        "grub-{}.tar.xz",
        config.grub.version
    ));

    let source = extract::extract(
        &archive,
        output,
    )?;

    fix_missing_files(&source)?;

    build::build(
        &config,
        &source,
        output,
    )?;

    stage::stage(
        output,
        output,
    )?;

    println!(
        "{}",
        "GRUB resources generated successfully!"
            .green()
            .bold()
    );

    Ok(())
}

fn main() {
    if let Err(error) = run() {
        eprintln!(
            "{}: {:#}",
            "GRUB build failed".red().bold(),
            error
        );

        std::process::exit(1);
    }
}