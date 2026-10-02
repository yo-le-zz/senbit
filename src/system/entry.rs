use anyhow::Result;

use colored::Colorize;

use crate::logln;

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

const NEW_ROOT: &str = "/newroot";
const UPDATE_URL: &str = "https://github.com/yo-le-zz/senbit";

pub fn start_system(sys_disk: &str) -> Result<()> {
    logln!("{}", "Mounting system...".cyan());
    mount_system(sys_disk, NEW_ROOT)?;

    logln!("{}", "Checking fstab...".cyan());
    if !fstab_is_ok(NEW_ROOT) {
        return Err(anyhow::anyhow!("fstab not found"));
    }

    logln!("{}", "Initializing hostname...".cyan());
    if init_hostname(NEW_ROOT).is_err() {
        logln!("{}", "Failed to initialize hostname.".red());
        return Err(anyhow::anyhow!("Failed to initialize hostname"));
    }

    let version = get_version(NEW_ROOT)?;

    logln!("{}", "Checking updates...".cyan());
    if check_for_updates(&version, UPDATE_URL) {
        logln!("{}", "Updates found.".cyan());

        let update = Text::new("Update?")
            .with_default("yes")
            .prompt()?;

        if update.trim().eq_ignore_ascii_case("yes") {
            logln!("{}", "Updating...".cyan());
            update_kernel(UPDATE_URL)?;
        }
    }

    logln!("{}", "Initializing network...".cyan());
    
    if let Err(e) = init_network(NEW_ROOT) {
        logln!("{}", "Failed to initialize network.".red());
        return Err(anyhow::anyhow!(
            "Failed to initialize network: {}",
            e
        ));
    }

    if switch_root(NEW_ROOT).is_err() {
        logln!("{}", "Failed to switch root.".red());
        return Err(anyhow::anyhow!("Failed to switch root"));
    }

    // new root is /
    const ROOT: &str = "/";

    logln!("{}", "Starting services...".cyan());
    if start_services().is_err() {
        logln!("{}", "Failed to start services.".red());
        return Err(anyhow::anyhow!("Failed to start services"));
    }

    logln!("{}", "Initializing keymaps...".cyan());
    if init_keymaps(ROOT).is_err() {
        logln!("{}", "Failed to initialize keymaps.".red());
        return Err(anyhow::anyhow!("Failed to initialize keymaps"));
    }

    logln!("{}", "Initializing variables...".cyan());
    
    init_vars(ROOT)
        .map_err(|e| {
            logln!(
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
                logln!(
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
        logln!(
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
