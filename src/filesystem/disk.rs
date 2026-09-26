use std::fs::File;
use std::io::{BufRead, BufReader};

use colored::Colorize;

use anyhow::{Result, Context};

use crate::filesystem::fs::{unmount, mount};
use crate::utils::files::file_exists;

pub fn detect_installation() -> Option<String> {
    let partitions = detect_partitions();

    for p in &partitions {
        let device = format!("/dev/{}", p);
        let mount_point = "/mnt";

        if mount_partition(&device, mount_point).is_err() {
            continue;
        }

        let marker = format!("{}/etc/senbit-release", mount_point);
        let installed = file_exists(&marker);

        let _ = unmount_partition(mount_point);

        if installed {
            return Some(p.clone());
        }
    }

    None
}

fn mount_partition(device: &str, target: &str) -> Result<()> {
    mount(device, target)
        .context("failed to mount partition")?;
    Ok(())
}

fn unmount_partition(target: &str) -> Result<()> {
    unmount(target)
        .context("failed to unmount partition")?;
    Ok(())
}

fn disk_detection() {
    let disks = detect_disks();
    let partitions = detect_partitions();

    println!("{}", "Detected disks:".green());
    for d in &disks {
        println!("{}", d);
    }

    println!("{}", "Detected partitions:".green());
    for p in &partitions {
        println!("{}", p);
    }
}

/// Detect all disks (e.g. "vda", "sda", "nvme0n1")
pub fn detect_disks() -> Vec<String> {
    let mut disks = Vec::new();

    // /proc/partitions format:
    // major minor #blocks name
    let file = match File::open("/proc/partitions") {
        Ok(f) => f,
        Err(_) => return disks,
    };

    let reader = BufReader::new(file);

    for line in reader.lines().flatten() {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 4 {
            continue;
        }

        let name = parts[3];

        // Skip partitions: they usually have digits in their name (e.g. vda1, sda2)
        // Disks are typically: vda, sda, nvme0n1, etc.
        if is_disk_name(name) {
            disks.push(name.to_string());
        }
    }

    disks
}

/// Detect all partitions (e.g. "vda1", "sda2", "nvme0n1p1")
pub fn detect_partitions() -> Vec<String> {
    let mut partitions = Vec::new();

    let file = match File::open("/proc/partitions") {
        Ok(f) => f,
        Err(_) => return partitions,
    };

    let reader = BufReader::new(file);

    for line in reader.lines().flatten() {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 4 {
            continue;
        }

        let name = parts[3];

        if !is_disk_name(name) {
            // Then it's a partition
            partitions.push(name.to_string());
        }
    }

    partitions
}

/// Heuristic to distinguish disk names from partition names.
/// This is not perfect but works for common cases:
/// - Disks: vda, sda, nvme0n1, mmcblk0, etc.
/// - Partitions: vda1, sda2, nvme0n1p1, mmcblk0p1, etc.
fn is_disk_name(name: &str) -> bool {
    // If the name ends with a digit, it's likely a partition
    // But some disks also end with digits (e.g. nvme0n1, mmcblk0)
    // So we use a simple heuristic:
    // - If it ends with a digit AND the previous char is also a digit or 'p', it's a partition
    // - Otherwise, consider it a disk

    let chars: Vec<char> = name.chars().collect();
    if chars.is_empty() {
        return false;
    }

    let last = chars[chars.len() - 1];

    if !last.is_ascii_digit() {
        // Doesn't end with a digit -> likely a disk
        return true;
    }

    // Ends with a digit, check previous char
    if chars.len() == 1 {
        // Single digit name, unlikely but treat as disk
        return true;
    }

    let prev = chars[chars.len() - 2];

    if prev == 'p' || prev.is_ascii_digit() {
        // Patterns like nvme0n1p1, sda1, mmcblk0p1
        return false;
    }

    // Patterns like nvme0n1, mmcblk0
    true
}