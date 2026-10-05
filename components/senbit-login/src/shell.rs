//! Starts the user's shell on the console.

use std::io;
use std::os::unix::process::CommandExt;
use std::process::Command;

use anyhow::{Context, Result};

use crate::accounts::Account;

/// Real console (VGA/framebuffer or serial port, depending on `console=`).
const CONSOLE: &[u8] = b"/dev/console\0";

/// Runs the account's shell and waits for it to exit.
///
/// The shell gets its own session with the console as controlling terminal
/// (setsid + TIOCSCTTY), otherwise ash prints "can't access tty; job control
/// turned off" and Ctrl+C does nothing. Privileges are dropped only after the
/// terminal is fully configured.
pub fn run(account: &Account, environment: &[(String, String)]) -> Result<()> {
    let shell = environment
        .iter()
        .find(|(key, _)| key == "SHELL")
        .map(|(_, value)| value.as_str())
        .unwrap_or("/bin/sh");

    let (uid, gid) = (account.uid, account.gid);

    let mut command = Command::new(shell);
    command.env_clear();
    command.envs(environment.iter().map(|(k, v)| (k.as_str(), v.as_str())));

    // SAFETY: the closure only uses async-signal-safe calls (setsid, open,
    // ioctl, dup2, close, setgroups, setgid, setuid) and does not allocate.
    unsafe {
        command.pre_exec(move || {
            if libc::setsid() < 0 {
                return Err(io::Error::last_os_error());
            }

            let fd = libc::open(CONSOLE.as_ptr() as *const libc::c_char, libc::O_RDWR);
            if fd < 0 {
                return Err(io::Error::last_os_error());
            }

            // The login service owns the console as its own controlling
            // terminal: take it over (arg 1, root only).
            if libc::ioctl(fd, libc::TIOCSCTTY as _, 1) < 0 {
                return Err(io::Error::last_os_error());
            }

            for target in 0..3 {
                if libc::dup2(fd, target) < 0 {
                    return Err(io::Error::last_os_error());
                }
            }

            if fd > 2 {
                libc::close(fd);
            }

            // Do not leak root's supplementary groups into the session.
            if libc::setgroups(0, std::ptr::null()) < 0 {
                return Err(io::Error::last_os_error());
            }

            if libc::setgid(gid) < 0 {
                return Err(io::Error::last_os_error());
            }

            if libc::setuid(uid) < 0 {
                return Err(io::Error::last_os_error());
            }

            Ok(())
        });
    }

    let status = command
        .status()
        .with_context(|| format!("failed to start shell '{shell}'"))?;

    log_info!("session of '{}' ended ({status})", account.name);

    Ok(())
}
