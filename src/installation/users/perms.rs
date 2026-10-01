// installation/users/perms.rs

use anyhow::{
    Context,
    Result,
};

use std::fs;

use std::os::unix::fs::PermissionsExt;

use std::path::Path;

use std::process::Command;

/* ============================================================
   Basic permission helpers
============================================================ */

fn set_mode(
    path: &Path,
    mode: u32,
) -> Result<()> {
    if !path.exists() {
        return Ok(());
    }

    fs::set_permissions(
        path,
        fs::Permissions::from_mode(mode),
    )
    .with_context(|| {
        format!(
            "failed to set permissions on {}",
            path.display()
        )
    })?;

    Ok(())
}

fn set_x_perm(
    path: &Path,
) -> std::io::Result<()> {
    let metadata =
        fs::symlink_metadata(path)?;

    /*
     * Never modify symlinks themselves.
     */
    if metadata.file_type().is_symlink()
        || !metadata.is_file()
    {
        return Ok(());
    }

    let mut permissions =
        metadata.permissions();

    permissions.set_mode(
        0o755
    );

    fs::set_permissions(
        path,
        permissions,
    )
}

/* ============================================================
   Executable permissions
============================================================ */

pub fn set_system_x_permissions(
    root: &Path,
) -> std::io::Result<()> {
    for dir in [
        "bin",
        "sbin",
        "usr/bin",
        "usr/sbin",
    ] {
        let path =
            root.join(dir);

        if !path.is_dir() {
            continue;
        }

        for entry in fs::read_dir(path)? {
            set_x_perm(
                &entry?.path()
            )?;
        }
    }

    Ok(())
}

/* ============================================================
   Ownership
============================================================ */

fn chown(
    path: &Path,
    uid: u32,
    gid: u32,
) -> Result<()> {
    let status =
        Command::new("chown")
            .args([
                &format!(
                    "{}:{}",
                    uid,
                    gid
                ),
                "--",
            ])
            .arg(path)
            .status()
            .with_context(|| {
                format!(
                    "failed to chown {}",
                    path.display()
                )
            })?;

    if !status.success() {
        anyhow::bail!(
            "chown failed for {}",
            path.display()
        );
    }

    Ok(())
}

fn chown_recursive(
    path: &Path,
    uid: u32,
    gid: u32,
) -> Result<()> {
    let metadata =
        fs::symlink_metadata(path)?;

    /*
     * Never follow symlinks.
     */
    if metadata.file_type().is_symlink() {
        return Ok(());
    }

    /*
     * /home is special.
     *
     * Its contents belong to individual users, so the global
     * root ownership pass must never recurse into it.
     */
    if path.file_name()
        .and_then(|name| name.to_str())
        == Some("home")
    {
        return Ok(());
    }

    chown(
        path,
        uid,
        gid,
    )?;

    if metadata.is_dir() {
        for entry in fs::read_dir(path)? {
            chown_recursive(
                &entry?.path(),
                uid,
                gid,
            )?;
        }
    }

    Ok(())
}

/* ============================================================
   System ownership
============================================================ */

pub fn set_proprietary_permissions(
    mount_point: &Path,
) -> Result<()> {
    /*
     * Everything except /home belongs to root.
     */
    chown_recursive(
        mount_point,
        0,
        0,
    )?;

    /*
     * /home itself belongs to root.
     */
    let home =
        mount_point.join("home");

    if home.exists() {
        chown(
            &home,
            0,
            0,
        )?;
    }

    Ok(())
}

/* ============================================================
   Basic user permissions
============================================================ */

