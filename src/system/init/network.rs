use anyhow::{Result, anyhow};

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::Command;

use crate::log_info;

use crate::utils::network::{detect_network_interfaces, filter_network_interfaces};

/// Copie du script présente dans l'initramfs (même source que le rootfs du build).
const INITRAMFS_UDHCPC_SCRIPT: &str = "/usr/share/udhcpc/default.script";

/// Interfaces décrites par /etc/network/interfaces : (nom, mode).
/// `iface eth0 inet dhcp` -> ("eth0", "dhcp"). Les lignes `auto`, les
/// commentaires et les lignes indentées (address, gateway...) sont ignorés.
fn parse_interfaces(config: &str) -> Vec<(String, String)> {
    config
        .lines()
        .filter_map(|line| {
            let line = line.trim();
            if !line.starts_with("iface ") {
                return None;
            }
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 4 {
                Some((parts[1].to_string(), parts[3].to_lowercase()))
            } else {
                None
            }
        })
        .collect()
}

fn read_interfaces(root: &str) -> Result<Vec<(String, String)>> {
    let config_path = Path::new(root).join("etc/network/interfaces");

    let config = fs::read_to_string(&config_path)
        .map_err(|e| anyhow!("Failed to read {}: {}", config_path.display(), e))?;

    Ok(parse_interfaces(&config))
}

/// Interfaces à configurer : celles que le noyau a détectées ET que
/// /etc/network/interfaces décrit. Une interface détectée mais non décrite
/// (carte non choisie à l'installation) est ignorée : ce n'est pas une erreur.
fn interfaces_to_configure(
    detected: &[String],
    configured: &[(String, String)],
) -> Vec<(String, String)> {
    detected
        .iter()
        .filter_map(|name| {
            configured
                .iter()
                .find(|(configured_name, _)| configured_name == name)
                .cloned()
        })
        .collect()
}

fn setup_resolv_conf(root: &str) -> Result<()> {
    let resolv_path = Path::new(root).join("etc/resolv.conf");

    fs::write(&resolv_path, "nameserver 10.0.2.3\nnameserver 1.1.1.1\n")
        .map_err(|e| anyhow!("Failed to write {}: {}", resolv_path.display(), e))?;

    Ok(())
}

fn prepare_dhcp(root: &str) -> Result<()> {
    /*
     * BusyBox ifupdown calls run-parts on these
     * directories. They must exist before ifup.
     */
    for dir in [
        "etc/network/if-pre-up.d",
        "etc/network/if-up.d",
        "etc/network/if-down.d",
        "etc/network/if-post-down.d",
        "usr/share/udhcpc",
    ] {
        let path = Path::new(root).join(dir);

        fs::create_dir_all(&path)
            .map_err(|e| anyhow!("Failed to create {}: {}", path.display(), e))?;
    }

    /*
     * default.script comes from the rootfs overlay.
     * The installed disk may be older than the current
     * build, so it is synchronised from the initramfs
     * copy when missing or different.
     */
    let script = Path::new(root).join("usr/share/udhcpc/default.script");

    let fallback = Path::new(INITRAMFS_UDHCPC_SCRIPT);

    let needs_copy = if !script.exists() {
        true
    } else if fallback.exists() {
        fs::read(fallback).ok() != fs::read(&script).ok()
    } else {
        false
    };

    if needs_copy {
        if !fallback.exists() {
            return Err(anyhow!(
                "udhcpc default.script not found: {} (nor in initramfs: {})",
                script.display(),
                fallback.display()
            ));
        }

        fs::copy(fallback, &script).map_err(|e| {
            anyhow!(
                "Failed to copy {} to {}: {}",
                fallback.display(),
                script.display(),
                e
            )
        })?;
    }

    /*
     * Make sure BusyBox can execute the script.
     */
    let mut permissions = fs::metadata(&script)?.permissions();

    permissions.set_mode(0o755);

    fs::set_permissions(&script, permissions)
        .map_err(|e| anyhow!("Failed to chmod {}: {}", script.display(), e))?;

    /*
     * /bin/sh is required by the script.
     */
    let sh = Path::new(root).join("bin/sh");

    if !sh.exists() {
        return Err(anyhow!(
            "/bin/sh not found in target rootfs: {}",
            sh.display()
        ));
    }

    /*
     * default.script uses the `ip` command.
     */
    let has_ip = ["bin/ip", "sbin/ip", "usr/bin/ip", "usr/sbin/ip"]
        .iter()
        .any(|path| Path::new(root).join(path).exists());

    if !has_ip {
        return Err(anyhow!("`ip` applet not found in target rootfs"));
    }

    Ok(())
}

