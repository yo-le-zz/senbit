use std::process::Command;
use inquire::Select;
use crate::logln;

use crate::filesystem::disk::{detect_disks, detect_partitions};

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
Disk and partition selection
============================================================ */

pub fn select_disk_interactive() -> Option<DiskOption> {
    let disks = detect_disks();
    let partitions = detect_partitions();

    let mut options: Vec<DiskOption> = Vec::new();

    // Add disks
    for d in disks {
        options.push(DiskOption {
            name: d.clone(),
            device: format!("/dev/{}", d),
            kind: DiskKind::Disk,
        });
    }

    // Add partitions
    for p in partitions {
        options.push(DiskOption {
            name: p.clone(),
            device: format!("/dev/{}", p),
            kind: DiskKind::Partition,
        });
    }

    if options.is_empty() {
        return None;
    }

    let choices: Vec<String> = options
        .iter()
        .map(|o| {
            let kind_str = match o.kind {
                DiskKind::Disk => "(disk)",
                DiskKind::Partition => "(partition)",
            };
            format!("{} {}", o.name, kind_str)
        })
        .collect();

    let selection = Select::new("Select target disk/partition:", choices.clone())
        .prompt()
        .ok()?;

    let index = choices.iter().position(|c| c == &selection)?;
    Some(options[index].clone())
}

pub fn manual_partitioning(disk: &str) -> Result<String, String> {
    logln!("Starting manual partitioning on {}...", disk);
    logln!("Use fdisk to create or modify your partitions.");
    logln!("When finished, use 'w' to write the changes and exit.");

    let status = Command::new("fdisk")
        .arg(disk)
        .status()
        .map_err(|e| format!("Failed to run fdisk: {}", e))?;

    if !status.success() {
        return Err(format!("fdisk exited with status: {}", status));
    }

    // Ask the kernel to reload the partition table.
    let status = Command::new("partprobe")
        .arg(disk)
        .status()
        .map_err(|e| format!("Failed to run partprobe: {}", e))?;

    if !status.success() {
        return Err(format!("partprobe exited with status: {}", status));
    }

    let partitions = detect_partitions();

    if partitions.is_empty() {
        return Err("No partitions found after manual partitioning.".to_string());
    }

    let choices: Vec<String> = partitions
        .iter()
        .map(|partition| format!("/dev/{}", partition))
        .collect();

    let selection = Select::new(
        "Select the partition to install Senbit on:",
        choices.clone(),
    )
    .prompt()
    .map_err(|e| format!("Partition selection error: {}", e))?;

    Ok(selection)
}

pub fn format_partition(device: &str) -> Result<(), String> {
    // Try mkfs.ext4, otherwise use mke2fs.
    let status = if Command::new("mkfs.ext4")
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
            .args(["-F", device])
            .status()
    }
    .map_err(|e| format!("Failed to format partition: {}", e))?;

    if status.success() {
        Ok(())
    } else {
        Err(format!("Format command exited with status: {}", status))
    }
}

pub fn create_single_partition(disk: &str) -> Result<(), String> {
    // Use parted to create a single partition using the entire disk.
    // Example: parted /dev/vda mklabel gpt mkpart primary ext4 0% 100%
    let status = Command::new("parted")
        .args([
            disk,
            "mklabel",
            "gpt",
            "mkpart",
            "primary",
            "ext4",
            "0%",
            "100%",
        ])
        .status()
        .map_err(|e| format!("Failed to run parted: {}", e))?;

    if status.success() {
        Ok(())
    } else {
        Err(format!("parted exited with status: {}", status))
    }
}