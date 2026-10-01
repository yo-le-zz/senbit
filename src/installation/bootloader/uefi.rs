use anyhow::{bail, Context, Result};
use colored::Colorize;

use std::ffi::CString;
use std::fs;
use std::io::Read;
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::logln;

const EFI_BOOTLOADER_ID: &str = "Senbit";
const GRUB_EFI_PLATFORM_DIR: &str = "/usr/lib/grub/x86_64-efi";

fn check_uefi_environment() -> Result<()> {
    if !Path::new("/sys/firmware/efi").exists() {
        bail!("System was not booted in UEFI mode");
    }

    Ok(())
}

fn partition_path(disk: &str, number: u32) -> String {
    if disk.contains("nvme") || disk.contains("mmcblk") {
        format!("{}p{}", disk, number)
    } else {
        format!("{}{}", disk, number)
    }
}

/*
 * Finds the EFI System Partition with `parted -sm <disk> print`.
 * parted reads the disk directly: no lsblk / udev needed.
 *
 * Example output:
 *   BYT;
 *   /dev/sda:21.5GB:scsi:512:512:gpt:QEMU HARDDISK:;
 *   1:1049kB:538MB:537MB:fat32:ESP:boot, esp;
 *   2:538MB:21.5GB:20.9GB:ext4:primary:;
 *
 * Partition lines: field 0 = number, field 6 = flags.
 */
fn find_efi_partition(disk: &Path) -> Result<PathBuf> {
    let disk_str = disk.to_str().context("Invalid disk path")?;

    let output = Command::new("parted")
        .args(["-sm", disk_str, "print"])
        .output()
        .context("Failed to execute parted")?;

    if !output.status.success() {
        bail!(
            "parted could not read the partition table of {}",
            disk.display()
        );
    }

    let text = String::from_utf8_lossy(&output.stdout);

    for raw in text.lines() {
        let line = raw.trim().trim_end_matches(';');

        let is_partition_line = line
            .chars()
            .next()
            .map_or(false, |c| c.is_ascii_digit());

        if !is_partition_line {
            continue;
        }

        let fields: Vec<&str> = line.split(':').collect();

        let number: u32 = match fields.first().and_then(|n| n.parse().ok()) {
            Some(n) => n,
            None => continue,
        };

        let flags = fields.get(6).copied().unwrap_or("");

        if flags.split(',').any(|f| f.trim() == "esp") {
            return Ok(PathBuf::from(partition_path(disk_str, number)));
        }
    }

    bail!(
        "EFI System Partition not found on {}",
        disk.display()
    );
}

/*
 * A FAT32 boot sector holds the string "FAT32   " at offset 82.
 * Reading it ourselves avoids lsblk / blkid.
 */
fn validate_efi_partition(partition: &Path) -> Result<()> {
    let mut file = fs::File::open(partition)
        .with_context(|| format!("Failed to open {}", partition.display()))?;

    let mut sector = [0u8; 512];

    file.read_exact(&mut sector)
        .with_context(|| format!("Failed to read {}", partition.display()))?;

    if &sector[82..90] != b"FAT32   " {
        bail!(
            "EFI partition {} is not formatted as FAT32",
            partition.display()
        );
    }

    Ok(())
}

fn efi_mount_path(mount_point: &Path) -> PathBuf {
    mount_point.join("boot/efi")
}

fn mount_vfat(source: &Path, target: &Path) -> Result<()> {
    let src = CString::new(source.as_os_str().as_bytes())?;
    let tgt = CString::new(target.as_os_str().as_bytes())?;
    let fstype = CString::new("vfat")?;

    let ret = unsafe {
        libc::mount(
            src.as_ptr(),
            tgt.as_ptr(),
            fstype.as_ptr(),
            0,
            std::ptr::null(),
        )
    };

    if ret != 0 {
        bail!(
            "mount {} on {} failed: {} \
             (if this says 'No such device', enable CONFIG_VFAT_FS, \
             CONFIG_FAT_FS, CONFIG_NLS_CODEPAGE_437 and \
             CONFIG_NLS_ISO8859_1 in the kernel)",
            source.display(),
            target.display(),
            std::io::Error::last_os_error()
        );
    }

    Ok(())
}

fn mount_efi_partition(
    efi_partition: &Path,
    mount_point: &Path,
) -> Result<PathBuf> {
    validate_efi_partition(efi_partition)?;

    let efi_directory = efi_mount_path(mount_point);

    fs::create_dir_all(&efi_directory)
        .context("Failed to create /boot/efi")?;

    logln!(
        "{}",
        format!(
            "Mounting EFI partition {} on {}...",
            efi_partition.display(),
            efi_directory.display()
        )
        .cyan()
    );

    mount_vfat(efi_partition, &efi_directory)?;

    Ok(efi_directory)
}

