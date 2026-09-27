// filesystem/disk.rs

use std::fs::File;
use std::io::{BufRead, BufReader};

use crate::logln;

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
    // On s'assure que target est un chemin absolu propre (pas de "//mnt")
    let target = target.trim_end_matches('/');
    mount(device, target)
        .context("failed to mount partition")?;
    Ok(())
}

fn unmount_partition(target: &str) -> Result<()> {
    let target = target.trim_end_matches('/');
    unmount(target)
        .context("failed to unmount partition")?;
    Ok(())
}

fn disk_detection() {
    let disks = detect_disks();
    let partitions = detect_partitions();

    logln!("{}", "Detected disks:".green());
    for d in &disks {
        logln!("{}", d);
    }

    logln!("{}", "Detected partitions:".green());
    for p in &partitions {
        logln!("{}", p);
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
    if name.starts_with("nvme") || name.starts_with("mmcblk") {
        return !name.contains('p');
    }

    !name.chars().last().is_some_and(|c| c.is_ascii_digit())
}
