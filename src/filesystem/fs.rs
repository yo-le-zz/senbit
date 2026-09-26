// src/utils/fs.rs

use std::process::Command;

use anyhow::{Context, Result as AnyhowResult};

pub fn mount(device: &str, target: &str) -> AnyhowResult<()> {
    std::fs::create_dir_all(target)
        .context("failed to create mount point")?;

    let status = Command::new("mount")
        .args([device, target])
        .status()
        .context("failed to run mount")?;

    if status.success() {
        Ok(())
    } else {
        anyhow::bail!("mount exited with status: {}", status)
    }
}

pub fn unmount(target: &str) -> AnyhowResult<()> {
    let status = Command::new("umount")
        .arg(target)
        .status()
        .context("failed to run umount")?;

    if status.success() {
        Ok(())
    } else {
        anyhow::bail!("umount exited with status: {}", status)
    }
}

fn mount_filesystem(
    filesystem_type: &str,
    source: &str,
    target: &str,
) -> AnyhowResult<()> {
    std::fs::create_dir_all(target)
        .context(format!("failed to create {}", target))?;

    let status = Command::new("mount")
        .args(["-t", filesystem_type, source, target])
        .status()
        .context(format!("failed to mount {}", target))?;

    if status.success() {
        Ok(())
    } else {
        anyhow::bail!(
            "mount {} failed with status: {}",
            target,
            status
        )
    }
}

pub fn mount_filesystems() -> AnyhowResult<()> {
    mount_filesystem("proc", "proc", "/proc")?;
    mount_filesystem("sysfs", "sysfs", "/sys")?;
    mount_filesystem("devtmpfs", "devtmpfs", "/dev")?;
    mount_filesystem("tmpfs", "tmpfs", "/run")?;

    Ok(())
}