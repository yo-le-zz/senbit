use std::fs;
use std::io::Write;
use std::os::unix::fs::{symlink, PermissionsExt};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use anyhow::{bail, Result};

use crate::common::*;
use crate::steps::{busybox, parted, rust_build, util_linux};

fn rootfs_dir(p: &Paths) -> PathBuf {
    p.join("build/rootfs")
}

pub fn initramfs_path(p: &Paths) -> PathBuf {
    p.join("build/initramfs.cpio.gz")
}

fn make_executable(path: &Path) -> Result<()> {
    let mut perms = fs::metadata(path)?.permissions();
    perms.set_mode(0o755);
    fs::set_permissions(path, perms)?;
    Ok(())
}

/// Copie les dépendances dynamiques (`ldd`) d'un binaire dans le rootfs -
/// équivalent de la boucle `ldd | grep '=>'` de build.sh.
fn copy_runtime_deps(binary: &Path, rootfs: &Path) -> Result<()> {
    let output = Command::new("ldd").arg(binary).output();
    let Ok(output) = output else { return Ok(()) };
    let text = String::from_utf8_lossy(&output.stdout);

    for line in text.lines() {
        if let Some((_, rhs)) = line.split_once("=>") {
            let lib_path = rhs.trim().split_whitespace().next().unwrap_or("");
            if !lib_path.starts_with('/') {
                continue;
            }
            let src = Path::new(lib_path);
            if !src.is_file() {
                continue;
            }
            let dest = rootfs.join(lib_path.trim_start_matches('/'));
            fs::create_dir_all(dest.parent().unwrap())?;
            fs::copy(src, &dest)?;
            detail(format!("{lib_path}"));
        }

        if line.contains("ld-linux") {
            if let Some(loader) = line.split_whitespace().next() {
                let src = Path::new(loader);
                if src.is_file() {
                    let dest = rootfs.join(loader.trim_start_matches('/'));
                    fs::create_dir_all(dest.parent().unwrap())?;
                    fs::copy(src, &dest)?;
                    detail(format!("{loader}"));
                }
            }
        }
    }

    Ok(())
}

fn copy_tree(src: &Path, dst: &Path) -> Result<()> {
    // rsync -a : on tente rsync, sinon repli sur une copie récursive maison.
    if Command::new("rsync").arg("--version").stdout(Stdio::null()).status().map(|s| s.success()).unwrap_or(false) {
        run(Command::new("rsync").args(["-a", &format!("{}/", src.display()), &format!("{}/", dst.display())]))?;
        return Ok(());
    }

    for entry in walkdir::WalkDir::new(src) {
        let entry = entry?;
        let rel = entry.path().strip_prefix(src)?;
        let target = dst.join(rel);
        if entry.file_type().is_dir() {
            fs::create_dir_all(&target)?;
        } else if entry.file_type().is_file() {
            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::copy(entry.path(), &target)?;
        }
    }
    Ok(())
}

pub fn build_rootfs(p: &Paths) -> Result<()> {
    let rootfs = rootfs_dir(p);
    fs::create_dir_all(&rootfs)?;

    info("Preparing root filesystem...");

    let _ = fs::remove_dir_all(&rootfs);
    for d in ["bin", "sbin", "etc", "proc", "sys", "dev", "run", "tmp", "usr/bin", "usr/sbin"] {
        fs::create_dir_all(rootfs.join(d))?;
    }

    let senbit_rootfs = p.join("rootfs");
    if senbit_rootfs.is_dir() {
        copy_tree(&senbit_rootfs, &rootfs)?;
    }

    info("Installing BusyBox into root filesystem...");
    let busybox_install = busybox::busybox_install_dir(p);
    if !busybox_install.is_dir() {
        bail!("BusyBox installation directory not found:\n  {}", busybox_install.display());
    }
    copy_tree(&busybox_install, &rootfs)?;

    info("Installing util-linux...");
    let fdisk = util_linux::fdisk_binary(p);
    let sfdisk = util_linux::sfdisk_binary(p);
    if !fdisk.is_file() {
        bail!("util-linux fdisk.static not found:\n  {}", fdisk.display());
    }
    if !sfdisk.is_file() {
        bail!("util-linux sfdisk.static not found:\n  {}", sfdisk.display());
    }
    fs::copy(&fdisk, rootfs.join("usr/sbin/fdisk"))?;
    fs::copy(&sfdisk, rootfs.join("usr/sbin/sfdisk"))?;
    make_executable(&rootfs.join("usr/sbin/fdisk"))?;
    make_executable(&rootfs.join("usr/sbin/sfdisk"))?;

    info("Installing GNU Parted...");
    let parted_bin = parted::parted_binary(p);
    if !parted_bin.is_file() {
        bail!("GNU Parted binary not found:\n  {}", parted_bin.display());
    }
    fs::copy(&parted_bin, rootfs.join("usr/sbin/parted"))?;
    make_executable(&rootfs.join("usr/sbin/parted"))?;

    info("Installing GNU Parted runtime dependencies...");
    copy_runtime_deps(&parted_bin, &rootfs)?;

    info("Installing Senbit Rust init...");
    let init_bin = rust_build::senbit_init_binary(p);
    if !init_bin.is_file() {
        bail!("Senbit init binary not found:\n  {}", init_bin.display());
    }
    let _ = fs::remove_file(rootfs.join("init"));
    fs::copy(&init_bin, rootfs.join("init"))?;
    make_executable(&rootfs.join("init"))?;
    let _ = fs::remove_file(rootfs.join("sbin/init"));
    symlink("../init", rootfs.join("sbin/init"))?;

    println!();
    info("Creating initramfs...");

    let initramfs = initramfs_path(p);
    fs::create_dir_all(initramfs.parent().unwrap())?;

    let find = Command::new("find")
        .arg(".")
        .arg("-print0")
        .current_dir(&rootfs)
        .stdout(Stdio::piped())
        .spawn()?;

    let cpio = Command::new("cpio")
        .args(["--null", "-o", "-H", "newc"])
        .current_dir(&rootfs)
        .stdin(Stdio::from(find.stdout.unwrap()))
        .stdout(Stdio::piped())
        .spawn()?;

    let gzip_out = cpio.stdout.ok_or_else(|| anyhow::anyhow!("failed to pipe cpio output"))?;
    let gzip_output = Command::new("gzip").arg("-9").stdin(Stdio::from(gzip_out)).output()?;

    if !gzip_output.status.success() {
        bail!("gzip exited with status: {}", gzip_output.status);
    }

    let mut f = fs::File::create(&initramfs)?;
    f.write_all(&gzip_output.stdout)?;

    println!();
    println!("Initramfs:");
    println!("  {}", initramfs.display());

    Ok(())
}
