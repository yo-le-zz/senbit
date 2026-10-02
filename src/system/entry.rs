use anyhow::Result;

use colored::Colorize;

use crate::{log_info, log_error};

use inquire::Text;

use crate::system::handler::socket::setup_socket;

use crate::system::init::updates::{
    get_version,
    check_for_updates,
    update_kernel,
};

use crate::system::init::local::hostname::init_hostname;
use crate::system::init::init::{
    mount_system,
    fstab_is_ok,
    init_vars,
    switch_root,
};
use crate::system::init::services::start_services;
use crate::system::init::network::init_network;
use crate::system::login::login::login;
use crate::system::init::local::keymaps::init_keymaps;
use crate::system::log::init_logs;

const NEW_ROOT: &str = "/newroot";
const UPDATE_URL: &str = "https://github.com/yo-le-zz/senbit";

pub fn start_system(sys_disk: &str) -> Result<()> {
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

    if switch_root(NEW_ROOT).is_err() {
        log_error!("{}", "Failed to switch root.".red());
        return Err(anyhow::anyhow!("Failed to switch root"));
    }

    if init_logs().is_err() {
        log_error!("{}", "Failed to initialize logs.".red());
        return Err(anyhow::anyhow!("Failed to initialize logs"));
    }

    // new root is /
    const ROOT: &str = "/";

    log_info!("{}", "Starting services...".cyan());
    if start_services().is_err() {
        log_error!("{}", "Failed to start services.".red());
        return Err(anyhow::anyhow!("Failed to start services"));
    }

    log_info!("{}", "Initializing keymaps...".cyan());
    if init_keymaps(ROOT).is_err() {
        log_error!("{}", "Failed to initialize keymaps.".red());
        return Err(anyhow::anyhow!("Failed to initialize keymaps"));
    }

    log_info!("{}", "Initializing variables...".cyan());
    
    init_vars(ROOT)
        .map_err(|e| {
            log_error!(
                "{}",
                format!(
                    "Failed to initialize variables: {:#}",
                    e
                )
                .red()
            );
    
            e
        })?;
    
    let listener =
        match setup_socket() {
            Ok(listener) => listener,
    
            Err(e) => {
                log_error!(
                    "{}",
                    format!(
                        "Failed to setup socket: {:#}",
                        e
                    )
                    .red()
                );
    
                return Err(e);
            }
        };
    
    if let Err(e) =
        login(
            ROOT,
            &version,
            listener,
        )
    {
        log_error!(
            "{}",
            format!(
                "Failed to login: {:#}",
                e
            )
            .red()
        );
    
        return Err(
            anyhow::anyhow!(
                "Failed to login: {:#}",
                e
            )
        );
    }
    
    Ok(())
}
