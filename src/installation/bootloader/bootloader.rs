use anyhow::{
    Context,
    Result,
};

use colored::Colorize;

use std::path::{
    Path,
    PathBuf,
};

use crate::log_info;

use crate::installation::bootloader::uefi;

/* ============================================================
Boot mode
============================================================ */

pub enum BootMode {
    Legacy,
    Efi,
}

pub fn detect_boot_mode()
    -> Result<BootMode>
{
    if Path::new(
        "/sys/firmware/efi",
    )
    .exists()
    {
        Ok(BootMode::Efi)
    } else {
        Ok(BootMode::Legacy)
    }
}

/* ============================================================
Parent disk detection
============================================================ */

pub fn parent_disk(
    device: &Path,
) -> Result<PathBuf> {
    /*
     * Uses sysfs instead of lsblk (lsblk depends on udev and
     * may fail on a minimal live system).
     *
     *   /sys/class/block/sda1  -> has a "partition" file
     *                             real path: .../block/sda/sda1
     *                             parent dir name: "sda"
     *
     *   /sys/class/block/sda   -> no "partition" file: already a disk
     */
    let name =
        device
            .file_name()
            .and_then(|n| n.to_str())
            .context(
                "Invalid device path",
            )?;

    let sys_path =
        Path::new(
            "/sys/class/block",
        )
        .join(name);

    if !sys_path
        .join("partition")
        .exists()
    {
        // Already a whole disk (or unknown): use it as is.
        return Ok(
            device.to_path_buf()
        );
    }

    let real_path =
        std::fs::canonicalize(
            &sys_path,
        )
        .with_context(|| {
            format!(
                "Failed to resolve {}",
                sys_path.display()
            )
        })?;

    let parent_name =
        real_path
            .parent()
            .and_then(|p| p.file_name())
            .and_then(|n| n.to_str())
            .context(
                "Failed to find parent disk",
            )?;

    Ok(
        PathBuf::from(
            format!(
                "/dev/{}",
                parent_name
            ),
        )
    )
}

/* ============================================================
Legacy layout verification
============================================================ */

/*
 * Uses `parted -sm <disk> print` (machine readable), which reads
 * the disk directly and does NOT depend on the udev database
 * (unlike lsblk PTTYPE / PARTTYPE columns).
 *
 * Output example:
 *   BYT;
 *   /dev/sda:21.5GB:scsi:512:512:gpt:QEMU HARDDISK:;
 *   1:1049kB:2097kB:1049kB::biosgrub:bios_grub;
 *   2:2097kB:21.5GB:21.5GB:ext4::;
 *
 * Disk line : field 5 = partition table type
 * Part lines: field 6 = flags
 */
fn check_legacy_layout(
    disk: &Path,
) -> Result<()> {
    let disk_str =
        disk
            .to_str()
            .context(
                "Invalid disk path",
            )?;

    let output =
        std::process::Command::new(
            "parted",
        )
        .args([
            "-sm",
            disk_str,
            "print",
        ])
        .output()
        .context(
            "Failed to execute parted",
        )?;

    if !output.status.success() {
        anyhow::bail!(
            "parted could not read the partition table of {}",
            disk.display()
        );
    }

    let text =
        String::from_utf8_lossy(
            &output.stdout,
        )
        .to_string();

    let mut table =
        String::new();

    let mut has_bios_boot =
        false;

    for raw in text.lines() {
        let line =
            raw
                .trim()
                .trim_end_matches(';');

        if line.starts_with('/') {
            table =
                line
                    .split(':')
                    .nth(5)
                    .unwrap_or("")
                    .trim()
                    .to_lowercase();

            continue;
        }

        let is_partition_line =
            line
                .chars()
                .next()
                .map_or(
                    false,
                    |c| c.is_ascii_digit(),
                );

        if is_partition_line {
            let flags =
                line
                    .split(':')
                    .nth(6)
                    .unwrap_or("");

            if flags
                .split(',')
                .any(
                    |f| f.trim() == "bios_grub",
                )
            {
                has_bios_boot = true;
            }
        }
    }

    log_info!(
        "Partition table on {}: {} (BIOS Boot partition: {})",
        disk.display(),
        if table.is_empty() {
            "unknown"
        } else {
            &table
        },
        has_bios_boot
    );

    /*
     * MBR (msdos) does not need a BIOS Boot partition:
     * GRUB embeds itself in the gap after the MBR.
     */
    if table == "gpt" && !has_bios_boot {
        anyhow::bail!(
            "{} uses GPT but has no BIOS Boot partition (flag bios_grub, ~1 MiB). \
             Choose 'Use entire disk', or create one with fdisk (type 4).",
            disk.display()
        );
    }

    Ok(())
}

