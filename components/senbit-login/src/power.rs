//! Power transitions (reboot / shutdown / halt / poweroff).
//!
//! systemd is PID 1 and owns the shutdown sequence: asking it for a
//! transition makes it stop EVERY unit in reverse dependency order (Senbit's
//! own, and any service installed later by a package such as apt), honoring
//! each unit's `TimeoutStopSec`, then unmount the filesystems and call the
//! final reboot syscall. Nothing here may hardcode service names, so that
//! packaged services are handled with no change.

use std::fs;
use std::path::Path;
use std::process::Command;

use anyhow::{bail, Context, Result};

#[derive(Debug, Clone, Copy)]
pub enum PowerAction {
    Shutdown,
    Reboot,
    Halt,
    Poweroff,
}

impl PowerAction {
    /// The systemctl verb. `shutdown` means power off.
    fn verb(self) -> &'static str {
        match self {
            PowerAction::Reboot => "reboot",
            PowerAction::Halt => "halt",
            PowerAction::Shutdown | PowerAction::Poweroff => "poweroff",
        }
    }

    fn syscall(self) -> libc::c_int {
        match self {
            PowerAction::Reboot => libc::LINUX_REBOOT_CMD_RESTART,
            PowerAction::Halt => libc::LINUX_REBOOT_CMD_HALT,
            PowerAction::Shutdown | PowerAction::Poweroff => libc::LINUX_REBOOT_CMD_POWER_OFF,
        }
    }
}

/// Same test as sd_booted(): true when systemd is the running init system.
fn systemd_is_running() -> bool {
    Path::new("/run/systemd/system").is_dir()
}

/// Removes the runtime resources owned by Senbit (they live in /run, which
/// is a tmpfs, but a clean shutdown must not depend on that).
fn cleanup_runtime() {
    let _ = fs::remove_file(crate::events::SOCKET_PATH);

    let dir = Path::new("/run/senbit");
    if fs::symlink_metadata(dir).is_ok() {
        if let Err(error) = fs::remove_dir_all(dir) {
            log_warn!("cannot remove {}: {error}", dir.display());
        }
    }
}

/// Stops all services by handing the transition to systemd.
///
/// `--no-block`: the request is queued and we return immediately, so the
/// client that asked for the transition gets its answer before this very
/// service (which systemd is about to stop) disappears.
fn stop_services(action: PowerAction) -> Result<()> {
    let status = Command::new("systemctl")
        .env("PATH", "/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin")
        .args(["--no-block", action.verb()])
        .status()
        .context("failed to run systemctl")?;

    if !status.success() {
        bail!("systemctl {} failed: {status}", action.verb());
    }

    Ok(())
}

/// Last resort when systemd is not PID 1 (never the case in a normal Senbit
/// boot): flush and ask the kernel directly.
fn direct_transition(action: PowerAction) -> Result<()> {
    log_warn!("systemd is not running: using the reboot syscall directly");

    unsafe {
        libc::sync();
        if libc::reboot(action.syscall()) != 0 {
            bail!("reboot syscall failed: {}", std::io::Error::last_os_error());
        }
    }

    Ok(())
}

pub fn request(action: PowerAction) -> Result<()> {
    log_info!("power request: {}", action.verb());

    cleanup_runtime();
    unsafe {
        libc::sync();
    }

    if systemd_is_running() {
        stop_services(action)
    } else {
        direct_transition(action)
    }
}
