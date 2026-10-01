//! ISO bootable GRUB — équivalent de build_iso() dans build.sh.

use std::fs;
use std::process::Command;

use anyhow::{bail, Result};

use crate::paths::Paths;
use crate::proc::run_quiet;
use crate::ui;

const GRUB_CFG: &str = r#"set timeout=3
set default=0
terminal_output console

menuentry "Senbit" {
    linux /boot/bzImage console=ttyS0,115200 console=tty0 quiet loglevel=3
    initrd /boot/initramfs.cpio.gz
}
"#;

pub fn build(p: &Paths) -> Result<()> {
    let kernel_image = p.kernel_image();
    let initramfs = p.initramfs();

    if !kernel_image.is_file() {
        bail!("kernel image not found:\n  {}", kernel_image.display());
    }
    if !initramfs.is_file() {
        bail!("initramfs not found:\n  {}", initramfs.display());
    }

    let iso_build_dir = p.iso_build_dir();
    fs::create_dir_all(&iso_build_dir)?;
    let iso_root = iso_build_dir.join("root");

    ui::info("Preparing ISO...");
    let _ = fs::remove_dir_all(&iso_root);
    fs::create_dir_all(iso_root.join("boot/grub"))?;

    fs::copy(&kernel_image, iso_root.join("boot/bzImage"))?;
    fs::copy(&initramfs, iso_root.join("boot/initramfs.cpio.gz"))?;
    fs::write(iso_root.join("boot/grub/grub.cfg"), GRUB_CFG)?;

    ui::info("Creating bootable ISO...");
    let iso_image = p.iso_image();

    let spin = ui::spinner("Running grub-mkrescue...");
    let result = run_quiet(Command::new("grub-mkrescue").arg("-o").arg(&iso_image).arg(&iso_root));
    spin.finish_and_clear();
    result?;

    let dist = p.iso_dist();
    fs::create_dir_all(dist.parent().unwrap())?;
    fs::copy(&iso_image, &dist)?;

    println!();
    println!("ISO:");
    println!("  {}", iso_image.display());
    println!();
    println!("Distribution ISO:");
    println!("  {}", dist.display());

    Ok(())
}
