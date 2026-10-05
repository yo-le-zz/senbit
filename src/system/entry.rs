use anyhow::Result;

use colored::Colorize;

use std::convert::Infallible;

use crate::{log_info, log_error};

use inquire::Text;

use crate::system::init::updates::{
    get_version,
    check_for_updates,
    update_kernel,
};

use crate::system::init::local::hostname::init_hostname;
use crate::system::init::init::{
    mount_system,
    fstab_is_ok,
    switch_root,
    exec_systemd,
};
use crate::system::init::network::init_network;
use crate::system::init::local::keymaps::init_keymaps;
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
/// This function never returns Ok: it either execs systemd or fails.
pub fn start_system(sys_disk: &str) -> Result<Infallible> {
    log_info!("{}", "Mounting system...".cyan());
    mount_system(sys_disk, NEW_ROOT)?;

    log_info!("{}", "Checking fstab...".cyan());
    if !fstab_is_ok(NEW_ROOT) {
        return Err(anyhow::anyhow!("fstab not found"));
    }

    log_info!("{}", "Initializing hostname...".cyan());
    if init_hostname(NEW_ROOT).is_err() {
        log_error!("{}", "Failed to initialize hostname.".red());
        return Err(anyhow::anyhow!("Failed to initialize hostname"));
    }

    let version = get_version(NEW_ROOT)?;

    log_info!("{}", "Checking updates...".cyan());
    if check_for_updates(&version, UPDATE_URL) {
        log_info!("{}", "Updates found.".cyan());

        let update = Text::new("Update?")
            .with_default("yes")
            .prompt()?;

        if update.trim().eq_ignore_ascii_case("yes") {
            log_info!("{}", "Updating...".cyan());
            update_kernel(UPDATE_URL)?;
        }
    }

    log_info!("{}", "Initializing network...".cyan());

    if let Err(e) = init_network(NEW_ROOT) {
        log_error!("{}", "Failed to initialize network.".red());
        return Err(anyhow::anyhow!(
            "Failed to initialize network: {}",
            e
        ));
    }

    if let Err(e) = switch_root(NEW_ROOT) {
        log_error!("{}", "Failed to switch root.".red());
        return Err(e.context("Failed to switch root"));
    }

    if init_logs().is_err() {
        log_error!("{}", "Failed to initialize logs.".red());
        return Err(anyhow::anyhow!("Failed to initialize logs"));
    }

    // The keyboard layout is console state kept by the kernel: it survives
    // the exec. A bad layout must not prevent the system from booting.
    log_info!("{}", "Initializing keymaps...".cyan());
    if let Err(e) = init_keymaps("/") {
        log_error!("{}", format!("Keymap not loaded: {:#}", e).red());
    }

    log_info!("{}", "Starting systemd (PID 1)...".cyan());

    Err(exec_systemd())
}
