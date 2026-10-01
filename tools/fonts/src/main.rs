use anyhow::{Context, Result};
use colored::Colorize;
use std::fs;
use std::path::{Path, PathBuf};

const FONT_URL: &str =
    "https://deb.debian.org/debian/pool/main/c/console-setup/console-setup-linux_1.242~deb13u1_all.deb";

const FONT_NAME: &str = "Uni3-Terminus16.psf.gz";

const OUTPUT_PATH: &str = "build/tools/generated/fonts";

fn download_package(destination: &Path) -> Result<()> {
    println!(
        "{} {}",
        "Downloading console fonts:".cyan().bold(),
        FONT_URL
    );

    let response = reqwest::blocking::get(FONT_URL)
        .context("Failed to download console font package")?;

    if !response.status().is_success() {
        anyhow::bail!(
            "Font package download failed with HTTP status {}",
            response.status()
        );
    }

    let data = response
        .bytes()
        .context("Failed to read font package response")?;

    fs::write(destination, data)
        .with_context(|| {
            format!(
                "Failed to write {}",
                destination.display()
            )
        })?;

    Ok(())
}

fn extract_font(
    package: &Path,
    output: &Path,
) -> Result<()> {
    let extract_dir =
        output.join(".extract");

    if extract_dir.exists() {
        fs::remove_dir_all(&extract_dir)
            .context("Failed to clean font extraction directory")?;
    }

    fs::create_dir_all(&extract_dir)
        .context("Failed to create font extraction directory")?;

    println!(
        "{}",
        "Extracting console font package..."
            .cyan()
            .bold()
    );

    let status = std::process::Command::new("dpkg-deb")
        .args([
            "-x",
            package
                .to_str()
                .context("Invalid package path")?,
            extract_dir
                .to_str()
                .context("Invalid extraction path")?,
        ])
        .status()
        .context("Failed to execute dpkg-deb")?;

    if !status.success() {
        anyhow::bail!(
            "dpkg-deb exited with status {}",
            status
        );
    }

    let source = extract_dir
        .join("usr/share/consolefonts")
        .join(FONT_NAME);

    if !source.exists() {
        anyhow::bail!(
            "Font {} was not found in the package",
            FONT_NAME
        );
    }

    let destination =
        output.join(FONT_NAME);

    fs::copy(&source, &destination)
        .with_context(|| {
            format!(
                "Failed to install {}",
                destination.display()
            )
        })?;

    fs::remove_dir_all(&extract_dir)
        .context("Failed to remove extraction directory")?;

    Ok(())
}

fn run() -> Result<()> {
    let output =
        PathBuf::from(OUTPUT_PATH);

    fs::create_dir_all(&output)
        .context("Failed to create font output directory")?;

    let font =
        output.join(FONT_NAME);

    if font.exists() {
        println!(
            "{} {}",
            "Console font already exists:"
                .green()
                .bold(),
            font.display()
        );

        return Ok(());
    }

    let cache_dir =
        PathBuf::from("build/tools/cache/fonts");

    fs::create_dir_all(&cache_dir)
        .context("Failed to create font cache directory")?;

    let package =
        cache_dir.join("console-setup-linux.deb");

    if package.exists() {
        println!(
            "{} {}",
            "Using cached font package:"
                .cyan(),
            package.display()
        );
    } else {
        download_package(&package)?;
    }

    extract_font(
        &package,
        &output,
    )?;

    println!(
        "{} {}",
        "Console font generated:"
            .green()
            .bold(),
        font.display()
    );

    Ok(())
}

fn main() {
    if let Err(error) = run() {
        eprintln!(
            "{}: {:#}",
            "Fonts build failed".red().bold(),
            error
        );

        std::process::exit(1);
    }
}