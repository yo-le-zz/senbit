//! Installation de Senbit.
//!
//! Déroulement :
//!
//!   1. `InstallPlan::collect` : TOUTES les questions (clavier, disque, comptes,
//!      hostname, langue, fuseau, réseau). Rien n'est écrit sur le disque.
//!   2. Récapitulatif, puis confirmation FINALE (par défaut : annuler). On peut
//!      annuler ou tout recommencer, sans que le disque ait été touché.
//!   3. `apply` : partitionnement, formatage, copie, comptes, bootloader...
//!      Chaque étape est affichée au fur et à mesure (« [ 3/14] ... »).

use colored::Colorize;

use std::path::Path;
use std::time::Instant;

use crate::log_info;

use crate::installation::bootloader::bootloader::{BootMode, setup_bootloader};
use crate::installation::hostname::write_hostname;
use crate::installation::local::local::apply_locale;
use crate::installation::local::timezone::apply_timezone;
use crate::installation::network::network::apply_network;
use crate::installation::partition::{
    create_legacy_partitions, create_uefi_partitions, format_efi_partition, format_partition,
    manual_partitioning,
};
use crate::installation::plan::{Decision, InstallPlan, Layout, ask_decision};
use crate::installation::rootfs::setup_rootfs;
use crate::installation::users::perms::set_sys_perms;
use crate::installation::users::user::setup_users;

use crate::filesystem::disk::get_partition_uuid;
use crate::filesystem::fs::{mount, unmount};

/// Résultat de l'installation (l'annulation n'est pas une erreur).
pub enum Outcome {
    Installed,
    Cancelled,
}

/// Affichage des étapes : « [ 3/14] Formatting the root partition... ».
struct Steps {
    total: usize,
    current: usize,
    started: Instant,
    step_started: Instant,
}

impl Steps {
    fn new(total: usize) -> Self {
        Steps {
            total,
            current: 0,
            started: Instant::now(),
            step_started: Instant::now(),
        }
    }

    fn begin(&mut self, label: &str) {
        self.current += 1;
        self.step_started = Instant::now();
        println!(
            "{} {}",
            format!("[{:>2}/{}]", self.current, self.total).cyan().bold(),
            label.bold()
        );
    }

    fn done(&self) {
        println!(
            "       {} {}",
            "done".green().bold(),
            format!("({:.1}s)", self.step_started.elapsed().as_secs_f32()).dimmed()
        );
    }

    fn finish(&self) {
        println!(
            "\n{} {}",
            "Installation finished in".green().bold(),
            format!("{:.0}s.", self.started.elapsed().as_secs_f32()).green().bold()
        );
    }
}

/// Pose les questions, demande la confirmation finale, puis installe.
pub fn install_system() -> Result<Outcome, String> {
    loop {
        let plan = InstallPlan::collect()?;

        plan.print_summary();

        match ask_decision(&plan)? {
            Decision::Install => {
                apply(&plan)?;
                return Ok(Outcome::Installed);
            }
            Decision::StartOver => {
                println!("{}", "Starting over...".yellow());
                continue;
            }
            Decision::Cancel => return Ok(Outcome::Cancelled),
        }
    }
}

