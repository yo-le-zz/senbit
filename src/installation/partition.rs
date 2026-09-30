use std::fs::OpenOptions;
use std::io::{Seek, SeekFrom, Write};
use std::os::unix::io::AsRawFd;
use std::process::Command;
use std::thread;
use std::time::Duration;

use inquire::Select;

use crate::logln;

use crate::filesystem::disk::{
    detect_disks,
    detect_partitions,
};

/* ============================================================
Structs definitions
============================================================ */

#[derive(Clone)]
pub struct DiskOption {
    pub name: String,
    pub device: String,
    pub kind: DiskKind,
}

#[derive(Clone, Copy)]
pub enum DiskKind {
    Disk,
    Partition,
}

/* ============================================================
Helpers
============================================================ */

fn partition_path(
    disk: &str,
    number: u32,
) -> String {
    if disk.contains("nvme")
        || disk.contains("mmcblk")
    {
        format!(
            "{}p{}",
            disk,
            number
        )
    } else {
        format!(
            "{}{}",
            disk,
            number
        )
    }
}

fn run(
    command: &str,
    args: &[&str],
) -> Result<(), String> {
    let status =
        Command::new(command)
            .args(args)
            .status()
            .map_err(|e| {
                format!(
                    "Failed to run {}: {}",
                    command,
                    e
                )
            })?;

    if !status.success() {
        return Err(
            format!(
                "{} exited with status: {}",
                command,
                status
            )
        );
    }

    Ok(())
}

/*
 * Pure Rust replacement for `wipefs -a`.
 *
 * Zeroes the first and last 2 MiB of the disk:
 *   - start: MBR / GPT header / filesystem signatures
 *   - end  : GPT backup header
 */
fn zero_disk_edges(
    disk: &str,
) -> Result<(), String> {
    const CHUNK: usize =
        2 * 1024 * 1024;

    let mut file =
        OpenOptions::new()
            .write(true)
            .open(disk)
            .map_err(|e| {
                format!(
                    "Failed to open {}: {}",
                    disk,
                    e
                )
            })?;

    let size =
        file.seek(
            SeekFrom::End(0),
        )
        .map_err(|e| {
            format!(
                "Failed to get size of {}: {}",
                disk,
                e
            )
        })?;

    let zeros =
        vec![0u8; CHUNK];

    let len =
        std::cmp::min(
            size,
            CHUNK as u64,
        ) as usize;

    file.seek(
        SeekFrom::Start(0),
    )
    .and_then(|_| {
        file.write_all(
            &zeros[..len],
        )
    })
    .map_err(|e| {
        format!(
            "Failed to wipe start of {}: {}",
            disk,
            e
        )
    })?;

    if size > CHUNK as u64 {
        file.seek(
            SeekFrom::Start(
                size - CHUNK as u64,
            ),
        )
        .and_then(|_| {
            file.write_all(
                &zeros,
            )
        })
        .map_err(|e| {
            format!(
                "Failed to wipe end of {}: {}",
                disk,
                e
            )
        })?;
    }

    file.sync_all()
        .map_err(|e| {
            format!(
                "Failed to sync {}: {}",
                disk,
                e
            )
        })?;

    Ok(())
}

fn wipe_disk(
    disk: &str,
) -> Result<(), String> {
    logln!(
        "Wiping existing filesystem and partition signatures on {}...",
        disk
    );

    // Prefer wipefs when it exists, otherwise do it ourselves.
    let wipefs_ok =
        Command::new("wipefs")
            .args([
                "-a",
                "-f",
                disk,
            ])
            .status()
            .map(|s| s.success())
            .unwrap_or(false);

    if wipefs_ok {
        return Ok(());
    }

    logln!(
        "wipefs unavailable, wiping disk edges directly..."
    );

    zero_disk_edges(disk)
}

/*
 * Asks the kernel to re-read the partition table
 * (BLKRRPART), without needing partprobe.
 */
