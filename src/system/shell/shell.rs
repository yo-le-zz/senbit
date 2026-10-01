use anyhow::{Context, Result};
use std::io;
use std::os::unix::process::CommandExt;
use std::process::Command;

/// Real console (VGA/framebuffer or serial port, depending on the kernel's `console=`).
const CONSOLE: &[u8] = b"/dev/console\0";

/// Starts the user's shell in its own session, with the console as its
/// controlling terminal.
///
/// Why: `/dev/tty` means "the controlling terminal of THIS process". Senbit
/// runs as PID 1 without a controlling terminal, so opening it fails with
/// ENXIO (os error 6). Without a controlling terminal, ash also cannot do
/// job control ("can't access tty; job control turned off", Ctrl+C does nothing).
///
/// Fix: in the child, create a new session (setsid), open the real console,
/// make it the controlling terminal (TIOCSCTTY), attach stdin/stdout/stderr,
/// then drop privileges to the logged-in user's UID/GID.
pub fn shell_start(
    uid: u32,
    gid: u32,
) -> Result<()> {
    let shell =
        std::env::var("SHELL")
            .unwrap_or_else(
                |_| "/bin/sh".to_string()
            );

    let mut cmd =
        Command::new(&shell);

    // SAFETY: the closure only uses async-signal-safe calls
    // (setsid, open, ioctl, dup2, close, setgid, setuid)
    // and performs no allocation.
    unsafe {
        cmd.pre_exec(move || {
            // Create a new session while still running as root.
            if libc::setsid() < 0 {
                return Err(
                    io::Error::last_os_error()
                );
            }

            // Open the real system console.
            let fd =
                libc::open(
                    CONSOLE.as_ptr()
                        as *const libc::c_char,
                    libc::O_RDWR,
                );

            if fd < 0 {
                return Ok(());
            }

            // Make /dev/console the controlling terminal.
            if libc::ioctl(
                fd,
                libc::TIOCSCTTY as _,
                0,
            ) < 0 {
                return Err(
                    io::Error::last_os_error()
                );
            }

            // Attach stdin/stdout/stderr to the console.
            for target in 0..3 {
                if libc::dup2(
                    fd,
                    target,
                ) < 0 {
                    return Err(
                        io::Error::last_os_error()
                    );
                }
            }

            if fd > 2 {
                libc::close(fd);
            }

            // Drop privileges only after the TTY is fully configured.
            if libc::setgid(gid) < 0 {
                return Err(
                    io::Error::last_os_error()
                );
            }

            if libc::setuid(uid) < 0 {
                return Err(
                    io::Error::last_os_error()
                );
            }

            Ok(())
        });
    }

    cmd.status()
        .context("Failed to start shell")?;

    Ok(())
}