use anyhow::{Context, Result};
use std::fs;
use std::process::Command;

use crate::paths::Paths;
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
        // -R 0:0 : tout appartient à root dans l'initramfs. Sans cela cpio
        // enregistre l'UID de l'utilisateur qui compile (ex. 1000) : les
        // fichiers système (et les binaires setuid) du système installé
        // appartiendraient alors à un utilisateur normal.
        .arg("find . -print0 | cpio --null -o -H newc -R 0:0 | gzip -9")
        .stdout(
            std::fs::File::create(&initramfs)
                .with_context(|| format!("Failed to create {}", initramfs.display()))?,
        )
        .status()
        .context("Failed to execute cpio/gzip")?;

    if !status.success() {
        anyhow::bail!("Failed to create initramfs: {}", initramfs.display());
    }

    ui::info(&format!("Initramfs created: {}", initramfs.display()));

    Ok(())
}
