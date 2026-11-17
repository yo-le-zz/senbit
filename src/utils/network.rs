//! Détection des interfaces réseau.
//!
//! Une interface « utilisable » est une carte réseau réelle : le noyau lui
//! associe un périphérique (`/sys/class/net/<nom>/device`). `lo`, `dummy0`,
//! les ponts, `veth*`, `docker0`, les tunnels... n'en ont pas et ne doivent
//! JAMAIS être proposés à l'installation ni configurés au démarrage.
//!
//! (Avec le noyau compilé pour Docker, `dummy0` apparaissait à chaque boot :
//! `ip link` la liste, elle n'a aucune configuration, et le démarrage
//! affichait « No network configuration found for interface dummy0 ».)

use anyhow::Result;
use anyhow::anyhow;
use std::path::Path;
use std::process::Command;

pub fn detect_network_interfaces() -> Result<Vec<String>> {
    let output = Command::new("ip")
        .args(["-o", "link"])
        .output()
        .map_err(|e| anyhow!("Failed to execute ip: {}", e))?;

    if !output.status.success() {
        return Err(anyhow!("ip command failed with status: {}", output.status));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);

    Ok(parse_ip_link(&stdout))
}

/// Noms d'interfaces d'une sortie de `ip -o link` (sans `lo`).
fn parse_ip_link(output: &str) -> Vec<String> {
    output
        .lines()
        .filter_map(|line| {
            let name = line.split(':').nth(1)?.trim();

            if name.is_empty() || name == "lo" {
                return None;
            }

            Some(name.to_string())
        })
        .collect()
}

/// `veth1234@if5` -> `veth1234`
pub fn base_name(interface: &str) -> &str {
    interface.split('@').next().unwrap_or(interface)
}

/// Interfaces qui ne sont jamais une carte réseau, d'après leur nom.
fn is_virtual_name(name: &str) -> bool {
    const PREFIXES: &[&str] = &[
        "lo", "dummy", "sit", "ip6", "ip_vti", "ip6tnl", "gre", "gretap", "erspan", "tun", "tap",
        "docker", "br-", "virbr", "veth", "bond", "team", "ifb", "vxlan", "macvlan", "macvtap",
        "ipvlan", "wg", "tailscale", "cni", "flannel", "cali", "nodelocaldns",
    ];

    PREFIXES.iter().any(|prefix| name.starts_with(prefix))
}

/// Le noyau associe-t-il un périphérique à cette interface ? (carte PCI,
/// virtio, USB... : oui ; dummy, pont, veth, tunnel : non)
fn has_hardware(name: &str) -> bool {
    Path::new("/sys/class/net")
        .join(name)
        .join("device")
        .exists()
}

/// Garde les cartes réseau réelles.
///
/// 1. on retire les noms qui ne sont jamais une carte (dummy0, docker0, veth...) ;
/// 2. s'il reste des interfaces adossées à un périphérique, ce sont les seules gardées ;
/// 3. sinon (sysfs illisible, noyau sans `device`), on garde le résultat de l'étape 1.
pub fn filter_network_interfaces(interfaces: &[String]) -> Vec<String> {
    filter_with(interfaces, &has_hardware)
}

fn filter_with(interfaces: &[String], hardware: &dyn Fn(&str) -> bool) -> Vec<String> {
    let named: Vec<String> = interfaces
        .iter()
        .filter(|interface| !is_virtual_name(base_name(interface)))
        .cloned()
        .collect();

    let with_hardware: Vec<String> = named
        .iter()
        .filter(|interface| hardware(base_name(interface)))
        .cloned()
        .collect();

    if with_hardware.is_empty() {
        named
    } else {
        with_hardware
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn parses_ip_link_output() {
        let output = "1: lo: <LOOPBACK,UP,LOWER_UP> mtu 65536 qdisc noqueue state UNKNOWN\\    link/loopback 00:00:00:00:00:00\n\
                      2: eth0: <BROADCAST,MULTICAST,UP,LOWER_UP> mtu 1500 qdisc fq_codel state UP\\    link/ether 52:54:00:12:34:56\n\
                      3: dummy0: <BROADCAST,NOARP> mtu 1500 qdisc noop state DOWN\\    link/ether 8e:1f:00:00:00:01\n\
                      4: veth1@if5: <BROADCAST,MULTICAST,UP> mtu 1500\\    link/ether 02:00:00:00:00:01\n";
        assert_eq!(parse_ip_link(output), names(&["eth0", "dummy0", "veth1@if5"]));
    }

    #[test]
    fn dummy0_is_never_a_network_card() {
        // The reported case: QEMU's card (eth0) and the kernel's dummy0.
        let found = names(&["dummy0", "eth0"]);
        assert_eq!(filter_with(&found, &|name| name == "eth0"), names(&["eth0"]));
        // Even if sysfs cannot tell, the name is enough.
        assert_eq!(filter_with(&found, &|_| false), names(&["eth0"]));
    }

    #[test]
    fn virtual_interfaces_are_removed() {
        let found = names(&["docker0", "br-1a2b3c", "veth9@if7", "tun0", "virbr0", "wg0", "enp1s0"]);
        assert_eq!(filter_with(&found, &|_| true), names(&["enp1s0"]));
    }

    #[test]
    fn the_name_is_not_assumed_to_be_eth0() {
        let found = names(&["dummy0", "enp0s3"]);
        assert_eq!(filter_with(&found, &|name| name == "enp0s3"), names(&["enp0s3"]));
    }

    #[test]
    fn several_cards_are_all_kept() {
        let found = names(&["eth0", "eth1", "dummy0"]);
        assert_eq!(
            filter_with(&found, &|name| name.starts_with("eth")),
            names(&["eth0", "eth1"])
        );
    }

    #[test]
    fn hardware_wins_over_names_that_look_real() {
        // A bridge called "lan0" has no device: the real card is kept alone.
        let found = names(&["lan0", "eth0"]);
        assert_eq!(filter_with(&found, &|name| name == "eth0"), names(&["eth0"]));
    }
}
