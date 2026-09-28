// installation/rootfs.rs

use anyhow::Result;

// ============================================================
// Misc functions imports
// ============================================================

use std::path::Path;

use crate::filesystem::fs::recursive_copy;

pub fn setup_rootfs(
    source: &Path,
    dest: &Path,
    root_uuid: &str,
) -> Result<()> {
    // Copy the rootfs from the ISO to the disk
    let excluded = [
        "/mnt",
        "/proc",
        "/sys",
        "/dev",
        "/run",
    ];

    recursive_copy(
        &source.to_string_lossy(),
        &dest.to_string_lossy(),
        &excluded,
    )?;

    make_sysconfig(dest, root_uuid)?;

    Ok(())
}

fn make_sysconfig(dest: &Path, root_uuid: &str) -> Result<()> {
    // /etc/fstab
    std::fs::write(
        dest.join("etc/fstab"),
        format!("UUID={} / ext4 defaults 0 1\n", root_uuid),
    )?;

    

    // /etc/senbit-release
    let version = env!("CARGO_PKG_VERSION");

    std::fs::write(
        dest.join("etc/senbit-release"),
        format!("NAME=Senbit\nVERSION={}\n", version),
    )?;

    Ok(())
}