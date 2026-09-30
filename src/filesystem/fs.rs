// src/utils/fs.rs

use std::process::Command;

use anyhow::{Context, Result as AnyhowResult};

pub fn mount(device: &str, target: &str, fs_type: Option<&str>) -> AnyhowResult<()> {
    std::fs::create_dir_all(target)
        .context("failed to create mount point")?;

    let mut command = Command::new("mount");

    if let Some(fs_type) = fs_type {
        command.args(["-t", fs_type]);
    }

    let status = command
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

pub fn recursive_copy(
    source: &str,
    dest: &str,
    excluded: &[&str],
) -> AnyhowResult<()> {
    std::fs::create_dir_all(dest)
        .context("failed to create destination directory")?;

    for file in std::fs::read_dir(source)
        .context("failed to read source directory")?
    {
        let file = file?;
        let source_path = file.path();
        let file_name = file.file_name();

        let source_path_str = source_path.to_string_lossy();

        // Skip excluded paths
        if excluded.iter().any(|path| source_path_str == *path) {
            continue;
        }

        let dest_path = std::path::Path::new(dest).join(&file_name);

        if source_path.is_dir() {
            recursive_copy(
                &source_path_str,
                &dest_path.to_string_lossy(),
                excluded,
            )?;
        } else {
            std::fs::copy(&source_path, &dest_path)
                .context("failed to copy file")?;
        }
    }

    Ok(())
}

#[test]
fn test_recursive_copy_excludes_paths() -> AnyhowResult<()> {
    use std::fs;
    use tempfile::tempdir;
    
    let source = tempdir()?;
    let dest = tempdir()?;

    fs::write(source.path().join("file.txt"), "hello")?;

    fs::create_dir(source.path().join("excluded"))?;
    fs::write(
        source.path().join("excluded").join("secret.txt"),
        "should not be copied",
    )?;

    let excluded = [
        source.path()
            .join("excluded")
            .to_string_lossy()
            .to_string(),
    ];

    let excluded_refs: Vec<&str> = excluded.iter().map(|s| s.as_str()).collect();

    recursive_copy(
        &source.path().to_string_lossy(),
        &dest.path().to_string_lossy(),
        &excluded_refs,
    )?;

    assert!(dest.path().join("file.txt").exists());
    assert!(!dest.path().join("excluded").exists());

    Ok(())
}