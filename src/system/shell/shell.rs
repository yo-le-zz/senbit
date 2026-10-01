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
/// ENXIO (os error 6). Without a controlling terminal, ash also cannot do job
/// control ("can't access tty; job control turned off", Ctrl+C does nothing).
///
/// Fix: in the child, right before exec, create a new session (setsid), then
/// open the console and make it the controlling terminal (TIOCSCTTY). The
/// parent (init) is left untouched.
pub fn shell_start() -> Result<()> {
    let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string());

    let mut cmd = Command::new(&shell);

    // SAFETY: the closure only uses async-signal-safe calls
    // (setsid, open, ioctl, dup2, close) and performs no allocation.
    unsafe {
        cmd.pre_exec(|| {
            if libc::setsid() < 0 {
                return Err(io::Error::last_os_error());
            }

            let fd = libc::open(CONSOLE.as_ptr() as *const libc::c_char, libc::O_RDWR);
            if fd < 0 {
                // No /dev/console: keep the inherited stdin/stdout/stderr.
                return Ok(());
            }

            // Failure is tolerated: the shell still starts, without job control.
            libc::ioctl(fd, libc::TIOCSCTTY as _, 0);

            for target in 0..3 {
                if libc::dup2(fd, target) < 0 {
                    return Err(io::Error::last_os_error());
                }
            }
            if fd > 2 {
                libc::close(fd);
            }
            Ok(())
        });
    }

    cmd.status().context("Failed to start shell")?;

    Ok(())
}