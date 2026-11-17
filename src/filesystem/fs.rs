// src/utils/fs.rs

use std::process::Command;

use anyhow::{Context, Result as AnyhowResult};

pub fn mount(device: &str, target: &str, fs_type: Option<&str>) -> AnyhowResult<()> {
    std::fs::create_dir_all(target).context("failed to create mount point")?;

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

/// Mounts the kernel API filesystems of the initramfs (live system / early
/// boot). Done with mount(2) directly so that a failure reports its errno and
/// the very first boot stage does not depend on a `mount` binary.
pub fn mount_filesystems() -> AnyhowResult<()> {
    use libc::{MS_NODEV, MS_NOEXEC, MS_NOSUID, MS_STRICTATIME};

    let table: [(&str, &str, &str, libc::c_ulong, &str); 4] = [
        (
            "proc",
            "/proc",
            "proc",
            MS_NOSUID | MS_NODEV | MS_NOEXEC,
            "",
        ),
        (
            "sysfs",
            "/sys",
            "sysfs",
            MS_NOSUID | MS_NODEV | MS_NOEXEC,
            "",
        ),
        (
            "devtmpfs",
            "/dev",
            "devtmpfs",
            MS_NOSUID | MS_STRICTATIME,
            "mode=0755,size=4m",
        ),
        (
            "tmpfs",
            "/run",
            "tmpfs",
            MS_NOSUID | MS_NODEV | MS_STRICTATIME,
            "mode=0755",
        ),
    ];

    for (source, target, fs_type, flags, data) in table {
        std::fs::create_dir_all(target).with_context(|| format!("failed to create {target}"))?;

        // Already mounted (kernel automount of devtmpfs, rescue shell...):
        // nothing to do.
        if is_mount_point(target) {
            continue;
        }

        sys_mount(source, target, fs_type, flags, data)
            .with_context(|| format!("failed to mount {fs_type} on {target}"))?;
    }

    Ok(())
}

/* ============================================================
   Native mount(2) helpers (no dependency on a `mount` binary)
============================================================ */

/// Calls mount(2) directly. `data` is the filesystem-specific option string
/// (e.g. "mode=0755,size=4m"), empty for none.
///
/// The error carries the errno, which is what makes boot failures
/// diagnosable ("cgroup2: No such device" = kernel without CONFIG_CGROUPS).
pub fn sys_mount(
    source: &str,
    target: &str,
    fs_type: &str,
    flags: libc::c_ulong,
    data: &str,
) -> std::io::Result<()> {
    use std::ffi::CString;

    let invalid = |e| std::io::Error::new(std::io::ErrorKind::InvalidInput, e);
    let source = CString::new(source).map_err(invalid)?;
    let target = CString::new(target).map_err(invalid)?;
    let fs_type = CString::new(fs_type).map_err(invalid)?;
    let data = CString::new(data).map_err(invalid)?;

    let data_ptr = if data.as_bytes().is_empty() {
        std::ptr::null()
    } else {
        data.as_ptr() as *const libc::c_void
    };

    // SAFETY: all pointers come from live CStrings that outlive the call.
    let ret = unsafe {
        libc::mount(
            source.as_ptr(),
            target.as_ptr(),
            fs_type.as_ptr(),
            flags,
            data_ptr,
        )
    };

    if ret == 0 {
        Ok(())
    } else {
        Err(std::io::Error::last_os_error())
    }
}

/// True if `path` is a mount point (its device differs from its parent's).
pub fn is_mount_point(path: &str) -> bool {
    use std::os::unix::fs::MetadataExt;

    let path = std::path::Path::new(path);
    let (Ok(meta), Some(parent)) = (std::fs::symlink_metadata(path), path.parent()) else {
        return false;
    };
    let Ok(parent_meta) = std::fs::symlink_metadata(parent) else {
        return false;
    };
    meta.dev() != parent_meta.dev()
}

/// Detaches a mount (lazy umount). Errors are returned, never fatal by
/// themselves: callers decide.
pub fn lazy_unmount(target: &str) -> std::io::Result<()> {
    use std::ffi::CString;

    let target = CString::new(target)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidInput, e))?;

    // SAFETY: `target` is a live, NUL-terminated string.
    if unsafe { libc::umount2(target.as_ptr(), libc::MNT_DETACH) } == 0 {
        Ok(())
    } else {
        Err(std::io::Error::last_os_error())
    }
}

