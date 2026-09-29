use crate::logln;

use std::process::Command;

use std::path::Path;

use anyhow::{Result, anyhow};

use inquire::Select;

use colored::Colorize;

fn detect_network_interfaces() -> Result<Vec<String>> {
    let output = Command::new("ip")
        .args(["-o", "link"])
        .output()
        .map_err(|e| anyhow!("Failed to execute ip: {}", e))?;

    if !output.status.success() {
        return Err(anyhow!(
            "ip command failed with status: {}",
            output.status
        ));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);

    let interfaces = stdout
        .lines()
        .filter_map(|line| {
            let name = line.split(':').nth(1)?.trim();

            if name == "lo" {
                return None;
            }

            Some(name.to_string())
        })
        .collect();

    Ok(interfaces)
}

fn filter_network_interfaces(interfaces: &[String]) -> Vec<String> {
    // Filter out virtual interfaces (e.g., veth, docker)
    interfaces
        .iter()
        .filter(|name| !name.contains("veth"))
        .cloned()
        .collect()
}

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
    
    logln!("{}", format!("Selected interface: {}", interface).green().bold());
    logln!("{}", format!("Selected mode: {}", mode).green().bold());

    logln!("{}", "Writing network config...".green().bold());
    if write_network_config(mount, &interface, &mode).is_err() {
        return Err(anyhow!("Failed to write network config"));
    }

    logln!("{}", "Preparing network start...".green().bold());
    if prepare_network_start(mount).is_err() {
        return Err(anyhow!("Failed to prepare network start"));
    }

    logln!("{}", "Network setup complete!".green().bold());    
    Ok(())
}