/// Passerelle par défaut d'après /proc/net/route (adresse en hexadécimal, octets
/// dans l'ordre de la machine : "0202000A" = 10.0.2.2).
fn parse_default_gateway(route_table: &str) -> Option<String> {
    for line in route_table.lines().skip(1) {
        let fields: Vec<&str> = line.split_whitespace().collect();
        if fields.len() < 3 || fields[1] != "00000000" {
            continue;
        }
        let Ok(raw) = u32::from_str_radix(fields[2], 16) else {
            continue;
        };
        if raw == 0 {
            continue;
        }
        return Some(std::net::Ipv4Addr::from(raw.to_le_bytes()).to_string());
    }
    None
}

/// Teste la connectivité : la passerelle par défaut doit répondre au ping.
/// (Avant, l'adresse 10.0.2.2 de QEMU était codée en dur : le test échouait
/// toujours hors de QEMU.)
pub fn test_network(root: &str) -> Result<()> {
    log_info!("Testing network connection...");

    let route_table = fs::read_to_string("/proc/net/route").unwrap_or_default();
    let gateway = parse_default_gateway(&route_table).ok_or_else(|| {
        anyhow!("no default route: the DHCP server did not answer or the interface has no address")
    })?;

    for attempt in 1..=3 {
        let status = Command::new("chroot")
            .arg(root)
            .arg("ping")
            .args(["-c", "1", "-W", "3", gateway.as_str()])
            .status()
            .map_err(|e| anyhow!("Failed to execute ping: {}", e))?;

        if status.success() {
            log_info!("Network connection is working (gateway {}).", gateway);
            return Ok(());
        }
        log_info!("Gateway {} did not answer (attempt {}/3).", gateway, attempt);
    }

    Err(anyhow!("the gateway {} does not answer", gateway))
}

/// Efface l'état d'ifup laissé par un démarrage précédent.
///
/// BusyBox ifup mémorise les interfaces « montées » dans un fichier d'état
/// (/var/run/ifstate). Sur le disque installé ce fichier survivait au
/// redémarrage : au démarrage suivant `ifup` répondait « interface eth0
/// already configured » sans rien faire, l'interface restait sans adresse et
/// le test réseau faisait paniquer le système au SECOND redémarrage.
fn clear_stale_ifstate(root: &str) {
    for relative in [
        "var/run/ifstate",
        "run/ifstate",
        "etc/network/run/ifstate",
        "var/lib/ifstate",
    ] {
        let _ = fs::remove_file(Path::new(root).join(relative));
    }
}

/// Configure (ifup) les interfaces détectées qui sont décrites dans
/// /etc/network/interfaces ; renvoie leurs noms.
pub fn configure_network(root: &str, interfaces: &[String]) -> Result<Vec<String>> {
    if interfaces.is_empty() {
        return Err(anyhow!("No network interfaces found"));
    }

    let config = Path::new(root).join("etc/network/interfaces");

    if !config.exists() {
        return Err(anyhow!(
            "Network configuration not found: {}",
            config.display()
        ));
    }

    let ifup = Path::new(root).join("sbin/ifup");

    if !ifup.exists() {
        return Err(anyhow!(
            "ifup not found in target rootfs: {}",
            ifup.display()
        ));
    }

    let configured = read_interfaces(root)?;
    let to_configure = interfaces_to_configure(interfaces, &configured);

    for interface in interfaces {
        if !to_configure.iter().any(|(name, _)| name == interface) {
            log_info!("Ignoring {} (not described in /etc/network/interfaces).", interface);
        }
    }

    if to_configure.is_empty() {
        let described: Vec<&str> = configured.iter().map(|(name, _)| name.as_str()).collect();
        return Err(anyhow!(
            "none of the detected interfaces ({}) is described in /etc/network/interfaces (it describes: {})",
            interfaces.join(", "),
            if described.is_empty() { "nothing".to_string() } else { described.join(", ") }
        ));
    }

    clear_stale_ifstate(root);

    let mut done = Vec::new();

    for (interface, mode) in &to_configure {
        log_info!("Configuring {} using {}...", interface, mode);

        match mode.as_str() {
            "dhcp" | "static" => {
                let status = Command::new("chroot")
                    .arg(root)
                    .arg("/sbin/ifup")
                    .arg(interface)
                    .status()
                    .map_err(|e| anyhow!("Failed to execute ifup for {}: {}", interface, e))?;

                if !status.success() {
                    return Err(anyhow!(
                        "{} configuration failed for {}",
                        mode.to_uppercase(),
                        interface
                    ));
                }
                done.push(interface.clone());
            }

            "loopback" => {}

            _ => {
                return Err(anyhow!(
                    "Unsupported network mode '{}' for {}",
                    mode,
                    interface
                ));
            }
        }
    }

    Ok(done)
}