fn reread_partition_table_ioctl(
    disk: &str,
) -> Result<(), String> {
    const BLKRRPART: u32 =
        0x125F;

    let file =
        OpenOptions::new()
            .read(true)
            .open(disk)
            .map_err(|e| {
                format!(
                    "Failed to open {}: {}",
                    disk,
                    e
                )
            })?;

    let ret = unsafe {
        libc::ioctl(
            file.as_raw_fd(),
            BLKRRPART as _,
        )
    };

    if ret < 0 {
        return Err(
            format!(
                "BLKRRPART failed on {}: {}",
                disk,
                std::io::Error::last_os_error()
            )
        );
    }

    Ok(())
}

fn reload_partition_table(
    disk: &str,
) -> Result<(), String> {
    logln!(
        "Reloading partition table on {}...",
        disk
    );

    let partprobe_ok =
        Command::new("partprobe")
            .arg(disk)
            .status()
            .map(|s| s.success())
            .unwrap_or(false);

    if !partprobe_ok {
        reread_partition_table_ioctl(
            disk,
        )?;
    }

    // udevadm may not exist on a minimal system.
    let _ =
        Command::new("udevadm")
            .args([
                "settle",
            ])
            .status();

    // Give the kernel/udev a little time to create
    // the new partition device nodes.
    thread::sleep(
        Duration::from_millis(1000),
    );

    Ok(())
}

/* ============================================================
UEFI partition creation
============================================================ */

pub fn create_uefi_partitions(
    disk: &str,
) -> Result<(String, String), String> {
    logln!(
        "Creating GPT partition table on {}...",
        disk
    );

    wipe_disk(disk)?;

    run(
        "parted",
        &[
            "-s",
            disk,

            "mklabel",
            "gpt",

            "mkpart",
            "ESP",
            "fat32",
            "1MiB",
            "513MiB",

            "set",
            "1",
            "esp",
            "on",

            "mkpart",
            "primary",
            "ext4",
            "513MiB",
            "100%",
        ],
    )?;

    reload_partition_table(disk)?;

    let efi_partition =
        partition_path(
            disk,
            1,
        );

    let root_partition =
        partition_path(
            disk,
            2,
        );

    logln!(
        "EFI partition: {}",
        efi_partition
    );

    logln!(
        "Root partition: {}",
        root_partition
    );

    Ok((
        efi_partition,
        root_partition,
    ))
}

/* ============================================================
Legacy BIOS partition creation
============================================================ */

/*
 * Legacy BIOS on GPT requires a BIOS Boot partition.
 *
 * Layout:
 *
 *   /dev/sda1 -> BIOS Boot  (~1 MiB)
 *   /dev/sda2 -> root       (ext4)
 *
 * The BIOS Boot partition does NOT contain a filesystem.
 */

pub fn create_legacy_partitions(
    disk: &str,
) -> Result<String, String> {
    logln!(
        "Creating GPT partition table with BIOS Boot partition on {}...",
        disk
    );

    wipe_disk(disk)?;

    run(
        "parted",
        &[
            "-s",
            disk,

            "mklabel",
            "gpt",

            "mkpart",
            "bios_grub",
            "1MiB",
            "2MiB",

            "set",
            "1",
            "bios_grub",
            "on",

            "mkpart",
            "primary",
            "ext4",
            "2MiB",
            "100%",
        ],
    )?;

    reload_partition_table(disk)?;

    let root_partition =
        partition_path(
            disk,
            2,
        );

    logln!(
        "BIOS Boot partition: {}",
        partition_path(
            disk,
            1,
        )
    );

    logln!(
        "Root partition: {}",
        root_partition
    );

    Ok(root_partition)
}

/* ============================================================
EFI partition formatting
============================================================ */

