use anyhow::{Context, Result};
use colored::Colorize;
use std::fs;
use std::path::Path;

pub fn stage(input: &Path, output: &Path) -> Result<()> {
    let stage = input.join("stage");

    if !stage.exists() {
        anyhow::bail!(
            "GRUB staging directory does not exist: {}",
            stage.display()
        );
    }

    let generated_rootfs = output.join("rootfs");

    if generated_rootfs.exists() {
        fs::remove_dir_all(&generated_rootfs)
            .context("Failed to clean previous GRUB rootfs staging")?;
    }

    fs::create_dir_all(&generated_rootfs)
        .context("Failed to create GRUB rootfs staging directory")?;

    copy_recursive(&stage, &generated_rootfs)?;

    println!(
        "{} {}",
        "GRUB resources staged in".green(),
        generated_rootfs.display()
    );

    // Merge the generated GRUB rootfs into the final Senbit rootfs.
    let final_rootfs = Path::new("build/rootfs");

    fs::create_dir_all(final_rootfs)
        .context("Failed to create final rootfs directory")?;

    copy_recursive(&generated_rootfs, final_rootfs)?;

    println!(
        "{} {}",
        "GRUB resources copied into final rootfs:".green(),
        final_rootfs.display()
    );

    Ok(())
}

fn copy_recursive(source: &Path, destination: &Path) -> Result<()> {
    fs::create_dir_all(destination)?;

    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let source_path = entry.path();
        let destination_path = destination.join(entry.file_name());

        if source_path.is_dir() {
            copy_recursive(&source_path, &destination_path)?;
        } else {
            fs::copy(&source_path, &destination_path)
                .with_context(|| {
                    format!(
                        "Failed to copy {} to {}",
                        source_path.display(),
                        destination_path.display()
                    )
                })?;
        }
    }

    Ok(())
}