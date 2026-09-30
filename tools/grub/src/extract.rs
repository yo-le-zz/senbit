use anyhow::{Context, Result};
use colored::Colorize;
use std::fs::File;
use std::path::{Path, PathBuf};

use tar::Archive;
use xz2::read::XzDecoder;

pub fn extract(archive_path: &Path, output: &Path) -> Result<PathBuf> {
    let source_dir = output.join("source");

    if source_dir.exists() {
        println!(
            "{} {}",
            "GRUB source already extracted:".cyan(),
            source_dir.display()
        );

        return Ok(source_dir);
    }

    println!("{}", "Extracting GRUB source...".cyan());

    let file = File::open(archive_path)
        .context("Failed to open GRUB archive")?;

    let decoder = XzDecoder::new(file);
    let mut archive = Archive::new(decoder);

    archive
        .unpack(output)
        .context("Failed to extract GRUB archive")?;

    let mut extracted = None;

    for entry in std::fs::read_dir(output)? {
        let entry = entry?;
        let path = entry.path();

        if path.is_dir()
            && path
                .file_name()
                .and_then(|x| x.to_str())
                .map(|x| x.starts_with("grub-"))
                .unwrap_or(false)
        {
            extracted = Some(path);
            break;
        }
    }

    let extracted = extracted
        .context("Could not find extracted GRUB source directory")?;

    std::fs::rename(&extracted, &source_dir)
        .context("Failed to rename GRUB source directory")?;

    println!(
        "{} {}",
        "GRUB source extracted to".green(),
        source_dir.display()
    );

    Ok(source_dir)
}