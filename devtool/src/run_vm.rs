//! `devtool run` — équivalent natif de run.sh (construit Senbit puis lance QEMU).

use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use anyhow::{bail, Result};

use crate::build::{self, Target};
use crate::host;
use crate::ovmf::{self, Nvram};
use crate::paths::Paths;
use crate::ui;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BootMode {
    Bios,
    Uefi,
}

#[derive(Debug)]
struct VmConfig {
    ram: String,
    cpus: String,
    /// QEMU CPU model (`-cpu`). Default: host (see CPU in config/vm.config).
    cpu: String,
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
            // QEMU's default model (qemu64) only has the x86-64 baseline: programs
            // built for x86-64-v2/v3 (kite, many Rust/Go/LLVM binaries) stop with
            // "CPU ISA level is lower than required". `host` gives the VM the
            // instructions of the real CPU (KVM).
            cpu: "host".into(),
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
            "CPU" => cfg.cpu = value,
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
                ui::warn(format!("unknown VM option: {other}"));
            }
        }
    }

    Ok(cfg)
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

    let target = if fast { Target::Fast } else { Target::All };

    build::run(p, target, jobs, force)?;

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

        crate::proc::run(host::command("qemu-img").args([
            "create",
            "-f",
            &cfg.disk_format,
            &disk.to_string_lossy(),
            &cfg.disk_size,
        ]))?;
    }

    let mut network_args: Vec<String> = Vec::new();

    match cfg.network.as_str() {
        "user" => {
            network_args.extend(["-nic".into(), "user".into()]);
        }

        "none" => {
            network_args.extend(["-nic".into(), "none".into()]);
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

    // Firmware UEFI : couple CODE/VARS COHÉRENT (voir ovmf.rs). Un couple
    // incohérent ou une NVRAM obsolète empêche OVMF de démarrer sans aucune
    // erreur : log série vide + « Guest has not initialized the display ».
    let ovmf = if cfg.boot == BootMode::Uefi {
        Some(ovmf::find_firmware()?)
    } else {
        None
    };

    let mut nvram_note: Option<String> = None;

    let ovmf_vars = if let Some(fw) = &ovmf {
        let vars_path = vm_dir.join("OVMF_VARS.fd");

        match ovmf::prepare_nvram(&vars_path, &fw.vars_template)? {
            Nvram::Created => nvram_note = Some("created from the firmware template".into()),
            Nvram::Reused => nvram_note = Some("existing, compatible with the firmware".into()),
            Nvram::Refreshed(reason) => {
                ui::warn(format!(
                    "The VM's UEFI NVRAM was incompatible with the firmware and has been recreated:\n  {reason}\n  \
                     (the old file was kept as {}.stale)",
                    vars_path.display()
                ));
                nvram_note = Some("recreated (the previous one was incompatible)".into());
            }
        }

        Some(vars_path)
    } else {
        None
    };

    // L'ISO doit porter une entrée de démarrage EFI, sinon OVMF ne peut pas la
    // lancer (une ISO « amorçable » peut très bien n'être amorçable qu'en BIOS).
    let iso_boot = if cfg.boot == BootMode::Uefi {
        match ovmf::inspect_iso(&iso) {
            Ok(b) => {
                if !b.efi {
                    ui::warn(format!(
                        "The ISO has no EFI boot entry (El Torito): OVMF cannot boot it.\n  {}\n  \
                         Rebuild it with GRUB x86_64-efi available (Ubuntu: grub-efi-amd64-bin and mtools; \
                         `devtool install` installs them), or set BOOT=bios in config/vm.config.",
                        iso.display()
                    ));
                }
                Some(b)
            }
            Err(e) => {
                ui::warn(format!("Could not inspect the ISO boot catalog: {e:#}"));
                None
            }
        }
    } else {
        None
    };

    if debug {
        println!();

        ui::info("Debug mode enabled");

        ui::detail(format!("QEMU log: {}", debug_log.display()));
    }

    ui::banner("Starting Senbit VM");

    println!();

    println!("RAM:     {}", cfg.ram);

    println!("CPUs:    {} ({})", cfg.cpus, cfg.cpu);

    println!("Disk:    {}", disk.display());

    println!("ISO:     {}", iso.display());

    println!("Network: {}", cfg.network);

    println!(
        "Boot:    {}",
        match cfg.boot {
            BootMode::Bios => "bios (legacy)",

            BootMode::Uefi => "uefi",
        }
    );

    if let Some(fw) = &ovmf {
        println!("Firmware: {}", fw.code.display());
    }

    if let Some(vars) = &ovmf_vars {
        println!("NVRAM:   {}", vars.display());
    }

    if debug {
        print_uefi_diagnostics(ovmf.as_ref(), ovmf_vars.as_deref(), nvram_note.as_deref(), iso_boot.as_ref());
    }

    if debug {
        println!("Debug:   enabled");

        println!("Log:     {}", debug_log.display());
    }

    println!();

    let mut cmd = host::command("qemu-system-x86_64");

    cmd.arg("-enable-kvm")
        .args(["-m", &cfg.ram])
        .args(["-smp", &cfg.cpus])
        .args(["-cpu", &cfg.cpu])
        .args([
            "-drive",
            &format!("file={},format={}", disk.display(), cfg.disk_format),
        ])
        .args(["-cdrom", &iso.to_string_lossy()])
        .args(&network_args);

    // OVMF CODE: firmware en lecture seule.
    if let Some(fw) = &ovmf {
        cmd.args([
            "-drive",
            &format!("if=pflash,format=raw,readonly=on,file={}", fw.code.display()),
        ]);
    }

    // OVMF VARS: mémoire UEFI persistante et modifiable.
    if let Some(vars) = &ovmf_vars {
        cmd.args([
            "-drive",
            &format!("if=pflash,format=raw,file={}", vars.display()),
        ]);
    }

    if debug {
        fs::create_dir_all(&vm_dir)?;

        fs::write(&debug_log, b"")?;

        cmd.args(["-serial", &format!("file:{}", debug_log.display())]);

        // Debug flag for the guest (read by senbit-init through
        // /sys/firmware/qemu_fw_cfg): the whole boot is mirrored to the kernel
        // log, hence to the serial console captured in the log above.
        cmd.args(["-fw_cfg", "name=opt/senbit/debug,string=1"]);

        // Sorties de diagnostic de QEMU lui-même et du firmware OVMF (port
        // 0x402), à côté du log série : utiles quand le log série est vide.
        let qemu_log = vm_dir.join("qemu-internal.log");
        let ovmf_log = vm_dir.join("ovmf-debugcon.log");
        cmd.args(["-d", "guest_errors,unimp", "-D", &qemu_log.to_string_lossy()]);
        if ovmf.is_some() {
            cmd.args([
                "-debugcon",
                &format!("file:{}", ovmf_log.display()),
                "-global",
                "isa-debugcon.iobase=0x402",
            ]);
        }

        let cmdline = format!(
            "{} {}\n",
            cmd.get_program().to_string_lossy(),
            cmd.get_args()
                .map(|a| shell_quote(&a.to_string_lossy()))
                .collect::<Vec<_>>()
                .join(" ")
        );
        fs::write(vm_dir.join("qemu-cmdline.txt"), &cmdline)?;
        ui::detail(format!("QEMU command: {}", vm_dir.join("qemu-cmdline.txt").display()));
    }

    let status = cmd.status()?;

    if debug {
        print_debug_summary(&vm_dir, &debug_log, ovmf.is_some());
    }

    if !status.success() {
        std::process::exit(status.code().unwrap_or(1));
    }

    Ok(())
}

/// Entoure d'apostrophes un argument s'il contient autre chose que des
/// caractères « sûrs » (pour la ligne de commande écrite en mode debug).
fn shell_quote(arg: &str) -> String {
    let safe = !arg.is_empty()
        && arg
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "-_./=:,+@%".contains(c));
    if safe {
        arg.to_string()
    } else {
        format!("'{}'", arg.replace('\'', "'\\''"))
    }
}

