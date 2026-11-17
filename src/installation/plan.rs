//! Plan d'installation : TOUTES les questions d'abord, l'installation ensuite.
//!
//! `InstallPlan::collect` pose les questions et garde les réponses en mémoire :
//! rien n'est écrit sur le disque. `print_summary` affiche le récapitulatif,
//! `ask_decision` demande la confirmation FINALE (par défaut : annuler). Ce n'est
//! qu'après elle que `install::apply` partitionne, formate et copie, en
//! affichant les étapes au fur et à mesure.
//!
//! Annuler ou recommencer avant la confirmation ne modifie jamais le disque.
//! (Seule exception : la disposition du clavier, chargée dans la session live
//! pour pouvoir taper les mots de passe.)

use colored::Colorize;
use inquire::{Confirm, Select};

use crate::installation::bootloader::bootloader::{BootMode, detect_boot_mode};
use crate::installation::hostname::get_hostname;
use crate::installation::local::lang::setup_lang;
use crate::installation::local::local::ask_locale;
use crate::installation::local::timezone::ask_timezone;
use crate::installation::network::network::{NetworkPlan, ask_network};
use crate::installation::partition::{DiskKind, DiskOption, select_disk_interactive};
use crate::installation::users::questions::{AccountsPlan, ask_accounts};
use crate::log_info;

/// Comment le disque choisi sera préparé.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Layout {
    /// Disque entier : effacé, partitions créées automatiquement.
    EntireDisk,
    /// fdisk est lancé sur le disque, puis on choisit la partition.
    Manual,
    /// Partition existante : seulement formatée.
    ExistingPartition,
}

pub struct InstallPlan {
    pub keyboard: String,
    pub boot_mode: BootMode,
    pub target: DiskOption,
    pub layout: Layout,
    pub accounts: AccountsPlan,
    pub hostname: String,
    pub locale: String,
    pub timezone: String,
    pub network: NetworkPlan,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Decision {
    Install,
    StartOver,
    Cancel,
}

/// Nombre d'étapes de l'installation (affiché « [3/14] »).
pub fn count_steps(layout: Layout, boot_mode: BootMode) -> usize {
    // formatage root, montage, UUID, rootfs, comptes, hostname, langue,
    // fuseau, réseau, bootloader, permissions, fin (sync + démontage + fsck)
    let mut steps = 12;

    if layout != Layout::ExistingPartition {
        steps += 1; // partitionnement
    }
    if layout == Layout::EntireDisk && boot_mode == BootMode::Efi {
        steps += 1; // formatage de l'ESP
    }
    steps
}

fn layout_description(layout: Layout, boot_mode: BootMode, device: &str) -> String {
    match layout {
        Layout::EntireDisk => match boot_mode {
            BootMode::Efi => format!(
                "ERASE ALL of {device}, create an EFI partition + a root partition"
            ),
            BootMode::Legacy => format!(
                "ERASE ALL of {device}, create a BIOS Boot partition + a root partition"
            ),
        },
        Layout::Manual => format!(
            "fdisk will be started on {device} (nothing is written until you press 'w'), \
             then you pick the root partition, which is formatted"
        ),
        Layout::ExistingPartition => format!("ERASE ALL data of the partition {device} (formatted as ext4)"),
    }
}

impl InstallPlan {
    /// Poses toutes les questions. N'écrit rien sur le disque.
    pub fn collect() -> Result<InstallPlan, String> {
        // Keyboard: loaded in the live session so that the passwords are typed
        // with the right layout.
        let keyboard =
            setup_lang().map_err(|e| format!("Language setup failed: {}", e))?;

        let boot_mode =
            detect_boot_mode().map_err(|e| format!("Failed to detect boot mode: {}", e))?;

        match boot_mode {
            BootMode::Efi => log_info!("{}", "Boot mode: UEFI".cyan().bold()),
            BootMode::Legacy => log_info!("{}", "Boot mode: Legacy BIOS".cyan().bold()),
        }

        let target = select_disk_interactive().ok_or_else(|| "No disk selected".to_string())?;

        let layout = match target.kind {
            DiskKind::Disk => {
                let prompt = match boot_mode {
                    BootMode::Efi => "Use entire disk and create EFI + root partitions?",
                    BootMode::Legacy => "Use entire disk and create BIOS Boot + root partitions?",
                };

                let entire = Confirm::new(prompt)
                    .with_default(false)
                    .with_help_message("Nothing is changed yet: you confirm at the very end")
                    .prompt()
                    .map_err(|e| format!("Confirmation error: {}", e))?;

                if entire {
                    Layout::EntireDisk
                } else {
                    Layout::Manual
                }
            }
            DiskKind::Partition => Layout::ExistingPartition,
        };

        let accounts =
            ask_accounts().map_err(|e| format!("User configuration failed: {}", e))?;

        let hostname = get_hostname().map_err(|e| format!("Hostname: {}", e))?;

        let locale = ask_locale().map_err(|e| format!("Language selection failed: {}", e))?;

        let timezone = ask_timezone().map_err(|e| format!("Timezone selection failed: {}", e))?;

        let network = ask_network().map_err(|e| format!("Network configuration failed: {}", e))?;

        Ok(InstallPlan {
            keyboard,
            boot_mode,
            target,
            layout,
            accounts,
            hostname,
            locale,
            timezone,
            network,
        })
    }

