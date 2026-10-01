use anyhow::{Context, Result};
use std::fs;
use std::process::Command;

use crate::paths::Paths;
use crate::proc::run;
use crate::ui;

pub fn build(p: &Paths, rootfs: &std::path::Path) -> Result<()> {
    let initramfs = p.initramfs();

    ui::info("Creating initramfs...");

    if let Some(parent) = initramfs.parent() {
        fs::create_dir_all(parent)?;
    }

    let status = Command::new("sh")
        .current_dir(rootfs)
        .arg("-c")
        .arg(
            "find . -print0 | cpio --null -o -H newc | gzip -9",
        )
        .stdout(
            std::fs::File::create(&initramfs)
                .with_context(|| {
                    format!(
                        "Failed to create {}",
                        initramfs.display()
                    )
                })?,
        )
        .status()
        .context("Failed to execute cpio/gzip")?;

    if !status.success() {
        anyhow::bail!(
            "Failed to create initramfs: {}",
            initramfs.display()
        );
    }

    ui::info(&format!(
        "Initramfs created: {}",
        initramfs.display()
    ));

    Ok(())
}