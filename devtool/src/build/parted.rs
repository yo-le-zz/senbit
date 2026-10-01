//! GNU Parted - equivalent of check_parted()/apply_parted_patches()/build_parted()
//! in build.sh. Git submodule pinned to the tag in config/parted/version.
//!
//! Parted is built as a fully static binary (no readline, no shared
//! libraries) because the Senbit rootfs does not contain a libc.

use std::path::PathBuf;
use std::process::Command;

use anyhow::{bail, Result};

use crate::cache::Cache;
use crate::paths::Paths;
use crate::proc::{run, run_quiet, succeeds};
use crate::{gitutil, ui};

fn read_version(p: &Paths) -> Result<String> {
    let file = p.parted_version_file();
    if !file.is_file() {
        bail!("Parted version file not found:\n  {}", file.display());
    }
    let v: String = std::fs::read_to_string(&file)?.chars().filter(|c| !c.is_whitespace()).collect();
    if v.is_empty() {
        bail!("GNU Parted version is empty.");
    }
    Ok(v)
}

fn check(p: &Paths) -> Result<String> {
    let dir = p.parted_dir();
    if !gitutil::is_repo(&dir) {
        bail!(
            "GNU Parted source tree not found:\n  {}\n\nInitialize Git submodules with:\n  git submodule update --init --recursive",
            dir.display()
        );
    }

    let version = read_version(p)?;
    let current = gitutil::describe_exact(&dir);

    ui::info("GNU Parted");
    ui::detail(format!("Requested: v{version}"));
    ui::detail(format!("Current:   {current}"));

    if current != format!("v{version}") {
        bail!(
            "GNU Parted submodule is not on v{version}.\n\nRun:\n  git -C third_party/parted checkout v{version}"
        );
    }
    Ok(version)
}

fn apply_patches(p: &Paths) -> Result<()> {
    let patch_dir = p.parted_patch_dir();
    if !patch_dir.is_dir() {
        ui::info("No GNU Parted patches directory.");
        return Ok(());
    }

    let mut patches: Vec<PathBuf> = std::fs::read_dir(&patch_dir)?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("patch"))
        .collect();
    patches.sort();

    if patches.is_empty() {
        ui::info("No GNU Parted patches to apply.");
        return Ok(());
    }

    println!();
    ui::info("Applying Senbit GNU Parted patches...");
    let dir = p.parted_dir();

    for patch in &patches {
        let name = patch.file_name().unwrap().to_string_lossy().to_string();
        println!();
        ui::detail(format!("Applying: {name}"));

        if succeeds(Command::new("git").arg("-C").arg(&dir).arg("apply").arg("--check").arg(patch)) {
            run(Command::new("git").arg("-C").arg(&dir).arg("apply").arg(patch))?;
        } else if succeeds(
            Command::new("git").arg("-C").arg(&dir).args(["apply", "--reverse", "--check"]).arg(patch),
        ) {
            ui::detail(format!("Already applied: {name}"));
        } else {
            bail!(
                "GNU Parted patch cannot be applied:\n  {}\n\nGNU Parted source version:\n{}\n\nThe patch may need to be updated for this Parted version.",
                patch.display(),
                gitutil::describe(&dir)
            );
        }
    }

    println!();
    ui::ok("GNU Parted patches applied.");
    Ok(())
}

pub fn build(p: &Paths, cache: &mut Cache, jobs: usize) -> Result<()> {
    let build_dir = p.parted_build_dir();
    std::fs::create_dir_all(&build_dir)?;

    let version = check(p)?;
    apply_patches(p)?;

    let dir = p.parted_dir();
    let binary = p.parted_binary();
    let fp = format!("v{version}");

    if cache.is_fresh("parted", &fp, &[&binary]) {
        println!();
        ui::skip("GNU Parted already built for this version - skipping compilation.");
        ui::detail(format!("Parted: {}", binary.display()));
        return Ok(());
    }

    println!();
    ui::info("Preparing GNU Parted...");

    if !dir.join("gnulib").is_dir() {
        bail!(
            "GNU Parted gnulib directory not found:\n  {}\n\nGNU Parted requires gnulib to bootstrap.\nRun the Parted source setup script before building.",
            dir.join("gnulib").display()
        );
    }

    if !dir.join("configure").is_file() {
        ui::info("Generating GNU Parted build system...");
        run_quiet(Command::new("./bootstrap").current_dir(&dir))?;
    }

    if !build_dir.join("Makefile").is_file() {
        ui::info("Configuring GNU Parted...");
        run_quiet(Command::new(dir.join("configure")).current_dir(&build_dir).args([
            "--prefix=/usr",
            "--bindir=/usr/bin",
            "--sbindir=/usr/sbin",
            "--libdir=/usr/lib",
            "--disable-device-mapper",
            // Do NOT pass --disable-debug: it turns PED_ASSERT() into a no-op,
            // and parted 3.6 (labels/atari.c) calls newlocale() inside a
            // PED_ASSERT, so the locale stays NULL and parted crashes with
            // SIGSEGV in freelocale() when it exits.
            "--enable-debug",
            "--without-readline",
        ]))?;
    }

    println!();
    ui::info("Building GNU Parted...");
    ui::detail(format!("Jobs: {jobs}"));

    let spin = ui::spinner("Compiling GNU Parted...");
    // -all-static makes libtool link a fully static executable.
    let result = run_quiet(Command::new("make")
        .arg("-C")
        .arg(&build_dir)
        .arg(format!("-j{jobs}"))
        .arg("LDFLAGS=-all-static"));
    spin.finish_and_clear();
    result?;

    println!();
    ui::info("Installing GNU Parted...");
    let install = build_dir.join("_install");
    let _ = std::fs::remove_dir_all(&install);
    let spin = ui::spinner("Installing GNU Parted...");
    let result = run_quiet(Command::new("make")
        .arg("-C")
        .arg(&build_dir)
        .arg(format!("DESTDIR={}", install.display()))
        .arg("install"));
    spin.finish_and_clear();
    result?;

    if !binary.is_file() {
        bail!("GNU Parted binary was not produced:\n  {}", binary.display());
    }

    println!();
    println!("GNU Parted:");
    println!("  {}", binary.display());

    cache.record("parted", &fp)?;
    Ok(())
}