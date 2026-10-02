use crate::log_info;

use std::path::Path;

use anyhow::{Result, anyhow};

use crate::utils::network::{
    detect_network_interfaces,
    filter_network_interfaces
};

use inquire::Select;

use colored::Colorize;

fn ask_interface(interfaces: &[String]) -> Result<String> {
    if interfaces.is_empty() {
        return Err(anyhow!("No network interfaces found"));
    }

    let interface = Select::new(
        "Select a network interface:",
        interfaces.to_vec(),
    )
    .prompt()?;

    Ok(interface)
}

fn select_network_mode() -> Result<String> {
    let mode = Select::new(
        "Select a network mode:",
        vec!["DHCP", "Static"].to_vec(),
    )
    .prompt()?;

    Ok(mode.to_lowercase())
}

fn write_network_config(
    mount: &Path,
    interface: &str,
    mode: &str,
) -> Result<()> {
    let network_dir = mount.join("etc/network");

    std::fs::create_dir_all(&network_dir)?;

    let config = match mode {
        "dhcp" => format!(
            "auto {}\niface {} inet dhcp\n",
            interface,
            interface
        ),

        "static" => format!(
            "auto {}\niface {} inet static\n",
            interface,
            interface
        ),

        _ => {
            return Err(anyhow!(
                "Unsupported network mode: {}",
                mode
            ));
        }
    };

    std::fs::write(
        network_dir.join("interfaces"),
        config,
    )?;

    Ok(())
}

fn prepare_network_start(mount: &Path) -> Result<()> {
    let ifup = mount.join("sbin/ifup");
    let ifdown = mount.join("sbin/ifdown");

    if !ifup.exists() {
        return Err(anyhow!(
            "ifup not found in target rootfs: {}",
            ifup.display()
        ));
    }

    if !ifdown.exists() {
        return Err(anyhow!(
            "ifdown not found in target rootfs: {}",
            ifdown.display()
        ));
    }

    Ok(())
}

pub fn setup_network(mount: &Path) -> Result<()> {
    let interfaces = detect_network_interfaces()?;
    if interfaces.is_empty() {
        return Err(anyhow!("No network interfaces found"));
    }
    
    let real_interfaces = filter_network_interfaces(&interfaces);
    if real_interfaces.is_empty() {
        return Err(anyhow!("No real network interfaces found"));
    }

    let interface = ask_interface(&real_interfaces)?;
    if interface.is_empty() {
        return Err(anyhow!("No interface selected"));
    }
    
    let mode = select_network_mode()?;
    if mode.is_empty() {
        return Err(anyhow!("No mode selected"));
    }
    
    log_info!("{}", format!("Selected interface: {}", interface).green().bold());
    log_info!("{}", format!("Selected mode: {}", mode).green().bold());

    log_info!("{}", "Writing network config...".green().bold());
    if write_network_config(mount, &interface, &mode).is_err() {
        return Err(anyhow!("Failed to write network config"));
    }

    log_info!("{}", "Preparing network start...".green().bold());
    if prepare_network_start(mount).is_err() {
        return Err(anyhow!("Failed to prepare network start"));
    }

    log_info!("{}", "Network setup complete!".green().bold());    
    Ok(())
}