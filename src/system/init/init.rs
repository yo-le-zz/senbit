use anyhow::{Context, Result, anyhow, bail};

use std::ffi::CString;
use std::fs;
use std::os::unix::fs::{MetadataExt, PermissionsExt, chroot, symlink};
use std::os::unix::process::CommandExt;
use std::path::Path;
use std::process::Command;

use libc::{MS_NODEV, MS_NOEXEC, MS_NOSUID, MS_STRICTATIME};

use crate::filesystem::fs::{is_mount_point, lazy_unmount, sys_mount};
use crate::{log_error, log_info, log_warn};

/* ============================================================
   Mounting the installed system
============================================================ */

/// GID of the "tty" group: systemd mounts /dev/pts with `gid=5`, so the
/// installed /etc/group must define it (see installation/users).
const TTY_GID: u32 = 5;

/// One kernel API filesystem, mounted with exactly the flags systemd's own
/// mount_setup() would use (so that systemd finds it ready and skips it).
struct ApiMount {
    source: &'static str,
    /// Path relative to the new root.
    target: &'static str,
    fs_type: &'static str,
    flags: libc::c_ulong,
    data: String,
    /// Mode of the mount point directory if it has to be created.
    dir_mode: u32,
    /// A failure stops the boot (systemd could not run without it).
    fatal: bool,
}

fn api_mounts() -> Vec<ApiMount> {
    vec![
        ApiMount {
            source: "proc",
            target: "proc",
            fs_type: "proc",
            flags: MS_NOSUID | MS_NODEV | MS_NOEXEC,
            data: String::new(),
            dir_mode: 0o755,
            fatal: true,
        },
        ApiMount {
            source: "sysfs",
            target: "sys",
            fs_type: "sysfs",
            flags: MS_NOSUID | MS_NODEV | MS_NOEXEC,
            data: String::new(),
            dir_mode: 0o755,
            fatal: true,
        },
        ApiMount {
            source: "devtmpfs",
            target: "dev",
            fs_type: "devtmpfs",
            flags: MS_NOSUID | MS_STRICTATIME,
            data: "mode=0755,size=4m".into(),
            dir_mode: 0o755,
            fatal: true,
        },
        ApiMount {
            source: "tmpfs",
            target: "run",
            fs_type: "tmpfs",
            flags: MS_NOSUID | MS_NODEV | MS_STRICTATIME,
            data: "mode=0755".into(),
            dir_mode: 0o755,
            fatal: true,
        },
    ]
}

/// Filesystems systemd treats as mandatory on top of the four above
/// (src/core/mount-setup.c): without them it stops with "Failed to mount API
/// filesystems". They are mounted here, once /dev and /sys are in place, so
/// that a failure is reported with its real cause (errno) instead of being
/// hidden by the kernel loglevel.
fn systemd_required_mounts(root: &str) -> Vec<(String, ApiMount)> {
    vec![
        (
            format!("{root}/dev/shm"),
            ApiMount {
                source: "tmpfs",
                target: "dev/shm",
                fs_type: "tmpfs",
                flags: MS_NOSUID | MS_NODEV | MS_STRICTATIME,
                data: "mode=01777".into(),
                dir_mode: 0o1777,
                fatal: true,
            },
        ),
        (
            format!("{root}/dev/pts"),
            ApiMount {
                source: "devpts",
                target: "dev/pts",
                fs_type: "devpts",
                flags: MS_NOSUID | MS_NOEXEC,
                data: format!("mode=0620,gid={TTY_GID}"),
                dir_mode: 0o755,
                fatal: false,
            },
        ),
    ]
}

/// cgroup v2 mount options, from the most to the least featureful: older
/// kernels reject the newer options, systemd tries the same fallbacks.
const CGROUP2_OPTIONS: &[&str] = &["nsdelegate,memory_recursiveprot", "nsdelegate", ""];

fn mount_one(m: &ApiMount, full_target: &str) -> Result<()> {
    if let Err(e) = fs::create_dir_all(full_target) {
        bail!("cannot create {full_target}: {e}");
    }
    let _ = fs::set_permissions(full_target, fs::Permissions::from_mode(m.dir_mode));

    if is_mount_point(full_target) {
        return Ok(());
    }

    sys_mount(m.source, full_target, m.fs_type, m.flags, &m.data)
        .with_context(|| format!("mount {} on {}", m.fs_type, full_target))
}