fn verify_efi_mount(efi_directory: &Path) -> Result<()> {
    let mounts = fs::read_to_string("/proc/mounts")
        .context("Failed to read /proc/mounts")?;

    let target = efi_directory
        .to_str()
        .context("Invalid EFI mount point")?;

    let mounted = mounts
        .lines()
        .any(|l| l.split_whitespace().nth(1) == Some(target));

    if !mounted {
        bail!(
            "EFI partition is not mounted at {}",
            efi_directory.display()
        );
    }

    Ok(())
}

fn prepare_efi_directories(efi_directory: &Path) -> Result<()> {
    fs::create_dir_all(
        efi_directory.join("EFI").join(EFI_BOOTLOADER_ID),
    )?;

    fs::create_dir_all(efi_directory.join("EFI").join("BOOT"))?;

    Ok(())
}

fn install_grub(
    mount_point: &Path,
    efi_directory: &Path,
) -> Result<()> {
    if !Path::new(GRUB_EFI_PLATFORM_DIR).is_dir() {
        bail!(
            "GRUB x86_64-efi files not found in {}: \
             the live system must include GRUB built for x86_64-efi",
            GRUB_EFI_PLATFORM_DIR
        );
    }

    let boot_directory = mount_point.join("boot");

    logln!(
        "{}",
        "Installing GRUB for x86_64 UEFI...".cyan()
    );

    /*
     * --no-nvram: do not call efibootmgr (not needed to boot, the
     * fallback loader EFI/BOOT/BOOTX64.EFI is created below).
     */
    let status = Command::new("/usr/sbin/grub-install")
        .args([
            "--target=x86_64-efi",
            "--efi-directory",
            efi_directory
                .to_str()
                .context("Invalid EFI directory")?,
            "--boot-directory",
            boot_directory
                .to_str()
                .context("Invalid boot directory")?,
            "--bootloader-id",
            EFI_BOOTLOADER_ID,
            "--no-nvram",
            "--recheck",
        ])
        .status()
        .with_context(|| {
            format!(
                "Failed to execute /usr/sbin/grub-install: {}",
                std::io::Error::last_os_error()
            )
        })?;

    if !status.success() {
        bail!("grub-install failed");
    }

    Ok(())
}

fn verify_grub_installation(efi_directory: &Path) -> Result<()> {
    let loader = efi_directory
        .join("EFI")
        .join(EFI_BOOTLOADER_ID)
        .join("grubx64.efi");

    if !loader.exists() {
        bail!(
            "GRUB EFI loader was not found: {}",
            loader.display()
        );
    }

    Ok(())
}

fn create_fallback_loader(efi_directory: &Path) -> Result<()> {
    let source = efi_directory
        .join("EFI")
        .join(EFI_BOOTLOADER_ID)
        .join("grubx64.efi");

    let fallback = efi_directory
        .join("EFI")
        .join("BOOT")
        .join("BOOTX64.EFI");

    if !source.exists() {
        bail!(
            "GRUB loader does not exist: {}",
            source.display()
        );
    }

    fs::copy(&source, &fallback)
        .context("Failed to create UEFI fallback loader")?;

    Ok(())
}

fn unmount_efi_partition(efi_directory: &Path) -> Result<()> {
    logln!(
        "{}",
        "Unmounting EFI partition...".cyan()
    );

    let tgt = CString::new(efi_directory.as_os_str().as_bytes())?;

    let ret = unsafe { libc::umount(tgt.as_ptr()) };

    if ret != 0 {
        bail!(
            "Failed to unmount EFI partition: {}",
            std::io::Error::last_os_error()
        );
    }

    Ok(())
}

pub fn install_uefi(
    mount_point: &Path,
    disk: &Path,
) -> Result<()> {
    logln!(
        "{}",
        "Setting up UEFI bootloader...".cyan().bold()
    );

    check_uefi_environment()?;

    logln!(
        "{}",
        "Searching for EFI System Partition...".cyan()
    );

    let efi_partition = find_efi_partition(disk)?;

    logln!(
        "{}",
        format!(
            "EFI System Partition found: {}",
            efi_partition.display()
        )
        .green()
        .bold()
    );

    let efi_directory =
        mount_efi_partition(&efi_partition, mount_point)?;

    verify_efi_mount(&efi_directory)?;

    prepare_efi_directories(&efi_directory)?;

    install_grub(mount_point, &efi_directory)?;

    verify_grub_installation(&efi_directory)?;

    create_fallback_loader(&efi_directory)?;

    unmount_efi_partition(&efi_directory)?;

    logln!(
        "{}",
        "UEFI bootloader installed successfully!"
            .green()
            .bold()
    );

    Ok(())
}