/* ============================================================
Legacy GRUB installation
============================================================ */

fn install_legacy(
    mount_point: &Path,
    disk: &Path,
) -> Result<()> {
    // Marker: proves the freshly built binary is the one running.
    log_info!(
        "[bootloader v2] legacy install"
    );

    /*
     * grub-install needs the physical disk,
     * never a partition.
     */
    let disk =
        parent_disk(disk)?;

    check_legacy_layout(
        &disk,
    )?;

    let boot_directory =
        mount_point.join(
            "boot",
        );

    std::fs::create_dir_all(
        &boot_directory,
    )?;

    log_info!(
        "Installing GRUB on {}...",
        disk.display()
    );

    let boot_directory_str =
        boot_directory
            .to_str()
            .context(
                "Invalid boot directory",
            )?;

    let disk_str =
        disk
            .to_str()
            .context(
                "Invalid disk path",
            )?;

    let status =
        std::process::Command::new(
            "grub-install",
        )
        .args([
            "--target=i386-pc",
            "--boot-directory",
            boot_directory_str,
            "--recheck",
            disk_str,
        ])
        .status()
        .context(
            "Failed to execute grub-install",
        )?;

    if !status.success() {
        anyhow::bail!(
            "grub-install failed on {}",
            disk.display()
        );
    }

    Ok(())
}

/* ============================================================
Bootloader installation
============================================================ */

fn install_bootloader(
    mode: &BootMode,
    mount_point: &Path,
    disk: &Path,
) -> Result<()> {
    match mode {
        BootMode::Legacy => {
            log_info!(
                "{}",
                "Installing GRUB in Legacy mode..."
                    .cyan()
            );

            install_legacy(
                mount_point,
                disk,
            )?;
        }

        BootMode::Efi => {
            log_info!(
                "{}",
                "Installing GRUB in EFI mode..."
                    .cyan()
            );

            /*
             * UEFI also receives the physical disk.
             * The UEFI module can then find the ESP
             * from that disk.
             */
            let disk =
                parent_disk(disk)?;

            uefi::install_uefi(
                mount_point,
                &disk,
            )?;
        }
    }

    Ok(())
}

/* ============================================================
GRUB configuration
============================================================ */

fn write_bootloader_config(
    mount_point: &Path,
    root_uuid: &str,
) -> Result<()> {
    let grub_directory =
        mount_point.join(
            "boot/grub",
        );

    std::fs::create_dir_all(
        &grub_directory,
    )?;
    
    let config =
        format!(
            r#"set timeout=5
set default=0
terminal_output console
set gfxpayload=keep

menuentry "Senbit" {{
    linux /boot/vmlinuz root=UUID={} ro console=tty0 console=ttyS0,115200
    initrd /boot/initramfs.cpio.gz
}}
"#,
            root_uuid
        );

    std::fs::write(
        grub_directory.join(
            "grub.cfg",
        ),
        config,
    )?;

    Ok(())
}

/* ============================================================
Public bootloader setup
============================================================ */

pub fn setup_bootloader(
    mount_point: &Path,
    disk: &Path,
    root_uuid: &str,
) -> Result<()> {
    let mode =
        detect_boot_mode()?;

    match mode {
        BootMode::Legacy => {
            log_info!(
                "{}",
                "Boot mode: Legacy"
                    .cyan()
            );
        }

        BootMode::Efi => {
            log_info!(
                "{}",
                "Boot mode: EFI"
                    .cyan()
            );
        }
    }

    install_bootloader(
        &mode,
        mount_point,
        disk,
    )?;

    write_bootloader_config(
        mount_point,
        root_uuid,
    )?;

    Ok(())
}