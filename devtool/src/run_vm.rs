use std::fs;
use std::io::{self, Write};
use std::path::PathBuf;
use std::process::Command;

use anyhow::{bail, Result};

use crate::build_cmd::{self, BuildTarget};
use crate::common::*;

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
            _ => warn(format!("unknown VM option: {key}")),
        }
    }

    Ok(cfg)
}

pub fn run(paths: &Paths, new_vm: bool, debug: bool) -> Result<()> {
    let vm_config_path = paths.join("config/vm.config");
    let vm_dir = paths.join("build/vm");

    if !vm_config_path.is_file() {
        bail!("VM configuration not found:\n  {}", vm_config_path.display());
    }

    if new_vm {
        banner("Create New Senbit VM");
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
                info("Removing existing VM state...");
                let _ = fs::remove_dir_all(&vm_dir);
                info("Creating fresh VM directory...");
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
        Some(d) => paths.join(d),
        None => vm_dir.join("senbit.qcow2"),
    };

    let iso = paths.join("iso/senbit.iso");
    let debug_log = vm_dir.join("qemu.log");

    banner("Senbit Run");
    println!();

    // Construit Senbit (équivalent de l'appel à build.sh sans argument -> "all")
    build_cmd::run(paths, BuildTarget::All)?;

    if !iso.is_file() {
        bail!("Senbit ISO not found:\n  {}", iso.display());
    }

    fs::create_dir_all(disk.parent().unwrap())?;

    if !disk.is_file() {
        println!();
        info("Creating VM disk...");
        detail(format!("Path:   {}", disk.display()));
        detail(format!("Size:   {}", cfg.disk_size));
        detail(format!("Format: {}", cfg.disk_format));

        crate::common::run(Command::new("qemu-img").args([
            "create",
            "-f",
            &cfg.disk_format,
            &disk.to_string_lossy(),
            &cfg.disk_size,
        ]))?;
    }

    let mut network_args: Vec<String> = Vec::new();
    match cfg.network.as_str() {
        "user" => network_args.extend(["-nic".into(), "user".into()]),
        "none" => network_args.extend(["-nic".into(), "none".into()]),
        other => {
            error(format!("unsupported NETWORK value: {other}"));
            println!();
            println!("Supported values:");
            println!("  user");
            println!("  none");
            std::process::exit(1);
        }
    }

    if debug {
        println!();
        info("Debug mode enabled");
        detail(format!("QEMU log: {}", debug_log.display()));
    }

    banner("Starting Senbit VM");
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
        cmd.args(["-serial", &format!("file:{}", debug_log.display())]);
    } else {
        cmd.args(["-serial", "mon:stdio"]);
    }

    let status = cmd.status()?;
    if !status.success() {
        std::process::exit(status.code().unwrap_or(1));
    }

    Ok(())
}
