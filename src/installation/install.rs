// installation/install.rs

use inquire::Confirm;
use crate::logln;
use std::path::Path;

// ============================================================
// Misc functions imports
// ============================================================

use crate::installation::lang::setup_lang;
use crate::installation::partition::{
    manual_partitioning,
    select_disk_interactive,
    format_partition,
    create_single_partition,
    DiskKind,
};
use crate::filesystem::disk::get_partition_uuid;
use crate::installation::rootfs::setup_rootfs;
use crate::filesystem::fs::{mount, unmount};
use crate::installation::users::user::setup_users;

// ============================================================
// Final installation
// ============================================================

pub fn install_system() -> Result<(), String> {
    // Step 1: keyboard layout
    setup_lang()
        .map_err(|e| format!("Language setup failed: {}", e))?;

    // Step 2: disk
    let target = match select_disk_interactive() {
        Some(t) => t,
        None => return Err("No disk selected".to_string()),
    };

    logln!("Installing on {} ({})", target.device, target.name);

    let partition_device = match target.kind {
        DiskKind::Disk => {
            let use_entire_disk = Confirm::new(
                "Use entire disk and create a single partition?"
            )
            .with_default(false)
            .prompt()
            .map_err(|e| format!("Confirmation error: {}", e))?;

            if use_entire_disk {
                create_single_partition(&target.device)?;

                if target.device.contains("nvme") {
                    format!("{}p1", target.device)
                } else {
                    format!("{}1", target.device)
                }
            } else {
                manual_partitioning(&target.device)?
            }
        }

        DiskKind::Partition => {
            let confirm = Confirm::new(&format!(
                "This will erase all data on {}. Continue?",
                target.device
            ))
            .with_default(false)
            .prompt()
            .map_err(|e| format!("Confirmation error: {}", e))?;

            if !confirm {
                return Err("Installation cancelled by user.".to_string());
            }

            target.device.clone()
        }
    };

    logln!("Formatting partition {}...", partition_device);
    format_partition(&partition_device)?;

    let mount_point = Path::new("/mnt");

    logln!("Mounting partition on {}...", mount_point.display());
    mount(
        &partition_device,
        mount_point.to_str().unwrap(),
    )
    .map_err(|e| format!("Failed to mount: {}", e))?;

    logln!("Installing rootfs...");

    let root_uuid = get_partition_uuid(&partition_device)
        .map_err(|e| format!("Failed to get partition UUID: {}", e))?;

    setup_rootfs(
        Path::new("/"),
        mount_point,
        &root_uuid,
    )
    .map_err(|e| format!("Failed to setup rootfs: {}", e))?;

    logln!("Rootfs setup successfully.");

    // Step 3: users
    setup_users(mount_point)?;

    // Step 4: unmount
    logln!("Unmounting...");

    unmount(mount_point.to_str().unwrap())
        .map_err(|e| format!("Failed to unmount: {}", e))?;

    Ok(())
}