fn file_size(path: &Path) -> String {
    match fs::metadata(path) {
        Ok(m) => format!("{} bytes", m.len()),
        Err(_) => "missing".into(),
    }
}

/// Mode debug : ce qui permet de distinguer « le firmware ne démarre pas »
/// de « GRUB / le noyau plantent » avant même de lancer QEMU.
fn print_uefi_diagnostics(
    fw: Option<&ovmf::Firmware>,
    nvram: Option<&Path>,
    nvram_note: Option<&str>,
    iso: Option<&ovmf::IsoBoot>,
) {
    println!();
    ui::info("UEFI diagnostics");

    match fw {
        None => ui::detail("Boot mode is BIOS: no UEFI firmware involved"),
        Some(fw) => {
            ui::detail(format!("Firmware source: {}", fw.describe_source()));
            ui::detail(format!("OVMF CODE:  {} ({})", fw.code.display(), file_size(&fw.code)));
            ui::detail(format!(
                "OVMF VARS:  {} ({}) [template]",
                fw.vars_template.display(),
                file_size(&fw.vars_template)
            ));
            if let Some(vars) = nvram {
                ui::detail(format!(
                    "NVRAM:      {} ({}) - {}",
                    vars.display(),
                    file_size(vars),
                    nvram_note.unwrap_or("unknown state")
                ));
            }
        }
    }

    if let Some(b) = iso {
        ui::detail(format!(
            "ISO boot entries: BIOS={} EFI={}",
            if b.bios { "yes" } else { "no" },
            if b.efi { "yes" } else { "no" }
        ));
    }

    match fs::OpenOptions::new().read(true).write(true).open("/dev/kvm") {
        Ok(_) => ui::detail("/dev/kvm: accessible"),
        Err(e) => ui::warn(format!(
            "/dev/kvm: {e} (QEMU is started with -enable-kvm and will refuse to run; \
             add your user to the `kvm` group and log in again)"
        )),
    }
    println!();
}

/// Après QEMU, en mode debug : interprète les logs (surtout le cas « vide »).
fn print_debug_summary(vm_dir: &Path, serial_log: &Path, uefi: bool) {
    println!();
    ui::info("Debug summary");
    let serial = fs::metadata(serial_log).map(|m| m.len()).unwrap_or(0);
    ui::detail(format!("Serial log: {} ({serial} bytes)", serial_log.display()));
    if uefi {
        ui::detail(format!(
            "OVMF debug: {} ({})",
            vm_dir.join("ovmf-debugcon.log").display(),
            file_size(&vm_dir.join("ovmf-debugcon.log"))
        ));
    }
    ui::detail(format!(
        "QEMU log:   {} ({})",
        vm_dir.join("qemu-internal.log").display(),
        file_size(&vm_dir.join("qemu-internal.log"))
    ));
    if serial == 0 && uefi {
        ui::warn(
            "The serial log is empty. If the QEMU window said \"Guest has not initialized the display (yet)\", \
             the UEFI firmware itself never started (GRUB and the kernel were never reached): \
             check the firmware/NVRAM lines of the UEFI diagnostics above. \
             If the window showed a UEFI shell or boot menu, the ISO has no EFI boot entry.",
        );
    }
}
