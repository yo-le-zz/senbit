// ISO bootable GRUB — équivalent de build_iso() dans build.sh.

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
    linux /boot/bzImage console=ttyS0,115200 console=tty0 loglevel=3 net.ifnames=0 biosdevname=0
    initrd /boot/initramfs.cpio.gz
}

menuentry "Senbit (repair an installed system)" {
    linux /boot/bzImage console=ttyS0,115200 console=tty0 loglevel=3 net.ifnames=0 biosdevname=0 senbit.repair=1
    initrd /boot/initramfs.cpio.gz
}
"#;

/// Détecte le nom de l'exécutable GRUB Rescue disponible sur l'hôte.
fn find_grub_mkrescue() -> Result<&'static str> {
    for executable in ["grub-mkrescue", "grub2-mkrescue"] {
        let available = Command::new("sh")
            .arg("-c")
            .arg("command -v \"$1\" >/dev/null 2>&1")
            .arg("sh")
            .arg(executable)
            .status()
            .map(|status| status.success())
            .unwrap_or(false);

        if available {
            return Ok(executable);
        }
    }

    bail!(
        "GRUB Rescue utility not found.\n\
         Install grub2-tools-extra on Fedora or the GRUB rescue tools package \
         on your distribution.\n\
         Expected executable: grub-mkrescue or grub2-mkrescue"
    );
}

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
    let grub_mkrescue = find_grub_mkrescue()?;

    ui::info(&format!("Using {}...", grub_mkrescue));

    let spin = ui::spinner("Running GRUB Rescue...");

    let result = run_quiet(
        Command::new(grub_mkrescue)
            .arg("-o")
            .arg(&iso_image)
            .arg(&iso_root),
    );

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
