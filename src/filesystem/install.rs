use inquire::{Confirm, Select};
use std::fs::{self, File};
use std::io::Write;
use std::path::Path;
use std::process::Command;

use crate::filesystem::disk::{detect_disks, detect_partitions};
use crate::filesystem::fs::{mount, unmount};

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

pub fn select_disk_interactive() -> Option<DiskOption> {
    let disks = detect_disks();
    let partitions = detect_partitions();

    let mut options: Vec<DiskOption> = Vec::new();

    // Ajouter les disques
    for d in disks {
        options.push(DiskOption {
            name: d.clone(),
            device: format!("/dev/{}", d),
            kind: DiskKind::Disk,
        });
    }

    // Ajouter les partitions
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

fn manual_partitioning(disk: &str) -> Result<String, String> {
    println!("Starting manual partitioning on {}...", disk);
    println!("Use fdisk to create or modify your partitions.");
    println!("When finished, use 'w' to write the changes and exit.");

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

    let selection = Select::new("Select the partition to install Senbit on:", choices.clone())
        .prompt()
        .map_err(|e| format!("Partition selection error: {}", e))?;

    Ok(selection)
}

fn format_partition(device: &str) -> Result<(), String> {
    // Essayer mkfs.ext4, sinon mke2fs
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

fn create_single_partition(disk: &str) -> Result<(), String> {
    // Utilise parted pour créer une partition unique qui prend tout le disque
    // Exemple : parted /dev/vda mklabel gpt mkpart primary ext4 0% 100%
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

fn create_minimal_rootfs(root: &str) -> Result<(), String> {
    let dirs = [
        "etc", "bin", "usr", "var", "tmp", "root", "home",
        "proc", "sys", "dev", "run",
    ];

    for d in &dirs {
        let path = Path::new(root).join(d);
        fs::create_dir_all(&path)
            .map_err(|e| format!("Failed to create directory {}: {}", path.display(), e))?;
    }

    // /etc/senbit-release
    let release_path = Path::new(root).join("etc/senbit-release");
    let mut f = File::create(&release_path)
        .map_err(|e| format!("Failed to create senbit-release: {}", e))?;
    writeln!(f, "Senbit 0.1.0")
        .map_err(|e| format!("Failed to write senbit-release: {}", e))?;

    // /etc/passwd minimal
    let passwd_path = Path::new(root).join("etc/passwd");
    let mut f = File::create(&passwd_path)
        .map_err(|e| format!("Failed to create passwd: {}", e))?;
    writeln!(f, "root:x:0:0:root:/root:/bin/sh")
        .map_err(|e| format!("Failed to write passwd: {}", e))?;

    Ok(())
}

pub fn install_system() -> Result<(), String> {
    let target = match select_disk_interactive() {
        Some(t) => t,
        None => return Err("No disk selected".to_string()),
    };

    println!("Installing on {} ({})", target.device, target.name);

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

    println!("Formatting partition {}...", partition_device);
    format_partition(&partition_device)?;

    let mount_point = "/mnt";

    println!("Mounting partition on {}...", mount_point);
    mount(&partition_device, mount_point)
        .map_err(|e| format!("Failed to mount: {}", e))?;

    println!("Creating minimal rootfs...");
    create_minimal_rootfs(mount_point)?;

    println!("Unmounting...");
    unmount(mount_point)
        .map_err(|e| format!("Failed to unmount: {}", e))?;

    Ok(())
}