pub fn init_network(root: &str) -> Result<()> {
    prepare_dhcp(root)?;

    let interfaces = detect_network_interfaces()?;

    let filtered = filter_network_interfaces(&interfaces);

    if filtered.is_empty() {
        return Err(anyhow!("No usable network interfaces found"));
    }

    let configured = configure_network(root, &filtered)?;
    log_info!("Network interface(s) configured: {}", configured.join(", "));

    let resolv = Path::new(root).join("etc/resolv.conf");

    if !resolv.exists() {
        setup_resolv_conf(root)?;
    }

    test_network(root)?;

    log_info!("Network initialized successfully.");

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_the_interfaces_file() {
        let config = "auto lo\niface lo inet loopback\n\nauto eth0\niface eth0 inet static\n    address 192.168.1.10\n    netmask 255.255.255.0\n# iface ghost inet dhcp\n";
        assert_eq!(
            parse_interfaces(config),
            vec![
                ("lo".to_string(), "loopback".to_string()),
                ("eth0".to_string(), "static".to_string()),
            ]
        );
    }

    #[test]
    fn only_described_interfaces_are_configured() {
        // The reported case: dummy0 is detected but was never configured.
        let configured = vec![("eth0".to_string(), "dhcp".to_string())];
        let detected = vec!["dummy0".to_string(), "eth0".to_string()];
        assert_eq!(
            interfaces_to_configure(&detected, &configured),
            vec![("eth0".to_string(), "dhcp".to_string())]
        );
        // Nothing described for the detected card: empty (the caller reports it).
        let other = vec!["enp0s3".to_string()];
        assert!(interfaces_to_configure(&other, &configured).is_empty());
    }

    #[test]
    fn the_configured_name_follows_the_detected_card() {
        // Installed on a machine whose card is enp0s3: that is the name written
        // by the installer and the one configured at boot (never an assumed eth0).
        let configured = vec![("enp0s3".to_string(), "dhcp".to_string())];
        let detected = vec!["enp0s3".to_string()];
        assert_eq!(interfaces_to_configure(&detected, &configured).len(), 1);
    }

    #[test]
    fn parses_the_default_gateway() {
        let table = "Iface\tDestination\tGateway\tFlags\tRefCnt\tUse\tMetric\tMask\tMTU\tWindow\tIRTT\n\
                     eth0\t00000000\t0202000A\t0003\t0\t0\t0\t00000000\t0\t0\t0\n\
                     eth0\t0002000A\t00000000\t0001\t0\t0\t0\t00FFFFFF\t0\t0\t0\n";
        assert_eq!(parse_default_gateway(table).as_deref(), Some("10.0.2.2"));
    }

    #[test]
    fn no_default_route_means_no_gateway() {
        let table = "Iface\tDestination\tGateway\tFlags\n\
                     eth0\t0002000A\t00000000\t0001\n";
        assert_eq!(parse_default_gateway(table), None);
        assert_eq!(parse_default_gateway(""), None);
    }

    #[test]
    fn stale_ifstate_is_removed() {
        let dir = std::env::temp_dir().join(format!("senbit-ifstate-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("var/run")).unwrap();
        fs::write(dir.join("var/run/ifstate"), "eth0=eth0\n").unwrap();

        clear_stale_ifstate(dir.to_str().unwrap());

        assert!(!dir.join("var/run/ifstate").exists());
        let _ = fs::remove_dir_all(&dir);
    }
}