pub fn set_basic_user_permissions(
    mount_point: &Path,
) -> Result<()> {
    /*
     * ========================================================
     * System directories
     * ========================================================
     */

    for dir in [
        "bin",
        "sbin",
        "usr",
        "usr/bin",
        "usr/sbin",
        "usr/lib",
        "usr/share",
        "etc",
        "var",
        "var/log",
        "var/lib",
    ] {
        let path =
            mount_point.join(dir);

        set_mode(
            &path,
            0o755,
        )?;
    }

    /*
     * ========================================================
     * Runtime directories
     * ========================================================
     */

    for dir in [
        "run",
        "dev",
        "proc",
        "sys",
    ] {
        let path =
            mount_point.join(dir);

        if path.exists() {
            set_mode(
                &path,
                0o755,
            )?;
        }
    }

    /*
     * ========================================================
     * /root
     * ========================================================
     */

    let root_home =
        mount_point.join("root");

    if root_home.exists() {
        set_mode(
            &root_home,
            0o700,
        )?;

        chown(
            &root_home,
            0,
            0,
        )?;
    }

    /*
     * ========================================================
     * /home
     * ========================================================
     */

    let home =
        mount_point.join("home");

    if home.exists() {
        set_mode(
            &home,
            0o755,
        )?;

        chown(
            &home,
            0,
            0,
        )?;
    }

    /*
     * ========================================================
     * /tmp
     * ========================================================
     */

    let tmp =
        mount_point.join("tmp");

    if tmp.exists() {
        set_mode(
            &tmp,
            0o1777,
        )?;
    }

    /*
     * ========================================================
     * Executable directories
     * ========================================================
     *
     * Normal executables are 0755.
     *
     * SUID permissions are intentionally applied later,
     * otherwise they would be overwritten here.
     */

    for dir in [
        "bin",
        "sbin",
        "usr/bin",
        "usr/sbin",
    ] {
        let path =
            mount_point.join(dir);

        if !path.is_dir() {
            continue;
        }

        for entry in fs::read_dir(&path)? {
            set_x_perm(
                &entry?.path()
            )?;
        }
    }

    /*
     * ========================================================
     * Basic configuration files
     * ========================================================
     */

    for file in [
        "etc/passwd",
        "etc/group",
        "etc/hostname",
        "etc/hosts",
        "etc/resolv.conf",
        "etc/fstab",
    ] {
        let path =
            mount_point.join(file);

        if path.exists() {
            set_mode(
                &path,
                0o644,
            )?;

            chown(
                &path,
                0,
                0,
            )?;
        }
    }

    /*
     * ========================================================
     * Sensitive authentication files
     * ========================================================
     */

    for file in [
        "etc/shadow",
        "etc/gshadow",
    ] {
        let path =
            mount_point.join(file);

        if path.exists() {
            set_mode(
                &path,
                0o600,
            )?;

            chown(
                &path,
                0,
                0,
            )?;
        }
    }

    Ok(())
}

/* ============================================================
   Privileged commands
============================================================ */

fn set_privileged_permissions(
    mount_point: &Path,
) -> Result<()> {
    /*
     * These programs need elevated privileges when launched
     * by an unprivileged user.
     *
     * 4755:
     *
     *   4 = SUID
     *   7 = owner rwx
     *   5 = group r-x
     *   5 = other r-x
     *
     * We deliberately do NOT make BusyBox itself SUID.
     */

    for command in [
        "bin/su",
        "usr/bin/su",
        "bin/ping",
        "usr/bin/ping",
        "bin/sudo",
    ] {
        let path =
            mount_point.join(command);

        /*
         * Only apply this to real files.
         *
         * This avoids accidentally making a symlink or some
         * unrelated path SUID.
         */
        if !path.is_file() {
            continue;
        }

        chown(
            &path,
            0,
            0,
        )?;

        set_mode(
            &path,
            0o4755,
        )?;
    }

    Ok(())
}

/* ============================================================
   Complete permission setup
============================================================ */

pub fn set_sys_perms(
    mount_point: &Path,
) -> Result<()> {
    /*
     * ========================================================
     * 1. Root ownership
     * ========================================================
     */

    set_proprietary_permissions(
        mount_point,
    )?;

    /*
     * ========================================================
     * 2. Normal user permissions
     * ========================================================
     */

    set_basic_user_permissions(
        mount_point,
    )?;

    /*
     * ========================================================
     * 3. Privileged commands
     * ========================================================
     *
     * Must be LAST because set_basic_user_permissions()
     * intentionally sets executables to 0755.
     */

    set_privileged_permissions(
        mount_point,
    )?;

    Ok(())
}