/// Copies a directory tree, keeping what a root filesystem needs:
///
/// * permission bits (including setuid/setgid/sticky) and ownership,
///   for files, directories AND symbolic links (never followed);
/// * symbolic links as links;
/// * hard links are copied as separate files (acceptable here).
///
/// Sockets, FIFOs and device nodes are skipped on purpose: copying a FIFO
/// with `fs::copy` would block forever, and /dev is a devtmpfs anyway.
/// `excluded` holds absolute source paths to leave out.
pub fn recursive_copy(source: &str, dest: &str, excluded: &[&str]) -> AnyhowResult<()> {
    use std::os::unix::fs::{MetadataExt, PermissionsExt, lchown};

    std::fs::create_dir_all(dest).with_context(|| format!("failed to create {dest}"))?;

    for entry in std::fs::read_dir(source).with_context(|| format!("failed to read {source}"))? {
        let entry = entry?;
        let source_path = entry.path();
        let source_str = source_path.to_string_lossy().to_string();

        if excluded.iter().any(|path| source_str == *path) {
            continue;
        }

        let dest_path = std::path::Path::new(dest).join(entry.file_name());
        let meta = std::fs::symlink_metadata(&source_path)
            .with_context(|| format!("failed to stat {source_str}"))?;
        let kind = meta.file_type();

        if kind.is_symlink() {
            let target = std::fs::read_link(&source_path)
                .with_context(|| format!("failed to read link {source_str}"))?;

            if std::fs::symlink_metadata(&dest_path).is_ok() {
                std::fs::remove_file(&dest_path)
                    .with_context(|| format!("failed to replace {}", dest_path.display()))?;
            }
            std::os::unix::fs::symlink(&target, &dest_path)
                .with_context(|| format!("failed to create link {}", dest_path.display()))?;
            lchown(&dest_path, Some(meta.uid()), Some(meta.gid()))
                .with_context(|| format!("failed to chown {}", dest_path.display()))?;
        } else if kind.is_dir() {
            recursive_copy(&source_str, &dest_path.to_string_lossy(), excluded)?;
            std::fs::set_permissions(
                &dest_path,
                std::fs::Permissions::from_mode(meta.mode() & 0o7777),
            )
            .with_context(|| format!("failed to chmod {}", dest_path.display()))?;
            lchown(&dest_path, Some(meta.uid()), Some(meta.gid()))
                .with_context(|| format!("failed to chown {}", dest_path.display()))?;
        } else if kind.is_file() {
            // Never write THROUGH an existing link at the destination.
            if std::fs::symlink_metadata(&dest_path).is_ok() {
                std::fs::remove_file(&dest_path)
                    .with_context(|| format!("failed to replace {}", dest_path.display()))?;
            }
            std::fs::copy(&source_path, &dest_path)
                .with_context(|| format!("failed to copy {source_str}"))?;
            // chown first: it clears setuid/setgid, chmod restores them.
            lchown(&dest_path, Some(meta.uid()), Some(meta.gid()))
                .with_context(|| format!("failed to chown {}", dest_path.display()))?;
            std::fs::set_permissions(
                &dest_path,
                std::fs::Permissions::from_mode(meta.mode() & 0o7777),
            )
            .with_context(|| format!("failed to chmod {}", dest_path.display()))?;
        }
        // Anything else (socket, FIFO, device node): skipped.
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

    let excluded = [source.path().join("excluded").to_string_lossy().to_string()];

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
