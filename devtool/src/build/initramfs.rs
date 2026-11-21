//! Creates `build/initramfs.cpio.gz` from the root filesystem.
//!
//! The root filesystem is big (glibc, systemd, apt, ...): compressing it is the slowest
//! step of `devtool build`, so:
//!   * nothing is done when the content of the root filesystem did not change (a signature
//!     of every path, type, mode, link target and file content is kept in `build/.state/`);
//!   * `pigz` is used when it is installed, and gzip level 6 otherwise (level 9 only
//!     costs a lot of time for a few percent).

use anyhow::{bail, Context, Result};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::host;
use crate::paths::Paths;
use crate::ui;

/// Changing the compression changes the signature: the archive is rebuilt.
const COMPRESSION: &str = "gzip-6";

fn signature_file(p: &Paths) -> PathBuf {
    p.state_file().with_file_name("initramfs.sha256")
}

/// Hash of everything that ends up in the archive (reading the files takes a few seconds,
/// compressing them takes minutes).
fn rootfs_signature(rootfs: &Path) -> Result<String> {
    let output = Command::new("sh")
        .current_dir(rootfs)
        .arg("-c")
        // Paths, types, modes and link targets, then the SHA-256 of every file. The dates
        // are not used: the root filesystem is copied again at each build.
        .arg(
            r"(find . -printf '%p\t%y\t%m\t%l\n' | LC_ALL=C sort; \
               find . -type f -print0 | LC_ALL=C sort -z | xargs -0 -r sha256sum)",
        )
        .output()
        .context("Failed to list the root filesystem (find/sort)")?;

    if !output.status.success() {
        bail!("Failed to list the root filesystem for the initramfs signature");
    }

    let mut hasher = Sha256::new();
    hasher.update(COMPRESSION.as_bytes());
    hasher.update(&output.stdout);
    Ok(hasher
        .finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect())
}

pub fn build(p: &Paths, rootfs: &Path) -> Result<()> {
    let initramfs = p.initramfs();
    let signature_path = signature_file(p);

    if let Some(parent) = initramfs.parent() {
        fs::create_dir_all(parent)?;
    }
    if let Some(parent) = signature_path.parent() {
        fs::create_dir_all(parent)?;
    }

    let signature = rootfs_signature(rootfs)?;
    let up_to_date = initramfs.is_file()
        && fs::metadata(&initramfs).map(|m| m.len() > 0).unwrap_or(false)
        && fs::read_to_string(&signature_path)
            .map(|saved| saved.trim() == signature)
            .unwrap_or(false);

    if up_to_date {
        ui::skip(&format!(
            "Initramfs is up to date (root filesystem unchanged): {}",
            initramfs.display()
        ));
        return Ok(());
    }

    ui::info("Creating initramfs...");

    let compressor = if host::find_tool("pigz").is_some() {
        "pigz -6"
    } else {
        "gzip -6"
    };
    ui::detail(format!("Compression: {compressor}"));

    let spin = ui::spinner("Packing and compressing the root filesystem...");

    let _ = fs::remove_file(&signature_path);

    let status = Command::new("sh")
        .current_dir(rootfs)
        .arg("-c")
        // -R 0:0 : tout appartient à root dans l'initramfs. Sans cela cpio
        // enregistre l'UID de l'utilisateur qui compile (ex. 1000) : les
        // fichiers système (et les binaires setuid) du système installé
        // appartiendraient alors à un utilisateur normal.
        .arg(format!(
            "find . -print0 | cpio --null -o -H newc -R 0:0 --quiet | {compressor}"
        ))
        .stdout(
            fs::File::create(&initramfs)
                .with_context(|| format!("Failed to create {}", initramfs.display()))?,
        )
        .status()
        .context("Failed to execute cpio/gzip");

    spin.finish_and_clear();
    let status = status?;

    let size = fs::metadata(&initramfs).map(|m| m.len()).unwrap_or(0);
    if !status.success() || size == 0 {
        let _ = fs::remove_file(&initramfs);
        bail!("Failed to create initramfs: {}", initramfs.display());
    }

    fs::write(&signature_path, format!("{signature}\n"))
        .with_context(|| format!("Failed to write {}", signature_path.display()))?;

    ui::info(&format!("Initramfs created: {}", initramfs.display()));

    Ok(())
}
