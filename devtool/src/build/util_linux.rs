//! util-linux (fdisk/sfdisk statiques) — équivalent de check_util_linux()/
//! build_util_linux() dans build.sh. Sources attendues comme sous-module git,
//! épinglées sur le tag de config/util-linux/version.

use std::path::Path;
use std::process::Command;

use anyhow::{bail, Result};

use crate::cache::Cache;
use crate::paths::Paths;
use crate::proc::{capture, run_quiet};
use crate::{gitutil, ui};

fn read_version(p: &Paths) -> Result<String> {
    let file = p.util_linux_version_file();
    if !file.is_file() {
        bail!("util-linux version file not found:\n  {}", file.display());
    }
    let v: String = std::fs::read_to_string(&file)?
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect();
    if v.is_empty() {
        bail!("util-linux version is empty.");
    }
    Ok(v)
}

fn check(p: &Paths) -> Result<String> {
    let dir = p.util_linux_dir();
    if !gitutil::is_repo(&dir) {
        bail!(
            "util-linux source tree not found:\n  {}\n\nRun:\n  devtool get util-linux",
            dir.display()
        );
    }

    let version = read_version(p)?;
    let current = gitutil::describe_exact(&dir);

    ui::info("util-linux");
    ui::detail(format!("Requested: v{version}"));
    ui::detail(format!("Current:   {current}"));

    if current != format!("v{version}") {
        bail!("util-linux is not on the pinned v{version}.\n\nRun:\n  devtool get util-linux");
    }
    Ok(version)
}

/// A Git checkout of util-linux has no `configure` script: autogen.sh generates it
/// (autoconf, automake, libtool, autopoint, bison, flex... see `devtool doctor`).
/// It runs again when `configure` is older than `configure.ac` (new version, patch).
fn ensure_configure(dir: &Path) -> Result<()> {
    let configure = dir.join("configure");
    let configure_ac = dir.join("configure.ac");

    let up_to_date = configure.is_file()
        && match (
            std::fs::metadata(&configure).and_then(|m| m.modified()),
            std::fs::metadata(&configure_ac).and_then(|m| m.modified()),
        ) {
            (Ok(generated), Ok(source)) => generated >= source,
            _ => true,
        };
    if up_to_date {
        return Ok(());
    }

    let autogen = dir.join("autogen.sh");
    if !autogen.is_file() {
        bail!(
            "util-linux: neither `configure` nor `autogen.sh` found in:\n  {}\n\nThe source tree is incomplete: run `devtool get util-linux`.",
            dir.display()
        );
    }

    ui::info("Generating util-linux's configure script (autogen.sh)...");
    let spin = ui::spinner("Running autogen.sh (autoreconf)...");
    let result = run_quiet(Command::new("sh").arg(&autogen).current_dir(dir));
    spin.finish_and_clear();
    result.map_err(|e| {
        e.context("autogen.sh failed: install autoconf, automake, libtool, gettext (autopoint), bison and flex (`devtool doctor`)")
    })?;

    if !configure.is_file() {
        bail!("autogen.sh ran but did not produce {}", configure.display());
    }
    Ok(())
}

pub fn build(p: &Paths, cache: &mut Cache, jobs: usize) -> Result<()> {
    let build_dir = p.util_linux_build_dir();
    std::fs::create_dir_all(&build_dir)?;

    let version = check(p)?;
    let dir = p.util_linux_dir();
    crate::patches::apply(p, "util-linux", "util-linux", &dir)?;
    // After the patches: they may touch configure.ac / Makefile.am.
    ensure_configure(&dir)?;
    let fdisk = p.util_linux_fdisk();
    let sfdisk = p.util_linux_sfdisk();
    let fp = format!(
        "v{version}:patches={}",
        crate::patches::fingerprint(p, "util-linux")
    );

    if cache.is_fresh("util-linux", &fp, &[&fdisk, &sfdisk]) {
        println!();
        ui::skip("util-linux already built for this version - skipping compilation.");
        ui::detail(format!("fdisk:  {}", fdisk.display()));
        ui::detail(format!("sfdisk: {}", sfdisk.display()));
        return Ok(());
    }

    println!();
    ui::info("Preparing util-linux...");

    {
        ui::info("Configuring util-linux...");
        run_quiet(
            Command::new(dir.join("configure"))
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
                    "--enable-libblkid",
                    "--enable-static",
                    "--enable-static-programs=blkid,fdisk,sfdisk",
                ]),
        )?;
    }

    println!();
    ui::info("Building static util-linux tools...");
    ui::detail(format!("Jobs: {jobs}"));

    let spin = ui::spinner("Compiling util-linux (fdisk, sfdisk)...");
    let result = run_quiet(
        Command::new("make")
            .arg("-C")
            .arg(&build_dir)
            .args(["libblkid.la", "fdisk.static", "sfdisk.static"])
            .arg(format!("-j{jobs}")),
    );
    spin.finish_and_clear();
    result?;

    if !fdisk.is_file() {
        bail!(
            "util-linux fdisk.static was not produced:\n  {}",
            fdisk.display()
        );
    }
    if !sfdisk.is_file() {
        bail!(
            "util-linux sfdisk.static was not produced:\n  {}",
            sfdisk.display()
        );
    }

    println!();
    ui::info("Verifying static binaries...");
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

    cache.record("util-linux", &fp)?;
    Ok(())
}
