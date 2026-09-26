// src/utils/fs.rs

use std::process::Command;
use anyhow::{Context, Result as AnyhowResult};

pub fn mount(fstype: &str, name: &str) -> AnyhowResult<()> {
    let status = Command::new("mount")
        .arg("-t")
        .arg(fstype)
        .arg(name)
        .arg(&format!("/{}", name))
        .status()
        .context(format!("failed to run mount for /{}", name))?;

    if !status.success() {
        anyhow::bail!("mount /{} failed with status: {status:?}", name);
    }

    Ok(())
}

pub fn unmount(name: &str) -> AnyhowResult<()> {
    let status = Command::new("umount")
        .arg(name)
        .status()
        .context(format!("failed to run umount for {}", name))?;

    if !status.success() {
        anyhow::bail!("umount {} failed with status: {status:?}", name);
    }

    Ok(())
}

pub fn mount_filesystems() -> AnyhowResult<()> {
    mount("proc", "proc")?;
    mount("sysfs", "sys")?;
    mount("devtmpfs", "dev")?;
    mount("tmpfs", "run")?;
    Ok(())
}