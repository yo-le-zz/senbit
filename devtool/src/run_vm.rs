//! `devtool run` — équivalent natif de run.sh (construit Senbit puis lance QEMU).

use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{bail, Result};

use crate::build::{self, Target};
use crate::paths::Paths;
use crate::ui;

/// Microgiciels OVMF couramment installés par les paquets `ovmf`/`qemu-efi`
/// des distributions Debian/Ubuntu.
const OVMF_CODE_CANDIDATES: &[&str] = &[
    "/usr/share/OVMF/OVMF_CODE.fd",
    "/usr/share/OVMF/OVMF_CODE_4M.fd",
    "/usr/share/ovmf/OVMF_CODE.fd",
    "/usr/share/ovmf/OVMF_CODE_4M.fd",
];

const OVMF_VARS_CANDIDATES: &[&str] = &[
    "/usr/share/OVMF/OVMF_VARS.fd",
    "/usr/share/OVMF/OVMF_VARS_4M.fd",
    "/usr/share/ovmf/OVMF_VARS.fd",
    "/usr/share/ovmf/OVMF_VARS_4M.fd",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BootMode {
    Bios,
    Uefi,
}

#[derive(Debug)]
struct VmConfig {
    ram: String,
    cpus: String,
    disk: Option<String>,
    disk_size: String,
    disk_format: String,
    network: String,
    boot: BootMode,
}

impl VmConfig {
    fn defaults() -> Self {
        Self {
            ram: "512M".into(),
            cpus: "2".into(),
            disk: None,
            disk_size: "10G".into(),
            disk_format: "qcow2".into(),
            network: "user".into(),
            boot: BootMode::Bios,
        }
    }
}

fn load_config(path: &Path) -> Result<VmConfig> {
    let mut cfg = VmConfig::defaults();
    let content = fs::read_to_string(path)?;

    for line in content.lines() {
        let trimmed = line.trim();

        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        let Some((key, value)) = trimmed.split_once('=') else {
            continue;
        };

        let key = key.trim();
        let value = value.trim().to_string();

        match key {
            "RAM" => cfg.ram = value,
            "CPUS" => cfg.cpus = value,
            "DISK" => cfg.disk = Some(value),
            "DISK_SIZE" => cfg.disk_size = value,
            "DISK_FORMAT" => cfg.disk_format = value,
            "NETWORK" => cfg.network = value,

            // Grub est déjà construit en image hybride BIOS + UEFI.
            // Ce réglage choisit uniquement le firmware utilisé par QEMU.
            "BOOT" => {
                cfg.boot = match value.to_lowercase().as_str() {
                    "uefi" | "efi" => BootMode::Uefi,

                    "bios" | "legacy" => BootMode::Bios,

                    other => bail!(
                        "unsupported BOOT value: {other}\n\n\
                         Supported values:\n\
                         \u{20} bios (default)\n\
                         \u{20} uefi"
                    ),
                }
            }

            other => {
                ui::warn(format!(
                    "unknown VM option: {other}"
                ));
            }
        }
    }

    Ok(cfg)
}

fn find_ovmf_firmware() -> Result<(&'static str, &'static str)> {
    let code = OVMF_CODE_CANDIDATES
        .iter()
        .find(|path| Path::new(path).is_file())
        .copied();

    let vars = OVMF_VARS_CANDIDATES
        .iter()
        .find(|path| Path::new(path).is_file())
        .copied();

    match (code, vars) {
        (Some(code), Some(vars)) => Ok((code, vars)),

        (None, _) => {
            bail!(
                "BOOT=uefi requires OVMF CODE firmware, none found among:\n{}\n\n\
                 Install it with:\n\
                 \u{20} sudo apt-get install ovmf",
                OVMF_CODE_CANDIDATES
                    .iter()
                    .map(|p| format!("  {p}"))
                    .collect::<Vec<_>>()
                    .join("\n")
            );
        }

        (_, None) => {
            bail!(
                "BOOT=uefi requires OVMF VARS firmware, none found among:\n{}\n\n\
                 Install it with:\n\
                 \u{20} sudo apt-get install ovmf",
                OVMF_VARS_CANDIDATES
                    .iter()
                    .map(|p| format!("  {p}"))
                    .collect::<Vec<_>>()
                    .join("\n")
            );
        }
    }
}

pub fn run(
    p: &Paths,
    new_vm: bool,
    debug: bool,
    fast: bool,
    force: bool,
    jobs: Option<usize>,
) -> Result<()> {
    let vm_config_path = p.vm_config();
    let vm_dir = p.vm_dir();

    if !vm_config_path.is_file() {
        bail!(
            "VM configuration not found:\n  {}",
            vm_config_path.display()
        );
    }

    if new_vm {
        ui::banner("Create New Senbit VM");
        println!();

        println!(
            "This will delete the entire VM directory:"
        );

        println!(
            "  {}",
            vm_dir.display()
        );

        println!();

        println!(
            "All VM state stored there will be lost."
        );

        println!();

        print!("Continue? [y/N] ");
        io::stdout().flush()?;

        let mut answer = String::new();
        io::stdin().read_line(&mut answer)?;

        match answer.trim().to_lowercase().as_str() {
            "y" | "yes" => {
                println!();

                ui::info(
                    "Removing existing VM state..."
                );

                let _ =
                    fs::remove_dir_all(&vm_dir);

                ui::info(
                    "Creating fresh VM directory..."
                );

                fs::create_dir_all(&vm_dir)?;
            }

            _ => {
                println!();
                println!("Cancelled.");
                return Ok(());
            }
        }
    }

    let cfg =
        load_config(&vm_config_path)?;

    let disk: PathBuf =
        match &cfg.disk {
            Some(d) if d.starts_with('/') =>
                PathBuf::from(d),

            Some(d) =>
                p.r(d),

            None =>
                vm_dir.join("senbit.qcow2"),
        };

    let iso = p.iso_dist();

    let debug_log =
        vm_dir.join("qemu.log");

    ui::banner("Senbit Run");
    println!();

    let target =
        if fast {
            Target::Fast
        } else {
            Target::All
        };

    build::run(
        p,
        target,
        jobs,
        force,
    )?;

    if !iso.is_file() {
        bail!(
            "Senbit ISO not found:\n  {}",
            iso.display()
        );
    }

    fs::create_dir_all(
        disk.parent().unwrap()
    )?;

    if !disk.is_file() {
        println!();

        ui::info(
            "Creating VM disk..."
        );

        ui::detail(format!(
            "Path:   {}",
            disk.display()
        ));

        ui::detail(format!(
            "Size:   {}",
            cfg.disk_size
        ));

        ui::detail(format!(
            "Format: {}",
            cfg.disk_format
        ));

        crate::proc::run(
            Command::new("qemu-img").args([
                "create",
                "-f",
                &cfg.disk_format,
                &disk.to_string_lossy(),
                &cfg.disk_size,
            ])
        )?;
    }

    let mut network_args:
        Vec<String> = Vec::new();

    match cfg.network.as_str() {
        "user" => {
            network_args.extend([
                "-nic".into(),
                "user".into(),
            ]);
        }

        "none" => {
            network_args.extend([
                "-nic".into(),
                "none".into(),
            ]);
        }

        other => {
            bail!(
                "unsupported NETWORK value: {other}\n\n\
                 Supported values:\n\
                 \u{20} user\n\
                 \u{20} none"
            );
        }
    }

    let ovmf =
        if cfg.boot == BootMode::Uefi {
            Some(find_ovmf_firmware()?)
        } else {
            None
        };

    let ovmf_vars =
        if let Some((_, vars_template)) = ovmf {
            let vars_path =
                vm_dir.join("OVMF_VARS.fd");

            if !vars_path.is_file() {
                fs::copy(
                    vars_template,
                    &vars_path,
                )?;
            }

            Some(vars_path)
        } else {
            None
        };

    if debug {
        println!();

        ui::info(
            "Debug mode enabled"
        );

        ui::detail(format!(
            "QEMU log: {}",
            debug_log.display()
        ));
    }

    ui::banner(
        "Starting Senbit VM"
    );

    println!();

    println!(
        "RAM:     {}",
        cfg.ram
    );

    println!(
        "CPUs:    {}",
        cfg.cpus
    );

    println!(
        "Disk:    {}",
        disk.display()
    );

    println!(
        "ISO:     {}",
        iso.display()
    );

    println!(
        "Network: {}",
        cfg.network
    );

    println!(
        "Boot:    {}",
        match cfg.boot {
            BootMode::Bios =>
                "bios (legacy)",

            BootMode::Uefi =>
                "uefi",
        }
    );

    if let Some((code, _)) = ovmf {
        println!(
            "Firmware: {}",
            code
        );
    }

    if let Some(vars) = &ovmf_vars {
        println!(
            "NVRAM:   {}",
            vars.display()
        );
    }

    if debug {
        println!(
            "Debug:   enabled"
        );

        println!(
            "Log:     {}",
            debug_log.display()
        );
    }

    println!();

    let mut cmd =
        Command::new(
            "qemu-system-x86_64"
        );

    cmd.arg("-enable-kvm")
        .args([
            "-m",
            &cfg.ram,
        ])
        .args([
            "-smp",
            &cfg.cpus,
        ])
        .args([
            "-drive",
            &format!(
                "file={},format={}",
                disk.display(),
                cfg.disk_format
            ),
        ])
        .args([
            "-cdrom",
            &iso.to_string_lossy(),
        ])
        .args(&network_args);

    // OVMF CODE: firmware en lecture seule.
    if let Some((code, _)) = ovmf {
        cmd.args([
            "-drive",
            &format!(
                "if=pflash,format=raw,readonly=on,file={}",
                code
            ),
        ]);
    }

    // OVMF VARS: mémoire UEFI persistante et modifiable.
    if let Some(vars) = &ovmf_vars {
        cmd.args([
            "-drive",
            &format!(
                "if=pflash,format=raw,file={}",
                vars.display()
            ),
        ]);
    }

    if debug {
        fs::create_dir_all(
            &vm_dir
        )?;

        fs::write(
            &debug_log,
            b"",
        )?;

        cmd.args([
            "-serial",
            &format!(
                "file:{}",
                debug_log.display()
            ),
        ]);
    }

    let status =
        cmd.status()?;

    if !status.success() {
        std::process::exit(
            status.code().unwrap_or(1)
        );
    }

    Ok(())
}
