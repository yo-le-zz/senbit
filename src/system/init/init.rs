use anyhow::{anyhow, bail, Context, Result};

use std::ffi::CString;
use std::fs;
use std::os::unix::fs::{chroot, symlink, MetadataExt, PermissionsExt};
use std::os::unix::process::CommandExt;
use std::path::Path;
use std::process::Command;

use crate::filesystem::fs::mount;

use crate::log_info;

pub fn mount_system(
    sys_disk: &str,
    root: &str,
) -> Result<()> {
    let device =
        if sys_disk.starts_with("/dev/") {
            sys_disk.to_string()
        } else {
            format!("/dev/{}", sys_disk)
        };

    mount(
        &device,
        root,
        Some("ext4"),
    )?;

    mount(
        "proc",
        &format!("{}/proc", root),
        Some("proc"),
    )?;

    mount(
        "sysfs",
        &format!("{}/sys", root),
        Some("sysfs"),
    )?;

    mount(
        "devtmpfs",
        &format!("{}/dev", root),
        Some("devtmpfs"),
    )?;

    mount(
        "tmpfs",
        &format!("{}/run", root),
        Some("tmpfs"),
    )?;

    let var_dir =
        format!("{}/var", root);

    let var_run =
        format!("{}/var/run", root);

    std::fs::create_dir_all(
        &var_dir
    )?;

    if !std::path::Path::new(
        &var_run
    ).exists() {
        symlink(
            "/run",
            &var_run
        )?;
    }

    Ok(())
}

pub fn fstab_is_ok(root: &str) -> bool {
    std::path::Path::new(root)
        .join("etc/fstab")
        .exists()
}

/* ============================================================
   Leaving the initramfs
============================================================ */

/// statfs magic numbers of the filesystems the kernel can use as rootfs.
const RAMFS_MAGIC: u64 = 0x8584_58f6;
const TMPFS_MAGIC: u64 = 0x0102_1994;

fn root_is_ramfs() -> bool {
    let path = CString::new("/").expect("static path");
    let mut buf = unsafe { std::mem::zeroed::<libc::statfs>() };

    if unsafe { libc::statfs(path.as_ptr(), &mut buf) } != 0 {
        return false;
    }

    let kind = buf.f_type as u64;
    kind == RAMFS_MAGIC || kind == TMPFS_MAGIC
}

/// Deletes everything that lives on `dev` below `dir`. Entries on another
/// device (/proc, /sys, /dev, /run, the new root...) are mount points and are
/// left alone.
fn remove_tree_on_device(dir: &Path, dev: u64) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };

    for entry in entries.flatten() {
        let path = entry.path();

        let Ok(meta) = fs::symlink_metadata(&path) else {
            continue;
        };

        if meta.dev() != dev {
            continue;
        }

        if meta.is_dir() {
            remove_tree_on_device(&path, dev);
            let _ = fs::remove_dir(&path);
        } else {
            let _ = fs::remove_file(&path);
        }
    }
}

/// Frees the memory held by the initramfs once the real root is ready, like
/// switch_root(8) does. Only done if "/" really is a rootfs (ramfs/tmpfs):
/// never touches a normal filesystem.
fn release_initramfs() {
    if !root_is_ramfs() {
        log_info!("Root is not a ramfs/tmpfs: initramfs not released.");
        return;
    }

    let Ok(meta) = fs::symlink_metadata("/") else {
        return;
    };

    remove_tree_on_device(Path::new("/"), meta.dev());

    log_info!("Initramfs released.");
}

/// Makes `new_root` the root of the system (mount --move + chroot).
pub fn switch_root(new_root: &str) -> Result<()> {
    log_info!("Switching root to {}...", new_root);

    std::env::set_current_dir(new_root)
        .with_context(|| format!("failed to chdir to {}", new_root))?;

    // The new root is the current directory, which is on another device, so
    // it is skipped by the cleanup.
    release_initramfs();

    let dot = CString::new(".")?;
    let slash = CString::new("/")?;

    let ret = unsafe {
        libc::mount(
            dot.as_ptr(),
            slash.as_ptr(),
            std::ptr::null(),
            libc::MS_MOVE,
            std::ptr::null(),
        )
    };

    if ret != 0 {
        bail!(
            "failed to move {} to /: {}",
            new_root,
            std::io::Error::last_os_error()
        );
    }

    chroot(".").context("failed to chroot")?;
    std::env::set_current_dir("/").context("failed to chdir to /")?;

    Ok(())
}

/* ============================================================
   Handing over to systemd
============================================================ */

const SYSTEMD: &str = "/usr/lib/systemd/systemd";

/// Replaces this process with systemd. senbit-init is PID 1: after exec(),
/// the same PID 1 runs systemd.
///
/// Only returns if the exec failed, and then always with an error.
pub fn exec_systemd() -> anyhow::Error {
    match fs::metadata(SYSTEMD) {
        Ok(meta) if meta.is_file() && meta.permissions().mode() & 0o111 != 0 => {}
        Ok(_) => return anyhow!("{SYSTEMD} is not an executable file"),
        Err(e) => return anyhow!("{SYSTEMD} not found in the installed system: {e}"),
    }

    let debug = crate::system::log::debug::enabled();

    let mut command = Command::new(SYSTEMD);
    command.arg("--system");

    if debug {
        dump_mount_state();
        // Everything through the kernel log: the debug forwarder copies it to
        // the serial port (build/vm/qemu.log), the screen stays clean.
        command.args(["--log-level=info", "--log-target=kmsg"]);
        let _ = fs::create_dir_all("/run/systemd/journald.conf.d");
        let _ = fs::write(
            "/run/systemd/journald.conf.d/10-senbit-debug.conf",
            "[Journal]\nForwardToKMsg=yes\nMaxLevelKMsg=info\n",
        );
    }

    // systemd gets a clean environment: as PID 1 it ignores it anyway (it
    // builds its own), and services get theirs from their units.
    let error = command
        .env_clear()
        .env(
            "PATH",
            "/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin",
        )
        .exec();

    anyhow!(
        "exec {SYSTEMD} failed: {error} \
         (its ELF loader or a shared library is probably missing from the installed system)"
    )
}

/// Debug helper (`senbit.debug` on the kernel command line): prints what
/// systemd's mount_setup() will see.
fn dump_mount_state() {
    if let Ok(m) = fs::read_to_string("/proc/self/mountinfo") {
        for line in m.lines() {
            log_info!("mountinfo: {}", line);
        }
    }

    for p in ["/", "/proc", "/sys", "/sys/fs/cgroup", "/dev", "/dev/pts", "/dev/shm", "/run"] {
        match fs::symlink_metadata(p) {
            Ok(m) => log_info!("stat {}: mode={:o} dev={}", p, m.mode(), m.dev()),
            Err(e) => log_info!("stat {}: {}", p, e),
        }
    }
}
