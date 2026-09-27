use std::fs;
use std::path::PathBuf;
use std::process::Command;

use anyhow::{bail, Result};

use crate::common::*;
use crate::steps::{kernel, rootfs};

fn iso_build_dir(p: &Paths) -> PathBuf {
    p.join("build/iso")
}

const GRUB_CFG: &str = r#"set timeout=0
set default=0

menuentry "Senbit" {
    linux /boot/bzImage console=tty0 console=ttyS0,115200 loglevel=3
    initrd /boot/initramfs.cpio.gz
}
"#;

pub fn build_iso(p: &Paths) -> Result<()> {
    let kernel_image = kernel::kernel_image_path(p);
    let initramfs = rootfs::initramfs_path(p);

    if !kernel_image.is_file() {
        bail!("kernel image not found:\n  {}", kernel_image.display());
    }
    if !initramfs.is_file() {
        bail!("initramfs not found:\n  {}", initramfs.display());
    }

    let iso_build_dir = iso_build_dir(p);
    fs::create_dir_all(&iso_build_dir)?;

    let iso_root = iso_build_dir.join("root");

    info("Preparing ISO...");

    let _ = fs::remove_dir_all(&iso_root);
    fs::create_dir_all(iso_root.join("boot/grub"))?;

    fs::copy(&kernel_image, iso_root.join("boot/bzImage"))?;
    fs::copy(&initramfs, iso_root.join("boot/initramfs.cpio.gz"))?;
    fs::write(iso_root.join("boot/grub/grub.cfg"), GRUB_CFG)?;

    info("Creating bootable ISO...");

    let iso_image = iso_build_dir.join("senbit.iso");
    let spin = spinner("Running grub-mkrescue...");
    let result = run(Command::new("grub-mkrescue")
        .arg("-o")
        .arg(&iso_image)
        .arg(&iso_root));
    spin.finish_and_clear();
    result?;

    let dist_dir = p.join("iso");
    fs::create_dir_all(&dist_dir)?;
    fs::copy(&iso_image, dist_dir.join("senbit.iso"))?;

    println!();
    println!("ISO:");
    println!("  {}", iso_image.display());
    println!();
    println!("Distribution ISO:");
    println!("  {}", dist_dir.join("senbit.iso").display());

    Ok(())
}
