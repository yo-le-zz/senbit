use anyhow::Result;

use colored::Colorize;

use std::convert::Infallible;

use crate::system::report;
use crate::{log_error, log_info, log_warn};

use inquire::Text;

use crate::system::init::updates::{check_for_updates, get_version, update_kernel};

use crate::system::init::init::{exec_systemd, fstab_is_ok, mount_system, switch_root};
use crate::system::init::local::hostname::init_hostname;
use crate::system::init::local::keymaps::init_keymaps;
use crate::system::init::network::init_network;
use crate::system::log::init_logs;

const NEW_ROOT: &str = "/newroot";
const UPDATE_URL: &str = "https://github.com/yo-le-zz/senbit";

/// Boots an installed system.
///
/// senbit-init is only the transition init (it runs from the initramfs):
/// it mounts the installed root, prepares what has to be done before
/// userspace starts, switches to the new root, then REPLACES itself with
/// systemd (exec). From then on PID 1 is systemd; the login, the system
/// event socket and every other service are started by systemd units
/// (see rootfs/usr/lib/systemd/system/senbit-login.service).
///
/// Only what is needed to reach systemd is fatal (mounting the root, switching
/// to it, the exec itself). Everything else (hostname, network, logs, keyboard)
/// is recorded in the boot report (system::report) and the system starts anyway:
/// a missing network must never prevent a login. The report is shown at login
/// (/etc/profile.d/senbit-bootreport.sh) and with the `bootreport` command.
///
/// This function never returns Ok: it either execs systemd or fails.
pub fn start_system(sys_disk: &str) -> Result<Infallible> {
    log_info!("{}", "Mounting system...".cyan());
    mount_system(sys_disk, NEW_ROOT)?;
    report::ok("Root filesystem", format!("{sys_disk} mounted"));

    log_info!("{}", "Checking fstab...".cyan());
    if fstab_is_ok(NEW_ROOT) {
        report::ok("fstab", "found");
    } else {
        log_warn!("{}", "fstab not found.".yellow());
        report::warn("fstab", "/etc/fstab not found: only the root filesystem is mounted");
    }

    log_info!("{}", "Initializing hostname...".cyan());
    match init_hostname(NEW_ROOT) {
        Ok(_) => report::ok("Hostname", "set"),
        Err(e) => {
            log_warn!("{}", "Failed to initialize hostname.".yellow());
            report::warn("Hostname", format!("not set: {e:#}"));
        }
    }

    match get_version(NEW_ROOT) {
        Ok(version) => {
            log_info!("{}", "Checking updates...".cyan());
            if check_for_updates(&version, UPDATE_URL) {
                log_info!("{}", "Updates found.".cyan());

                let update = Text::new("Update?").with_default("yes").prompt()?;

                if update.trim().eq_ignore_ascii_case("yes") {
                    log_info!("{}", "Updating...".cyan());
                    update_kernel(UPDATE_URL)?;
                }
            }
            report::ok("Version", format!("Senbit {version}"));
        }
        Err(e) => {
            log_warn!("{}", "Could not read the Senbit version.".yellow());
            report::warn("Version", format!("{e:#}"));
        }
    }

    log_info!("{}", "Initializing network...".cyan());
    match init_network(NEW_ROOT) {
        Ok(()) => report::ok("Network", "configured and reachable"),
        Err(e) => {
            // Not fatal: boot goes on, the login banner reminds the user.
            log_warn!("{}", format!("Network not available: {e:#}").yellow());
            report::warn(
                "Network",
                format!("{e:#} - check the cable / DHCP server (ip addr), then: systemctl restart senbit-dhcp@<interface>"),
            );
        }
    }

    if let Err(e) = switch_root(NEW_ROOT) {
        log_error!("{}", "Failed to switch root.".red());
        return Err(e.context("Failed to switch root"));
    }
    report::ok("Switch root", "now running from the installed system");

    match init_logs() {
        Ok(_) => report::ok("Logs", "initialized"),
        Err(e) => {
            log_warn!("{}", "Failed to initialize logs.".yellow());
            report::warn("Logs", format!("{e:#}"));
        }
    }

    // The keyboard layout is console state kept by the kernel: it survives
    // the exec. A bad layout must not prevent the system from booting.
    log_info!("{}", "Initializing keymaps...".cyan());
    match init_keymaps("/") {
        Ok(_) => report::ok("Keyboard", "layout loaded"),
        Err(e) => {
            log_warn!("{}", format!("Keymap not loaded: {:#}", e).yellow());
            report::warn("Keyboard", format!("layout not loaded: {e:#}"));
        }
    }

    log_info!("{}", "Starting systemd (PID 1)...".cyan());
    report::ok("systemd", "starting as PID 1");
    report::flush();

    Err(exec_systemd())
}
