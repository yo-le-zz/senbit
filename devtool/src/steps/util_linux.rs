use std::path::PathBuf;
use std::process::Command;

use anyhow::{bail, Result};

use crate::common::*;

fn util_linux_dir(p: &Paths) -> PathBuf {
    p.join("third_party/util-linux")
}
fn util_linux_build_dir(p: &Paths) -> PathBuf {
    p.join("build/util-linux")
}
fn version_file(p: &Paths) -> PathBuf {
    p.join("config/util-linux/version")
}
pub fn fdisk_binary(p: &Paths) -> PathBuf {
    util_linux_build_dir(p).join("fdisk.static")
}
pub fn sfdisk_binary(p: &Paths) -> PathBuf {
    util_linux_build_dir(p).join("sfdisk.static")
}

fn check_util_linux(p: &Paths) -> Result<String> {
    let dir = util_linux_dir(p);
    if !dir.join(".git").exists() {
        error(format!("util-linux source tree not found:\n  {}", dir.display()));
        println!();
        println!("Initialize Git submodules with:");
        println!("  git submodule update --init --recursive");
        std::process::exit(1);
    }

    let version = read_version_file(&version_file(p))?;
    let current = describe_tags_exact(&dir);

    info("util-linux");
    detail(format!("Requested: v{version}"));
    detail(format!("Current:   {current}"));

    if current != format!("v{version}") {
        println!();
        error(format!("util-linux submodule is not on v{version}."));
        println!();
        println!("Run:");
        println!("  git -C third_party/util-linux checkout v{version}");
        std::process::exit(1);
    }

    Ok(version)
}

pub fn build_util_linux(p: &Paths, cache: &mut StateCache) -> Result<()> {
    let build_dir = util_linux_build_dir(p);
    std::fs::create_dir_all(&build_dir)?;

    let version = check_util_linux(p)?;

    let dir = util_linux_dir(p);
    let fingerprint = format!("v{version}");
    let fdisk = fdisk_binary(p);
    let sfdisk = sfdisk_binary(p);

    if cache.is_up_to_date("util-linux", &fingerprint, &fdisk) && sfdisk.is_file() {
        println!();
        ok("util-linux already built for this version - skipping compilation.");
        detail(format!("fdisk:  {}", fdisk.display()));
        detail(format!("sfdisk: {}", sfdisk.display()));
        return Ok(());
    }

    println!();
    info("Preparing util-linux...");

    if !build_dir.join("Makefile").is_file() {
        info("Configuring util-linux...");
        run(Command::new(dir.join("configure"))
            .current_dir(&build_dir)
            .args([
                "--prefix=/usr",
                "--bindir=/usr/bin",
                "--sbindir=/usr/sbin",
                "--libdir=/usr/lib",
                "--disable-all-programs",
                "--enable-fdisks",
                "--enable-libfdisk",
                "--enable-libsmartcols",
                "--enable-libuuid",
                "--enable-static-programs=fdisk,sfdisk",
            ]))?;
    }

    println!();
    let jobs = nproc();
    info("Building static util-linux tools...");
    detail(format!("Jobs: {jobs}"));

    let spin = spinner("Compiling util-linux (fdisk, sfdisk)...");
    let result = run(Command::new("make").args([
        "-C",
        &build_dir.to_string_lossy(),
        "fdisk.static",
        "sfdisk.static",
        &format!("-j{jobs}"),
    ]));
    spin.finish_and_clear();
    result?;

    if !fdisk.is_file() {
        bail!("util-linux fdisk.static was not produced:\n  {}", fdisk.display());
    }
    if !sfdisk.is_file() {
        bail!("util-linux sfdisk.static was not produced:\n  {}", sfdisk.display());
    }

    println!();
    info("Verifying static binaries...");
    for bin in [&fdisk, &sfdisk] {
        let out = capture(Command::new("file").arg(bin))?;
        if out.contains("dynamically linked") {
            bail!("{} is dynamically linked.", bin.display());
        }
    }

    println!();
    println!("util-linux:");
    println!("  fdisk:  {}", fdisk.display());
    println!("  sfdisk: {}", sfdisk.display());

    cache.mark_built("util-linux", &fingerprint)?;
    Ok(())
}