    pub fn total_steps(&self) -> usize {
        count_steps(self.layout, self.boot_mode)
    }

    /// Lignes (libellé, valeur) du récapitulatif. Jamais de mot de passe.
    pub fn summary_lines(&self) -> Vec<(&'static str, String)> {
        let boot = match self.boot_mode {
            BootMode::Efi => "UEFI",
            BootMode::Legacy => "Legacy BIOS",
        };
        let kind = match self.target.kind {
            DiskKind::Disk => "disk",
            DiskKind::Partition => "partition",
        };

        vec![
            ("Keyboard", self.keyboard.clone()),
            ("Boot mode", boot.to_string()),
            (
                "Target",
                format!("{} ({})", self.target.device, kind),
            ),
            (
                "Disk changes",
                layout_description(self.layout, self.boot_mode, &self.target.device),
            ),
            (
                "User",
                format!(
                    "{} (home {}, shell {})",
                    self.accounts.username, self.accounts.home, self.accounts.shell
                ),
            ),
            ("Passwords", "root and user (hidden)".to_string()),
            ("Hostname", self.hostname.clone()),
            ("Language", self.locale.clone()),
            ("Timezone", self.timezone.clone()),
            ("Network", self.network.summary()),
        ]
    }

    pub fn print_summary(&self) {
        println!();
        println!("{}", "═══ Installation summary ═══".cyan().bold());
        for (label, value) in self.summary_lines() {
            println!("  {} {}", format!("{:<13}", format!("{label}:")).bold(), value);
        }
        println!();
        println!(
            "  {}",
            format!(
                "{} steps will run. Nothing has been written to the disk yet.",
                self.total_steps()
            )
            .green()
        );
        println!();
    }
}

/// Confirmation finale. Le choix par défaut (Entrée) est d'annuler.
pub fn ask_decision(plan: &InstallPlan) -> Result<Decision, String> {
    let install = format!("INSTALL now (modifies {})", plan.target.device);
    let start_over = "Start over (change the answers)".to_string();
    let cancel = "Cancel (nothing is modified)".to_string();

    let choice = Select::new(
        "Ready to install?",
        vec![cancel.clone(), start_over.clone(), install.clone()],
    )
    .prompt()
    .map_err(|e| format!("Confirmation error: {}", e))?;

    Ok(if choice == install {
        Decision::Install
    } else if choice == start_over {
        Decision::StartOver
    } else {
        Decision::Cancel
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn step_counts() {
        assert_eq!(count_steps(Layout::ExistingPartition, BootMode::Legacy), 12);
        assert_eq!(count_steps(Layout::ExistingPartition, BootMode::Efi), 12);
        assert_eq!(count_steps(Layout::Manual, BootMode::Legacy), 13);
        assert_eq!(count_steps(Layout::Manual, BootMode::Efi), 13);
        assert_eq!(count_steps(Layout::EntireDisk, BootMode::Legacy), 13);
        assert_eq!(count_steps(Layout::EntireDisk, BootMode::Efi), 14);
    }

    #[test]
    fn destructive_layouts_say_so() {
        let entire = layout_description(Layout::EntireDisk, BootMode::Efi, "/dev/sda");
        assert!(entire.contains("ERASE ALL of /dev/sda"));
        assert!(entire.contains("EFI"));
        let part = layout_description(Layout::ExistingPartition, BootMode::Legacy, "/dev/sda2");
        assert!(part.contains("ERASE ALL data of the partition /dev/sda2"));
        let manual = layout_description(Layout::Manual, BootMode::Efi, "/dev/sda");
        assert!(manual.contains("fdisk"));
    }
}
