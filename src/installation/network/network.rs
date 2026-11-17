//! Réseau de l'installation : `ask_network` pose les questions (rien n'est écrit),
//! `apply_network` écrit la configuration après la confirmation finale.
//!
//! La configuration est celle de BusyBox ifupdown (/etc/network/interfaces),
//! lue au démarrage par senbit-init :
//!
//!   DHCP    auto eth0 / iface eth0 inet dhcp
//!   statique  + address, netmask, gateway ; DNS dans /etc/resolv.conf
//!
//! Pour changer plus tard : éditer /etc/network/interfaces puis `ifdown eth0 && ifup eth0`.

use crate::log_info;

use std::net::Ipv4Addr;
use std::path::Path;

use anyhow::{Result, anyhow};

use crate::utils::network::{base_name, detect_network_interfaces, filter_network_interfaces};

use inquire::validator::Validation;
use inquire::{Select, Text};

use colored::Colorize;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NetworkMode {
    Dhcp,
    Static {
        address: Ipv4Addr,
        prefix: u8,
        gateway: Option<Ipv4Addr>,
        dns: Vec<Ipv4Addr>,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NetworkPlan {
    pub interface: String,
    pub mode: NetworkMode,
}

impl NetworkPlan {
    /// Description d'une ligne pour le récapitulatif.
    pub fn summary(&self) -> String {
        match &self.mode {
            NetworkMode::Dhcp => format!("{} (DHCP)", self.interface),
            NetworkMode::Static {
                address,
                prefix,
                gateway,
                dns,
            } => {
                let mut text = format!("{} (static {}/{}", self.interface, address, prefix);
                if let Some(gateway) = gateway {
                    text.push_str(&format!(", gateway {gateway}"));
                }
                if !dns.is_empty() {
                    let list: Vec<String> = dns.iter().map(|d| d.to_string()).collect();
                    text.push_str(&format!(", dns {}", list.join(" ")));
                }
                text.push(')');
                text
            }
        }
    }
}

/// "192.168.1.10/24" -> (192.168.1.10, 24)
pub fn parse_cidr(text: &str) -> Result<(Ipv4Addr, u8)> {
    let (address, prefix) = text
        .trim()
        .split_once('/')
        .ok_or_else(|| anyhow!("expected ADDRESS/PREFIX, e.g. 192.168.1.10/24"))?;

    let address: Ipv4Addr = address
        .parse()
        .map_err(|_| anyhow!("`{}` is not a valid IPv4 address", address))?;

    let prefix: u8 = prefix
        .parse()
        .map_err(|_| anyhow!("`{}` is not a valid prefix length", prefix))?;

    if !(1..=32).contains(&prefix) {
        return Err(anyhow!("the prefix must be between 1 and 32"));
    }

    Ok((address, prefix))
}

pub fn prefix_to_netmask(prefix: u8) -> Ipv4Addr {
    let bits: u32 = if prefix == 0 {
        0
    } else {
        u32::MAX << (32 - u32::from(prefix.min(32)))
    };
    Ipv4Addr::from(bits)
}

/// Liste de serveurs DNS séparés par des virgules ou des espaces.
pub fn parse_dns_list(text: &str) -> Result<Vec<Ipv4Addr>> {
    text.split([',', ' '])
        .filter(|s| !s.is_empty())
        .map(|s| {
            s.parse::<Ipv4Addr>()
                .map_err(|_| anyhow!("`{}` is not a valid IPv4 address", s))
        })
        .collect()
}

/// The card to configure: the only real network card is selected automatically
/// (no question, no assumption about its name: eth0, ens3, enp1s0...), the user
/// is only asked when there are several.
fn pick_interface(interfaces: &[String]) -> Result<String> {
    match interfaces {
        [] => Err(anyhow!("No network interfaces found")),
        [only] => {
            let name = base_name(only).to_string();
            log_info!(
                "{}",
                format!("Network interface: {name} (the only network card, selected automatically)")
                    .green()
                    .bold()
            );
            Ok(name)
        }
        _ => {
            let choice = Select::new("Select a network interface:", interfaces.to_vec()).prompt()?;
            Ok(base_name(&choice).to_string())
        }
    }
}

fn ask_static() -> Result<NetworkMode> {
    let address_text = Text::new("IPv4 address with prefix (e.g. 192.168.1.10/24):")
        .with_validator(|input: &str| match parse_cidr(input) {
            Ok(_) => Ok(Validation::Valid),
            Err(e) => Ok(Validation::Invalid(e.to_string().into())),
        })
        .prompt()?;
    let (address, prefix) = parse_cidr(&address_text)?;

    let gateway_text = Text::new("Default gateway (empty = none):")
        .with_validator(|input: &str| {
            if input.trim().is_empty() || input.trim().parse::<Ipv4Addr>().is_ok() {
                Ok(Validation::Valid)
            } else {
                Ok(Validation::Invalid("Not a valid IPv4 address.".into()))
            }
        })
        .prompt()?;
    let gateway = if gateway_text.trim().is_empty() {
        None
    } else {
        Some(gateway_text.trim().parse::<Ipv4Addr>()?)
    };

    let dns_text = Text::new("DNS servers (comma separated):")
        .with_default("1.1.1.1")
        .with_validator(|input: &str| match parse_dns_list(input) {
            Ok(_) => Ok(Validation::Valid),
            Err(e) => Ok(Validation::Invalid(e.to_string().into())),
        })
        .prompt()?;
    let dns = parse_dns_list(&dns_text)?;

    Ok(NetworkMode::Static {
        address,
        prefix,
        gateway,
        dns,
    })
}

/// Questions seulement : interface, DHCP ou statique (adresse, passerelle, DNS).
pub fn ask_network() -> Result<NetworkPlan> {
    let interfaces = detect_network_interfaces()?;
    if interfaces.is_empty() {
        return Err(anyhow!("No network interfaces found"));
    }

    // Real network cards only: lo, dummy0, bridges, veth, docker0... are never offered.
    let real_interfaces = filter_network_interfaces(&interfaces);
    if real_interfaces.is_empty() {
        return Err(anyhow!(
            "No real network card found (only virtual interfaces: {})",
            interfaces.join(", ")
        ));
    }

    let interface = pick_interface(&real_interfaces)?;
    if interface.is_empty() {
        return Err(anyhow!("No interface selected"));
    }

    let mode = Select::new("Select a network mode:", vec!["DHCP", "Static"]).prompt()?;
    let mode = if mode == "Static" {
        ask_static()?
    } else {
        NetworkMode::Dhcp
    };

    Ok(NetworkPlan { interface, mode })
}

/// Contenu de /etc/network/interfaces.
pub fn render_interfaces(plan: &NetworkPlan) -> String {
    let interface = &plan.interface;
    match &plan.mode {
        NetworkMode::Dhcp => format!("auto {interface}\niface {interface} inet dhcp\n"),
        NetworkMode::Static {
            address,
            prefix,
            gateway,
            ..
        } => {
            let mut text = format!("auto {interface}\niface {interface} inet static\n");
            text.push_str(&format!("    address {address}\n"));
            text.push_str(&format!("    netmask {}\n", prefix_to_netmask(*prefix)));
            if let Some(gateway) = gateway {
                text.push_str(&format!("    gateway {gateway}\n"));
            }
            text
        }
    }
}

fn write_network_config(mount: &Path, plan: &NetworkPlan) -> Result<()> {
    let network_dir = mount.join("etc/network");

    std::fs::create_dir_all(&network_dir)?;
    std::fs::write(network_dir.join("interfaces"), render_interfaces(plan))?;

    // Static: no DHCP client will write the DNS servers, so they are written now.
    if let NetworkMode::Static { dns, .. } = &plan.mode {
        if !dns.is_empty() {
            let mut resolv = String::from("# Written by the Senbit installer (static network)\n");
            for server in dns {
                resolv.push_str(&format!("nameserver {server}\n"));
            }
            std::fs::write(mount.join("etc/resolv.conf"), resolv)?;
        }
    }

    Ok(())
}

/// Active (DHCP) ou désactive (statique) le client DHCP de l'interface choisie :
/// seule `senbit-dhcp@eth0` est activée dans l'image.
fn configure_dhcp_unit(mount: &Path, plan: &NetworkPlan) -> Result<()> {
    let wants = mount.join("etc/systemd/system/multi-user.target.wants");
    let link = wants.join(format!("senbit-dhcp@{}.service", plan.interface));

    match plan.mode {
        NetworkMode::Dhcp => {
            std::fs::create_dir_all(&wants)?;
            if std::fs::symlink_metadata(&link).is_err() {
                std::os::unix::fs::symlink("/usr/lib/systemd/system/senbit-dhcp@.service", &link)?;
            }
            // The image enables senbit-dhcp@eth0 by default: when the card has
            // another name, the DHCP client of the chosen card is the only one.
            if plan.interface != "eth0" {
                let _ = std::fs::remove_file(wants.join("senbit-dhcp@eth0.service"));
            }
        }
        NetworkMode::Static { .. } => {
            // A DHCP client and a static address do not mix.
            let _ = std::fs::remove_file(&link);
        }
    }

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

/// Écrit la configuration réseau choisie (après la confirmation finale).
pub fn apply_network(mount: &Path, plan: &NetworkPlan) -> Result<()> {
    log_info!("{}", format!("Network: {}", plan.summary()).green().bold());

    log_info!("{}", "Writing network config...".green().bold());
    write_network_config(mount, plan).map_err(|e| anyhow!("Failed to write network config: {e}"))?;

    configure_dhcp_unit(mount, plan).map_err(|e| anyhow!("Failed to set up the DHCP client: {e}"))?;

    log_info!("{}", "Preparing network start...".green().bold());
    prepare_network_start(mount).map_err(|e| anyhow!("Failed to prepare network start: {e}"))?;

    log_info!("{}", "Network setup complete!".green().bold());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn static_plan() -> NetworkPlan {
        NetworkPlan {
            interface: "eth0".into(),
            mode: NetworkMode::Static {
                address: "192.168.1.10".parse().unwrap(),
                prefix: 24,
                gateway: Some("192.168.1.1".parse().unwrap()),
                dns: vec!["1.1.1.1".parse().unwrap(), "9.9.9.9".parse().unwrap()],
            },
        }
    }

    #[test]
    fn cidr_parsing() {
        assert_eq!(
            parse_cidr("192.168.1.10/24").unwrap(),
            ("192.168.1.10".parse().unwrap(), 24)
        );
        assert!(parse_cidr("192.168.1.10").is_err());
        assert!(parse_cidr("192.168.1.10/0").is_err());
        assert!(parse_cidr("192.168.1.10/33").is_err());
        assert!(parse_cidr("nope/24").is_err());
        assert!(parse_cidr("1.2.3.4/x").is_err());
    }

    #[test]
    fn netmasks() {
        assert_eq!(prefix_to_netmask(24).to_string(), "255.255.255.0");
        assert_eq!(prefix_to_netmask(16).to_string(), "255.255.0.0");
        assert_eq!(prefix_to_netmask(22).to_string(), "255.255.252.0");
        assert_eq!(prefix_to_netmask(32).to_string(), "255.255.255.255");
    }

    #[test]
    fn dns_lists() {
        assert_eq!(parse_dns_list("1.1.1.1, 9.9.9.9").unwrap().len(), 2);
        assert_eq!(parse_dns_list("1.1.1.1,9.9.9.9").unwrap().len(), 2);
        assert!(parse_dns_list("1.1.1.1 nope").is_err());
        assert!(parse_dns_list("").unwrap().is_empty());
    }

    #[test]
    fn dhcp_config() {
        let plan = NetworkPlan {
            interface: "eth0".into(),
            mode: NetworkMode::Dhcp,
        };
        assert_eq!(render_interfaces(&plan), "auto eth0\niface eth0 inet dhcp\n");
        assert_eq!(plan.summary(), "eth0 (DHCP)");
    }

    #[test]
    fn static_config_is_complete() {
        // Before: "iface eth0 inet static" alone, which ifup rejects.
        let text = render_interfaces(&static_plan());
        assert!(text.contains("iface eth0 inet static\n"));
        assert!(text.contains("    address 192.168.1.10\n"));
        assert!(text.contains("    netmask 255.255.255.0\n"));
        assert!(text.contains("    gateway 192.168.1.1\n"));
        assert_eq!(
            static_plan().summary(),
            "eth0 (static 192.168.1.10/24, gateway 192.168.1.1, dns 1.1.1.1 9.9.9.9)"
        );
    }

    #[test]
    fn static_config_writes_resolv_and_disables_dhcp() {
        let dir = std::env::temp_dir().join(format!("senbit-net-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let wants = dir.join("etc/systemd/system/multi-user.target.wants");
        std::fs::create_dir_all(&wants).unwrap();
        let link = wants.join("senbit-dhcp@eth0.service");
        std::os::unix::fs::symlink("/usr/lib/systemd/system/senbit-dhcp@.service", &link).unwrap();

        let plan = static_plan();
        write_network_config(&dir, &plan).unwrap();
        configure_dhcp_unit(&dir, &plan).unwrap();

        let resolv = std::fs::read_to_string(dir.join("etc/resolv.conf")).unwrap();
        assert!(resolv.contains("nameserver 1.1.1.1\n"));
        assert!(std::fs::symlink_metadata(&link).is_err());

        // The image's default (eth0) client goes away when another card is chosen.
        std::os::unix::fs::symlink("/usr/lib/systemd/system/senbit-dhcp@.service", &link).unwrap();
        // Back to DHCP on another interface: its unit is enabled.
        let dhcp = NetworkPlan {
            interface: "eth1".into(),
            mode: NetworkMode::Dhcp,
        };
        configure_dhcp_unit(&dir, &dhcp).unwrap();
        assert!(std::fs::symlink_metadata(wants.join("senbit-dhcp@eth1.service")).is_ok());
        assert!(std::fs::symlink_metadata(&link).is_err());

        let _ = std::fs::remove_dir_all(&dir);
    }
}
