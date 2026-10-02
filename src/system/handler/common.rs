use anyhow::{
    Context,
    Result,
};

use std::fs;
use std::path::Path;
use std::thread;
use std::time::Duration;

/* ============================================================
   Shutdown preparation
============================================================ */

pub fn prepare_shutdown() -> Result<()> {
    stop_services()?;

    stop_remaining_processes()?;

    cleanup_temporary_resources()?;

    invalidate_sensitive_caches()?;

    sync_filesystems()?;

    unmount_filesystems()?;

    Ok(())
}

/* ============================================================
   Services
============================================================ */

pub fn stop_services() -> Result<()> {
    /*
     * Senbit does not have a service manager yet.
     *
     * Keep this step in the shutdown pipeline so the service
     * manager can be integrated here later.
     */

    Ok(())
}

/* ============================================================
   Remaining processes
============================================================ */

pub fn stop_remaining_processes() -> Result<()> {
    let proc =
        Path::new("/proc");

    let entries =
        fs::read_dir(proc)
            .context(
                "failed to read /proc",
            )?;

    let mut pids =
        Vec::new();

    for entry in entries {
        let entry =
            entry.context(
                "failed to read /proc entry",
            )?;

        let name =
            entry.file_name();

        let name =
            match name.to_str() {
                Some(name) => name,
                None => continue,
            };

        let pid =
            match name.parse::<i32>() {
                Ok(pid) => pid,
                Err(_) => continue,
            };

        /*
         * PID 1 is Senbit itself.
         *
         * Never terminate PID 1 because it must perform
         * the final reboot/halt/poweroff syscall.
         */

        if pid == 1 {
            continue;
        }

        pids.push(pid);
    }

    /*
     * First ask every remaining process to terminate cleanly.
     */

    for pid in &pids {
        let result =
            unsafe {
                libc::kill(
                    *pid,
                    libc::SIGTERM,
                )
            };

        if result != 0 {
            let error =
                std::io::Error::last_os_error();

            /*
             * ESRCH means that the process already exited.
             */

            if error.raw_os_error()
                == Some(libc::ESRCH)
            {
                continue;
            }
        }
    }

    /*
     * Give processes a short time to exit cleanly.
     */

    thread::sleep(
        Duration::from_millis(500),
    );

    /*
     * Force-kill anything that is still alive.
     */

    for pid in pids {
        let result =
            unsafe {
                libc::kill(
                    pid,
                    0,
                )
            };

        if result == 0 {
            unsafe {
                libc::kill(
                    pid,
                    libc::SIGKILL,
                );
            }
        }
    }

    Ok(())
}

/* ============================================================
   Temporary resources
============================================================ */

pub fn cleanup_temporary_resources() -> Result<()> {
    /*
     * Remove the Senbit runtime directory if it exists.
     */

    let senbit_runtime =
        Path::new(
            "/run/senbit",
        );

    if fs::symlink_metadata(
        senbit_runtime,
    ).is_ok() {
        fs::remove_dir_all(
            senbit_runtime,
        )
        .context(
            "failed to clean Senbit runtime directory",
        )?;
    }

    /*
     * Remove the system event socket.
     *
     * The listener itself remains owned by PID 1, but removing
     * the filesystem entry prevents the socket from surviving
     * as a runtime resource.
     */

    let socket =
        Path::new(
            "/run/senbit.sock",
        );

    if fs::symlink_metadata(
        socket,
    ).is_ok() {
        fs::remove_file(socket)
            .context(
                "failed to remove Senbit event socket",
            )?;
    }

    /*
     * Clear /tmp.
     *
     * Remove everything inside /tmp, but keep the /tmp
     * directory itself.
     *
     * symlink_metadata() is used so a symbolic link inside
     * /tmp is removed instead of being followed.
     */

    let tmp =
        Path::new("/tmp");

    if tmp.is_dir() {
        let entries =
            fs::read_dir(tmp)
                .context(
                    "failed to read /tmp",
                )?;

        for entry in entries {
            let entry =
                entry.context(
                    "failed to read /tmp entry",
                )?;

            let path =
                entry.path();

            let metadata =
                fs::symlink_metadata(&path)
                    .with_context(|| {
                        format!(
                            "failed to inspect {}",
                            path.display()
                        )
                    })?;

            if metadata.file_type().is_dir() {
                fs::remove_dir_all(&path)
                    .with_context(|| {
                        format!(
                            "failed to remove temporary directory {}",
                            path.display()
                        )
                    })?;
            } else {
                fs::remove_file(&path)
                    .with_context(|| {
                        format!(
                            "failed to remove temporary file {}",
                            path.display()
                        )
                    })?;
            }
        }
    }

    Ok(())
}

/* ============================================================
   Sensitive caches
============================================================ */

pub fn invalidate_sensitive_caches() -> Result<()> {
    /*
     * Senbit does not currently have a dedicated
     * sensitive-cache subsystem.
     *
     * Keep this stage in the shutdown pipeline so it can be
     * implemented when such caches are introduced.
     */

    Ok(())
}

/* ============================================================
   Filesystem synchronization
============================================================ */

pub fn sync_filesystems() -> Result<()> {
    /*
     * Flush all filesystem buffers before unmounting.
     */

    unsafe {
        libc::sync();
    }

    Ok(())
}

/* ============================================================
   Filesystem unmount
============================================================ */

fn unmount(path: &str) -> Result<()> {
    let target =
        std::ffi::CString::new(path)
            .context(
                "invalid mount path",
            )?;

    /*
     * MNT_DETACH performs a lazy unmount.
     *
     * This is important for PID 1 because the event listener
     * can still hold the Unix socket open while the shutdown
     * handler is executing.
     *
     * The mount is detached immediately and fully released
     * once the remaining references disappear.
     */

    let result =
        unsafe {
            libc::umount2(
                target.as_ptr(),
                libc::MNT_DETACH,
            )
        };

    if result != 0 {
        let error =
            std::io::Error::last_os_error();

        /*
         * If the mount is already gone, there is nothing left
         * to do.
         */

        if error.raw_os_error()
            == Some(libc::EINVAL)
            || error.raw_os_error()
                == Some(libc::ENOENT)
        {
            return Ok(());
        }

        return Err(
            anyhow::anyhow!(
                "failed to unmount {}: {}",
                path,
                error
            )
        );
    }

    Ok(())
}

pub fn unmount_filesystems() -> Result<()> {
    /*
     * switch_root() moved the complete installed system from
     * /newroot to /.
     *
     * Therefore / itself must NOT be unmounted.
     *
     * Only the auxiliary filesystems created by mount_system()
     * are detached.
     */

    /*
     * /run
     *
     * Unmount first because it contains Senbit's runtime
     * resources and event socket.
     */

    unmount("/run")
        .context(
            "failed to unmount /run",
        )?;

    /*
     * /dev
     */

    unmount("/dev")
        .context(
            "failed to unmount /dev",
        )?;

    /*
     * /sys
     */

    unmount("/sys")
        .context(
            "failed to unmount /sys",
        )?;

    /*
     * /proc
     */

    unmount("/proc")
        .context(
            "failed to unmount /proc",
        )?;

    /*
     * Never unmount /.
     *
     * PID 1 is currently executing from the installed root
     * filesystem and needs it for the final kernel syscall.
     */

    Ok(())
}