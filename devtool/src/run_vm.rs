//! `devtool run` — équivalent natif de run.sh (construit Senbit puis lance QEMU).

use std::fs;
use std::io::{self, Write};
use std::path::PathBuf;
use std::process::Command;

use anyhow::{bail, Result};

use crate::build::{self, Target};
use crate::paths::Paths;
use crate::ui;

#[derive(Debug, Default)]
struct VmConfig {
    ram: String,
    cpus: String,
    disk: Option<String>,
    disk_size: String,
    disk_format: String,
    network: String,
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
        }
    }
}

fn load_config(path: &std::path::Path) -> Result<VmConfig> {
    let mut cfg = VmConfig::defaults();
    let content = fs::read_to_string(path)?;
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let Some((key, value)) = trimmed.split_once('=') else { continue };
        let key = key.trim();
        let value = value.trim().to_string();
        match key {
            "RAM" => cfg.ram = value,
            "CPUS" => cfg.cpus = value,
            "DISK" => cfg.disk = Some(value),
            "DISK_SIZE" => cfg.disk_size = value,
            "DISK_FORMAT" => cfg.disk_format = value,
            "NETWORK" => cfg.network = value,
            other => ui::warn(format!("unknown VM option: {other}")),
        }
    }
    Ok(cfg)
}

pub fn run(p: &Paths, new_vm: bool, debug: bool) -> Result<()> {
    let vm_config_path = p.vm_config();
    let vm_dir = p.vm_dir();

    if !vm_config_path.is_file() {
        bail!("VM configuration not found:\n  {}", vm_config_path.display());
    }

    if new_vm {
        ui::banner("Create New Senbit VM");
        println!();
        println!("This will delete the entire VM directory:");
        println!("  {}", vm_dir.display());
        println!();
        println!("All VM state stored there will be lost.");
        println!();
        print!("Continue? [y/N] ");
        io::stdout().flush()?;
        let mut answer = String::new();
        io::stdin().read_line(&mut answer)?;

        match answer.trim().to_lowercase().as_str() {
            "y" | "yes" => {
                println!();
                ui::info("Removing existing VM state...");
                let _ = fs::remove_dir_all(&vm_dir);
                ui::info("Creating fresh VM directory...");
                fs::create_dir_all(&vm_dir)?;
            }
            _ => {
                println!();
                println!("Cancelled.");
                return Ok(());
            }
        }
    }

    let cfg = load_config(&vm_config_path)?;
    let disk: PathBuf = match &cfg.disk {
        Some(d) if d.starts_with('/') => PathBuf::from(d),
        Some(d) => p.r(d),
        None => vm_dir.join("senbit.qcow2"),
    };

    let iso = p.iso_dist();
    let debug_log = vm_dir.join("qemu.log");

    ui::banner("Senbit Run");
    println!();

    build::run(p, Target::All)?;

    if !iso.is_file() {
        bail!("Senbit ISO not found:\n  {}", iso.display());
    }

    fs::create_dir_all(disk.parent().unwrap())?;
    if !disk.is_file() {
        println!();
        ui::info("Creating VM disk...");
        ui::detail(format!("Path:   {}", disk.display()));
        ui::detail(format!("Size:   {}", cfg.disk_size));
        ui::detail(format!("Format: {}", cfg.disk_format));
        crate::proc::run(Command::new("qemu-img").args([
            "create", "-f", &cfg.disk_format, &disk.to_string_lossy(), &cfg.disk_size,
        ]))?;
    }

    let mut network_args: Vec<String> = Vec::new();
    match cfg.network.as_str() {
        "user" => network_args.extend(["-nic".into(), "user".into()]),
        "none" => network_args.extend(["-nic".into(), "none".into()]),
        other => bail!("unsupported NETWORK value: {other}\n\nSupported values:\n  user\n  none"),
    }

    if debug {
        println!();
        ui::info("Debug mode enabled");
        ui::detail(format!("QEMU log: {}", debug_log.display()));
    }

    ui::banner("Starting Senbit VM");
    println!();
    println!("RAM:     {}", cfg.ram);
    println!("CPUs:    {}", cfg.cpus);
    println!("Disk:    {}", disk.display());
    println!("ISO:     {}", iso.display());
    println!("Network: {}", cfg.network);
    if debug {
        println!("Debug:   enabled");
        println!("Log:     {}", debug_log.display());
    }
    println!();

    let mut cmd = Command::new("qemu-system-x86_64");
    cmd.arg("-enable-kvm")
        .args(["-m", &cfg.ram])
        .args(["-smp", &cfg.cpus])
        .args(["-drive", &format!("file={},format={}", disk.display(), cfg.disk_format)])
        .args(["-cdrom", &iso.to_string_lossy()])
        .args(&network_args);

    if debug {
        fs::create_dir_all(&vm_dir)?;
        fs::write(&debug_log, b"")?;
        cmd.args(["-serial", &format!("file:{}", debug_log.display())]);
    }

    let status = cmd.status()?;
    if !status.success() {
        std::process::exit(status.code().unwrap_or(1));
    }
    Ok(())
}