fn mount_cgroup2(root: &str) -> Result<()> {
    let target = format!("{root}/sys/fs/cgroup");

    // sysfs only has this directory when the kernel has cgroups at all.
    if !Path::new(&target).is_dir() {
        bail!(
            "{target} does not exist: the kernel was built without cgroup support \
             (CONFIG_CGROUPS); systemd cannot run without it"
        );
    }
    if is_mount_point(&target) {
        return Ok(());
    }

    let flags = MS_NOSUID | MS_NODEV | MS_NOEXEC;
    let mut last_error = None;

    for options in CGROUP2_OPTIONS {
        match sys_mount("cgroup2", &target, "cgroup2", flags, options) {
            Ok(()) => return Ok(()),
            Err(e) => last_error = Some(e),
        }
    }

    Err(anyhow!(
        "mount cgroup2 on {target}: {} (kernel without CONFIG_CGROUPS / cgroup v2?)",
        last_error.map(|e| e.to_string()).unwrap_or_default()
    ))
}

/// Mounts the installed root on `root`, then the API filesystems systemd
/// needs. Every failure carries its errno: this is the first thing to read
/// when systemd answers "Failed to mount API filesystems".
pub fn mount_system(sys_disk: &str, root: &str) -> Result<()> {
    let device = if sys_disk.starts_with("/dev/") {
        sys_disk.to_string()
    } else {
        format!("/dev/{sys_disk}")
    };

    fs::create_dir_all(root).with_context(|| format!("cannot create {root}"))?;

    sys_mount(&device, root, "ext4", 0, "")
        .with_context(|| format!("mount {device} on {root} (ext4)"))?;

    for m in api_mounts() {
        let target = format!("{root}/{}", m.target);
        if let Err(e) = mount_one(&m, &target) {
            if m.fatal {
                return Err(e);
            }
            log_warn!("{:#}", e);
        }
    }

    for (target, m) in systemd_required_mounts(root) {
        if let Err(e) = mount_one(&m, &target) {
            if m.fatal {
                return Err(e);
            }
            log_warn!("{:#}", e);
        }
    }

    if let Err(e) = mount_cgroup2(root) {
        // Not fatal HERE: systemd will try again and print its own error.
        // But this message says why, in clear, whatever the kernel loglevel.
        log_error!("{:#}", e);
    }

    let var_dir = format!("{root}/var");
    let var_run = format!("{root}/var/run");

    fs::create_dir_all(&var_dir)?;

    if fs::symlink_metadata(&var_run).is_err() {
        symlink("/run", &var_run)?;
    }

    Ok(())
}

pub fn fstab_is_ok(root: &str) -> bool {
    std::path::Path::new(root).join("etc/fstab").exists()
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

/// Lazily unmounts the API filesystems mounted in the initramfs.
fn detach_initramfs_mounts() {
    for target in ["/run", "/dev", "/sys", "/proc"] {
        if !is_mount_point(target) {
            continue;
        }
        if let Err(e) = lazy_unmount(target) {
            log_warn!("Could not detach the initramfs {}: {}", target, e);
        }
    }
}

/// Makes `new_root` the root of the system (mount --move + chroot).
pub fn switch_root(new_root: &str) -> Result<()> {
    log_info!("Switching root to {}...", new_root);

    std::env::set_current_dir(new_root)
        .with_context(|| format!("failed to chdir to {}", new_root))?;

    // The initramfs' own /proc, /sys, /dev and /run are no longer needed (the
    // new root has its own, mounted by mount_system). Detach them so that
    // they do not stay stacked under the new root for the whole life of the
    // system, where they would show up twice in /proc/self/mountinfo. Open
    // file descriptors (stdio, log files) keep working.
    detach_initramfs_mounts();

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
    } else {
        // The GRUB entries use `loglevel=3`, which hides every kernel-log
        // line of level "error". systemd's own failures ("Failed to mount
        // ...") are errors, so they used to vanish and PID 1 only said
        // "Failed to mount API filesystems" with no cause. Send systemd's
        // errors straight to the console instead (bypasses the printk
        // loglevel), unless the user chose a level on the command line.
        let cmdline = fs::read_to_string("/proc/cmdline").unwrap_or_default();
        if !cmdline
            .split_whitespace()
            .any(|a| a.starts_with("systemd.log_target="))
        {
            command.arg("--log-target=console");
        }
        if !cmdline
            .split_whitespace()
            .any(|a| a.starts_with("systemd.log_level="))
        {
            command.arg("--log-level=err");
        }
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

    for p in [
        "/",
        "/proc",
        "/sys",
        "/sys/fs/cgroup",
        "/dev",
        "/dev/pts",
        "/dev/shm",
        "/run",
    ] {
        match fs::symlink_metadata(p) {
            Ok(m) => log_info!("stat {}: mode={:o} dev={}", p, m.mode(), m.dev()),
            Err(e) => log_info!("stat {}: {}", p, e),
        }
    }
}