/// Applique le plan : à partir d'ici le disque est modifié.
fn apply(plan: &InstallPlan) -> Result<(), String> {
    let mut steps = Steps::new(plan.total_steps());
    let device = plan.target.device.as_str();

    println!(
        "\n{}",
        format!("Installing Senbit on {device}...").cyan().bold()
    );

    // ========================================================
    // Partitioning
    // ========================================================

    let (partition_device, efi_partition) = match plan.layout {
        Layout::EntireDisk => {
            steps.begin(&format!("Creating the partitions on {device}"));
            let created = match plan.boot_mode {
                BootMode::Efi => {
                    let (efi, root) = create_uefi_partitions(device)
                        .map_err(|e| format!("Partitioning failed: {}", e))?;
                    (root, Some(efi))
                }
                BootMode::Legacy => {
                    let root = create_legacy_partitions(device)
                        .map_err(|e| format!("Partitioning failed: {}", e))?;
                    (root, None)
                }
            };
            steps.done();
            created
        }
        Layout::Manual => {
            steps.begin(&format!("Manual partitioning of {device} (fdisk)"));
            let root = manual_partitioning(device)
                .map_err(|e| format!("Manual partitioning failed: {}", e))?;
            steps.done();
            (root, None)
        }
        Layout::ExistingPartition => (device.to_string(), None),
    };

    // ========================================================
    // Formatting
    //
    //   UEFI   : ESP -> FAT32, root -> ext4
    //   Legacy : BIOS Boot -> no filesystem, root -> ext4
    // ========================================================

    if plan.boot_mode == BootMode::Efi {
        if let Some(efi) = &efi_partition {
            steps.begin(&format!("Formatting the EFI partition {efi}"));
            format_efi_partition(efi).map_err(|e| format!("EFI format failed: {}", e))?;
            steps.done();
        }
    }

    steps.begin(&format!("Formatting the root partition {partition_device} (ext4)"));
    format_partition(&partition_device).map_err(|e| format!("Format failed: {}", e))?;
    steps.done();

    // ========================================================
    // Mount root
    // ========================================================

    let mount_point = Path::new(crate::installation::INSTALL_ROOT);

    steps.begin(&format!("Mounting {partition_device} on {}", mount_point.display()));
    mount(&partition_device, crate::installation::INSTALL_ROOT, None)
        .map_err(|e| format!("Failed to mount: {}", e))?;
    steps.done();

    steps.begin("Reading the root partition UUID");
    let root_uuid = get_partition_uuid(&partition_device)
        .map_err(|e| format!("Failed to get partition UUID: {}", e))?;
    steps.done();

    // ========================================================
    // System files
    // ========================================================

    steps.begin("Copying the system files (this takes a while)");
    setup_rootfs(
        // install the installation file
        Path::new("/"),
        mount_point,
        &root_uuid,
    )
    .map_err(|e| format!("Failed to setup rootfs: {}", e))?;
    steps.done();

    // ========================================================
    // Configuration (answers collected before the confirmation)
    // ========================================================

    steps.begin(&format!("Creating the accounts (root and {})", plan.accounts.username));
    setup_users(mount_point, &plan.accounts)?;
    steps.done();

    steps.begin(&format!("Setting the hostname ({})", plan.hostname));
    write_hostname(mount_point, &plan.hostname)
        .map_err(|e| format!("Failed to set hostname: {}", e))?;
    steps.done();

    steps.begin(&format!("Setting the language ({})", plan.locale));
    apply_locale(mount_point, &plan.locale)
        .map_err(|e| format!("Failed to setup locale: {}", e))?;
    steps.done();

    steps.begin(&format!("Setting the timezone ({})", plan.timezone));
    apply_timezone(mount_point, &plan.timezone)
        .map_err(|e| format!("Failed to setup timezone: {}", e))?;
    steps.done();

    steps.begin(&format!("Configuring the network ({})", plan.network.summary()));
    apply_network(mount_point, &plan.network)
        .map_err(|e| format!("Failed to setup network: {}", e))?;
    steps.done();

    // ========================================================
    // Bootloader and permissions
    // ========================================================

    steps.begin("Installing the bootloader");
    setup_bootloader(mount_point, Path::new(device), &root_uuid)
        .map_err(|e| format!("Failed to install bootloader: {}", e))?;
    steps.done();

    steps.begin("Setting the system permissions");
    set_sys_perms(mount_point).map_err(|e| format!("Failed to set sys perms: {}", e))?;
    steps.done();

    // ========================================================
    // Unmount and check
    // ========================================================

    steps.begin("Writing everything to the disk, unmounting and checking the filesystem");

    // Make sure everything is on the disk before the unmount.
    let _ = std::process::Command::new("sync").status();

    unmount(crate::installation::INSTALL_ROOT)
        .map_err(|e| format!("Failed to unmount: {}", e))?;

    // Check the new filesystem (best effort: e2fsck may not be installed).
    // Exit codes 0 and 1 (= errors corrected) are fine.
    match std::process::Command::new("e2fsck")
        .args(["-f", "-y", &partition_device])
        .status()
    {
        Ok(status) => log_info!("e2fsck finished: {}", status),
        Err(e) => log_info!("e2fsck not run: {}", e),
    }
    steps.done();

    steps.finish();
    Ok(())
}
