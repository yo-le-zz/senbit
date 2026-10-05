use inquire::Confirm;

use crate::log_info;

use std::path::Path;

use colored::Colorize;

use crate::installation::local::lang::setup_lang;
use crate::installation::local::local::setup_locale;
use crate::installation::local::timezone::setup_timezone;

use crate::installation::network::network::setup_network;

use crate::installation::bootloader::bootloader::{
    detect_boot_mode,
    BootMode,
    setup_bootloader,
};

use crate::installation::partition::{
    manual_partitioning,
    select_disk_interactive,
    format_partition,
    format_efi_partition,
    create_uefi_partitions,
    create_legacy_partitions,
    DiskKind,
};

use crate::filesystem::disk::get_partition_uuid;

use crate::installation::rootfs::setup_rootfs;

use crate::filesystem::fs::{
    mount,
    unmount,
};

use crate::installation::users::perms::set_sys_perms;

use crate::installation::users::user::setup_users;

use crate::installation::hostname::setup_hostname;

pub fn install_system() -> Result<(), String> {
    // ========================================================
    // Step 1: keyboard / language
    // ========================================================

    setup_lang()
        .map_err(|e| {
            format!(
                "Language setup failed: {}",
                e
            )
        })?;

    // ========================================================
    // Step 2: detect boot mode
    // ========================================================

    let boot_mode =
        detect_boot_mode()
            .map_err(|e| {
                format!(
                    "Failed to detect boot mode: {}",
                    e
                )
            })?;

    match boot_mode {
        BootMode::Efi => {
            log_info!(
                "{}",
                "Boot mode: UEFI"
                    .cyan()
                    .bold()
            );
        }

        BootMode::Legacy => {
            log_info!(
                "{}",
                "Boot mode: Legacy BIOS"
                    .cyan()
                    .bold()
            );
        }
    }

    // ========================================================
    // Step 3: disk
    // ========================================================

    let target =
        match select_disk_interactive() {
            Some(t) => t,

            None => {
                return Err(
                    "No disk selected"
                        .to_string()
                )
            }
        };

    log_info!(
        "Installing on {} ({})",
        target.device,
        target.name
    );

    // ========================================================
    // Step 4: partition selection
    // ========================================================

    let (
        partition_device,
        efi_partition,
    ) =
        match target.kind {
            DiskKind::Disk => {
                let prompt =
                    match boot_mode {
                        BootMode::Efi => {
                            "Use entire disk and create EFI + root partitions?"
                        }

                        BootMode::Legacy => {
                            "Use entire disk and create BIOS Boot + root partitions?"
                        }
                    };

                let use_entire_disk =
                    Confirm::new(
                        prompt,
                    )
                    .with_default(false)
                    .prompt()
                    .map_err(|e| {
                        format!(
                            "Confirmation error: {}",
                            e
                        )
                    })?;

                if !use_entire_disk {
                    let root =
                        manual_partitioning(
                            &target.device,
                        )?;

                    (
                        root,
                        None,
                    )
                } else {
                    match boot_mode {
                        BootMode::Efi => {
                            let (
                                efi,
                                root,
                            ) =
                                create_uefi_partitions(
                                    &target.device,
                                )?;

                            (
                                root,
                                Some(efi),
                            )
                        }

                        BootMode::Legacy => {
                            let root =
                                create_legacy_partitions(
                                    &target.device,
                                )?;

                            (
                                root,
                                None,
                            )
                        }
                    }
                }
            }

            DiskKind::Partition => {
                let confirm =
                    Confirm::new(
                        &format!(
                            "This will erase all data on {}. Continue?",
                            target.device
                        ),
                    )
                    .with_default(false)
                    .prompt()
                    .map_err(|e| {
                        format!(
                            "Confirmation error: {}",
                            e
                        )
                    })?;

                if !confirm {
                    return Err(
                        "Installation cancelled by user."
                            .to_string()
                    );
                }

                (
                    target.device.clone(),
                    None,
                )
            }
        };

    // ========================================================
    // Step 5: format partitions
    // ========================================================

    /*
     * UEFI:
     *   ESP  -> FAT32
     *   root -> ext4
     *
     * Legacy:
     *   BIOS Boot -> no filesystem
     *   root      -> ext4
     */

    if let BootMode::Efi = boot_mode {
        if let Some(efi) =
            &efi_partition
        {
            log_info!(
                "Formatting EFI partition {}...",
                efi
            );

            format_efi_partition(
                efi,
            )?;
        }
    }

    log_info!(
        "Formatting root partition {}...",
        partition_device
    );

    format_partition(
        &partition_device,
    )?;

    // ========================================================
    // Step 6: mount root
    // ========================================================

    let mount_point =
        Path::new("/mnt");

    log_info!(
        "Mounting partition on {}...",
        mount_point.display()
    );

    mount(
        &partition_device,
        mount_point
            .to_str()
            .unwrap(),
        None,
    )
    .map_err(|e| {
        format!(
            "Failed to mount: {}",
            e
        )
    })?;

    // ========================================================
    // Step 7: root UUID
    // ========================================================

    let root_uuid =
        get_partition_uuid(
            &partition_device,
        )
        .map_err(|e| {
            format!(
                "Failed to get partition UUID: {}",
                e
            )
        })?;

    // ========================================================
    // Step 8: rootfs
    // ========================================================

    log_info!(
        "Installing rootfs..."
    );

    setup_rootfs( // install the installation file
        Path::new("/"),
        mount_point,
        &root_uuid,
    )
    .map_err(|e| {
        format!(
            "Failed to setup rootfs: {}",
            e
        )
    })?;

    log_info!(
        "{}",
        "Rootfs setup successfully."
            .green()
            .bold()
    );

    // ========================================================
    // Step 9: users
    // ========================================================

    setup_users(
        mount_point,
    )?;

    // ========================================================
    // Step 10: hostname
    // ========================================================

    setup_hostname(
        mount_point,
    )
    .map_err(|e| {
        format!(
            "Failed to set hostname: {}",
            e
        )
    })?;

    log_info!(
        "{}",
        "Hostname set successfully."
            .green()
            .bold()
    );

    // ========================================================
    // Step 11: locale
    // ========================================================

    setup_locale(
        mount_point,
    )
    .map_err(|e| {
        format!(
            "Failed to setup locale: {}",
            e
        )
    })?;

    // ========================================================
    // Step 12: timezone
    // ========================================================

    setup_timezone(
        mount_point,
    )
    .map_err(|e| {
        format!(
            "Failed to setup timezone: {}",
            e
        )
    })?;

    // ========================================================
    // Step 13: network
    // ========================================================

    setup_network(
        mount_point,
    )
    .map_err(|e| {
        format!(
            "Failed to setup network: {}",
            e
        )
    })?;

    // ========================================================
    // Step 14: bootloader
    // ========================================================

    log_info!(
        "Installing bootloader..."
    );

    setup_bootloader(
        mount_point,
        Path::new(
            &target.device,
        ),
        &root_uuid,
    )
    .map_err(|e| {
        format!(
            "Failed to install bootloader: {}",
            e
        )
    })?;

    // ========================================================
    // Step 15: set sys perms
    // ========================================================

    log_info!(
        "Setting sys perms..."
    );

    set_sys_perms(
        mount_point,
    )
    .map_err(|e| {
        format!(
            "Failed to set sys perms: {}",
            e
        )
    })?;

    // ========================================================
    // Step 16: unmount
    // ========================================================

    // Make sure everything is on the disk before the unmount.
    let _ = std::process::Command::new("sync").status();

    log_info!(
        "Unmounting..."
    );

    unmount(
        mount_point
            .to_str()
            .unwrap(),
    )
    .map_err(|e| {
        format!(
            "Failed to unmount: {}",
            e
        )
    })?;

    // Check the new filesystem (best effort: e2fsck may not be installed).
    // Exit codes 0 and 1 (= errors corrected) are fine.
    match std::process::Command::new("e2fsck")
        .args(["-f", "-y", &partition_device])
        .status()
    {
        Ok(status) => log_info!("e2fsck finished: {}", status),
        Err(e) => log_info!("e2fsck not run: {}", e),
    }

    Ok(())
}
