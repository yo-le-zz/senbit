use anyhow::{anyhow, Result};

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::Command;

use crate::log_info;

use crate::utils::network::{
    detect_network_interfaces,
    filter_network_interfaces,
};

/// Copie du script présente dans l'initramfs (même source que le rootfs du build).
const INITRAMFS_UDHCPC_SCRIPT: &str = "/usr/share/udhcpc/default.script";

fn get_interface_mode(
    root: &str,
    interface: &str,
) -> Result<String> {
    let config_path =
        Path::new(root)
            .join("etc/network/interfaces");

    let config =
        fs::read_to_string(&config_path)
            .map_err(|e| {
                anyhow!(
                    "Failed to read {}: {}",
                    config_path.display(),
                    e
                )
            })?;

    for line in config.lines() {
        let line = line.trim();

        if line.starts_with("iface ") {
            let parts: Vec<&str> =
                line.split_whitespace()
                    .collect();

            if parts.len() >= 4
                && parts[1] == interface
            {
                return Ok(
                    parts[3].to_lowercase()
                );
            }
        }
    }

    Err(anyhow!(
        "No network configuration found for interface {}",
        interface
    ))
}

fn setup_resolv_conf(
    root: &str,
) -> Result<()> {
    let resolv_path =
        Path::new(root)
            .join("etc/resolv.conf");

    fs::write(
        &resolv_path,
        "nameserver 10.0.2.3\n",
    )
    .map_err(|e| {
        anyhow!(
            "Failed to write {}: {}",
            resolv_path.display(),
            e
        )
    })?;

    Ok(())
}

fn prepare_dhcp(
    root: &str,
) -> Result<()> {
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
        let path =
            Path::new(root)
                .join(dir);

        fs::create_dir_all(&path)
            .map_err(|e| {
                anyhow!(
                    "Failed to create {}: {}",
                    path.display(),
                    e
                )
            })?;
    }

    /*
     * default.script comes from the rootfs overlay.
     * The installed disk may be older than the current
     * build, so it is synchronised from the initramfs
     * copy when missing or different.
     */
    let script =
        Path::new(root)
            .join("usr/share/udhcpc/default.script");

    let fallback =
        Path::new(INITRAMFS_UDHCPC_SCRIPT);

    let needs_copy =
        if !script.exists() {
            true
        } else if fallback.exists() {
            fs::read(fallback).ok()
                != fs::read(&script).ok()
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

        fs::copy(fallback, &script)
            .map_err(|e| {
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
    let mut permissions =
        fs::metadata(&script)?
            .permissions();

    permissions.set_mode(
        0o755
    );

    fs::set_permissions(
        &script,
        permissions,
    )
    .map_err(|e| {
        anyhow!(
            "Failed to chmod {}: {}",
            script.display(),
            e
        )
    })?;

    /*
     * /bin/sh is required by the script.
     */
    let sh =
        Path::new(root)
            .join("bin/sh");

    if !sh.exists() {
        return Err(anyhow!(
            "/bin/sh not found in target rootfs: {}",
            sh.display()
        ));
    }

    /*
     * default.script uses the `ip` command.
     */
    let has_ip = [
        "bin/ip",
        "sbin/ip",
        "usr/bin/ip",
        "usr/sbin/ip",
    ]
    .iter()
    .any(|path| {
        Path::new(root)
            .join(path)
            .exists()
    });

    if !has_ip {
        return Err(anyhow!(
            "`ip` applet not found in target rootfs"
        ));
    }

    Ok(())
}

pub fn test_network(
    root: &str,
) -> Result<()> {
    log_info!(
        "Testing network connection..."
    );

    let status =
        Command::new("chroot")
            .arg(root)
            .arg("ping")
            .args([
                "-c",
                "1",
                "-W",
                "5",
                "10.0.2.2",
            ])
            .status()
            .map_err(|e| {
                anyhow!(
                    "Failed to execute ping: {}",
                    e
                )
            })?;

    if !status.success() {
        return Err(anyhow!(
            "Network connection test failed"
        ));
    }

    log_info!(
        "Network connection is working."
    );

    Ok(())
}

pub fn configure_network(
    root: &str,
    interfaces: &[String],
) -> Result<()> {
    if interfaces.is_empty() {
        return Err(anyhow!(
            "No network interfaces found"
        ));
    }

    let config =
        Path::new(root)
            .join("etc/network/interfaces");

    if !config.exists() {
        return Err(anyhow!(
            "Network configuration not found: {}",
            config.display()
        ));
    }

    let ifup =
        Path::new(root)
            .join("sbin/ifup");

    if !ifup.exists() {
        return Err(anyhow!(
            "ifup not found in target rootfs: {}",
            ifup.display()
        ));
    }

    for interface in interfaces {
        let mode =
            get_interface_mode(
                root,
                interface,
            )?;

        log_info!(
            "Configuring {} using {}...",
            interface,
            mode
        );

        match mode.as_str() {
            "dhcp" | "static" => {
                let status =
                    Command::new("chroot")
                        .arg(root)
                        .arg("/sbin/ifup")
                        .arg(interface)
                        .status()
                        .map_err(|e| {
                            anyhow!(
                                "Failed to execute ifup for {}: {}",
                                interface,
                                e
                            )
                        })?;

                if !status.success() {
                    return Err(anyhow!(
                        "{} configuration failed for {}",
                        mode.to_uppercase(),
                        interface
                    ));
                }
            }

            _ => {
                return Err(anyhow!(
                    "Unsupported network mode '{}' for {}",
                    mode,
                    interface
                ));
            }
        }
    }

    Ok(())
}

pub fn init_network(root: &str) -> Result<()> {
    prepare_dhcp(root)?;

    let interfaces =
        detect_network_interfaces()?;


    let filtered =
        filter_network_interfaces(
            &interfaces,
        );

    if filtered.is_empty() {
        return Err(anyhow!(
            "No usable network interfaces found"
        ));
    }

    configure_network(
        root,
        &filtered,
    )?;

    let resolv =
        Path::new(root)
            .join("etc/resolv.conf");

    if !resolv.exists() {
        setup_resolv_conf(root)?;
    }

    test_network(root)?;

    log_info!(
        "Network initialized successfully."
    );

    Ok(())
}