pub fn format_efi_partition(
    device: &str,
) -> Result<(), String> {
    logln!(
        "Formatting EFI partition {}...",
        device
    );

    let status =
        Command::new("mkfs.vfat")
            .args([
                "-F",
                "32",
                device,
            ])
            .status()
            .map_err(|e| {
                format!(
                    "Failed to run mkfs.vfat: {}",
                    e
                )
            })?;

    if !status.success() {
        return Err(
            format!(
                "mkfs.vfat exited with status: {}",
                status
            )
        );
    }

    Ok(())
}

/* ============================================================
Disk and partition selection
============================================================ */

pub fn select_disk_interactive()
    -> Option<DiskOption>
{
    let disks =
        detect_disks();

    let partitions =
        detect_partitions();

    let mut options:
        Vec<DiskOption> =
        Vec::new();

    // Add disks
    for d in disks {
        options.push(
            DiskOption {
                name: d.clone(),
                device: format!(
                    "/dev/{}",
                    d
                ),
                kind: DiskKind::Disk,
            },
        );
    }

    // Add partitions
    for p in partitions {
        options.push(
            DiskOption {
                name: p.clone(),
                device: format!(
                    "/dev/{}",
                    p
                ),
                kind: DiskKind::Partition,
            },
        );
    }

    if options.is_empty() {
        return None;
    }

    let choices:
        Vec<String> =
        options
            .iter()
            .map(|o| {
                let kind_str =
                    match o.kind {
                        DiskKind::Disk =>
                            "(disk)",

                        DiskKind::Partition =>
                            "(partition)",
                    };

                format!(
                    "{} {}",
                    o.name,
                    kind_str
                )
            })
            .collect();

    let selection =
        Select::new(
            "Select target disk/partition:",
            choices.clone(),
        )
        .prompt()
        .ok()?;

    let index =
        choices
            .iter()
            .position(
                |c| c == &selection
            )?;

    Some(
        options[index].clone()
    )
}

/* ============================================================
Manual partitioning
============================================================ */

pub fn manual_partitioning(
    disk: &str,
) -> Result<String, String> {
    logln!(
        "Starting manual partitioning on {}...",
        disk
    );

    logln!(
        "Use fdisk to create or modify your partitions."
    );

    logln!(
        "When finished, use 'w' to write the changes and exit."
    );

    let status =
        Command::new("fdisk")
            .arg(disk)
            .status()
            .map_err(|e| {
                format!(
                    "Failed to run fdisk: {}",
                    e
                )
            })?;

    if !status.success() {
        return Err(
            format!(
                "fdisk exited with status: {}",
                status
            )
        );
    }

    reload_partition_table(disk)?;

    let partitions =
        detect_partitions();

    if partitions.is_empty() {
        return Err(
            "No partitions found after manual partitioning."
                .to_string()
        );
    }

    let choices:
        Vec<String> =
        partitions
            .iter()
            .map(
                |partition| {
                    format!(
                        "/dev/{}",
                        partition
                    )
                }
            )
            .collect();

    let selection =
        Select::new(
            "Select the partition to install Senbit on:",
            choices.clone(),
        )
        .prompt()
        .map_err(|e| {
            format!(
                "Partition selection error: {}",
                e
            )
        })?;

    Ok(selection)
}

/* ============================================================
Partition formatting
============================================================ */

pub fn format_partition(
    device: &str,
) -> Result<(), String> {
    logln!(
        "Formatting {} as ext4...",
        device
    );

    let status =
        if Command::new("mkfs.ext4")
            .arg("--version")
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
        {
            Command::new("mkfs.ext4")
                .arg("-F")
                .arg(device)
                .status()
        } else {
            Command::new("mke2fs")
                .args([
                    "-F",
                    device,
                ])
                .status()
        }
        .map_err(|e| {
            format!(
                "Failed to format partition: {}",
                e
            )
        })?;

    if status.success() {
        Ok(())
    } else {
        Err(
            format!(
                "Format command exited with status: {}",
                status
            )
        )
    }
}

/* ============================================================
Compatibility helper
============================================================ */

pub fn create_single_partition(
    disk: &str,
) -> Result<(), String> {
    create_legacy_partitions(disk)
        .map(|_| ())
}