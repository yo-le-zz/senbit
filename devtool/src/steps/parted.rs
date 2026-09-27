use std::path::PathBuf;
use std::process::Command;

use anyhow::{bail, Result};

use crate::common::*;

fn parted_dir(p: &Paths) -> PathBuf {
    p.join("third_party/parted")
}
fn parted_build_dir(p: &Paths) -> PathBuf {
    p.join("build/parted")
}
fn version_file(p: &Paths) -> PathBuf {
    p.join("config/parted/version")
}
fn patch_dir(p: &Paths) -> PathBuf {
    p.join("third_party/patches/parted")
}
pub fn parted_binary(p: &Paths) -> PathBuf {
    parted_build_dir(p).join("_install/usr/sbin/parted")
}

fn check_parted(p: &Paths) -> Result<String> {
    let dir = parted_dir(p);
    if !dir.join(".git").exists() {
        error(format!("GNU Parted source tree not found:\n  {}", dir.display()));
        println!();
        println!("Initialize Git submodules with:");
        println!("  git submodule update --init --recursive");
        std::process::exit(1);
    }

    let version = read_version_file(&version_file(p))?;
    let current = describe_tags_exact(&dir);

    info("GNU Parted");
    detail(format!("Requested: v{version}"));
    detail(format!("Current:   {current}"));

    if current != format!("v{version}") {
        println!();
        error(format!("GNU Parted submodule is not on v{version}."));
        println!();
        println!("Run:");
        println!("  git -C third_party/parted checkout v{version}");
        std::process::exit(1);
    }

    Ok(version)
}

fn apply_parted_patches(p: &Paths) -> Result<()> {
    let dir_patches = patch_dir(p);
    if !dir_patches.is_dir() {
        info("No GNU Parted patches directory.");
        return Ok(());
    }

    let mut patches: Vec<PathBuf> = std::fs::read_dir(&dir_patches)?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("patch"))
        .collect();
    patches.sort();

    if patches.is_empty() {
        info("No GNU Parted patches to apply.");
        return Ok(());
    }

    println!();
    info("Applying Senbit GNU Parted patches...");

    let dir = parted_dir(p);

    for patch in &patches {
        let patch_name = patch.file_name().unwrap().to_string_lossy().to_string();
        println!();
        detail(format!("Applying: {patch_name}"));

        let check_ok = Command::new("git")
            .args(["-C", &dir.to_string_lossy(), "apply", "--check"])
            .arg(patch)
            .status()
            .map(|s| s.success())
            .unwrap_or(false);

        if check_ok {
            run(Command::new("git").args(["-C", &dir.to_string_lossy(), "apply"]).arg(patch))?;
            continue;
        }

        let reverse_ok = Command::new("git")
            .args(["-C", &dir.to_string_lossy(), "apply", "--reverse", "--check"])
            .arg(patch)
            .status()
            .map(|s| s.success())
            .unwrap_or(false);

        if reverse_ok {
            detail(format!("Already applied: {patch_name}"));
            continue;
        }

        println!();
        error(format!("GNU Parted patch cannot be applied:\n  {}", patch.display()));
        println!();
        println!("GNU Parted source version:");
        println!("{}", describe_tags(&dir));
        println!();
        println!("The patch may need to be updated for this Parted version.");
        std::process::exit(1);
    }

    println!();
    ok("GNU Parted patches applied.");
    Ok(())
}

pub fn build_parted(p: &Paths, cache: &mut StateCache) -> Result<()> {
    let build_dir = parted_build_dir(p);
    std::fs::create_dir_all(&build_dir)?;

    let version = check_parted(p)?;
    apply_parted_patches(p)?;

    let dir = parted_dir(p);
    let binary = parted_binary(p);
    let fingerprint = format!("v{version}");

    if cache.is_up_to_date("parted", &fingerprint, &binary) {
        println!();
        ok("GNU Parted already built for this version - skipping compilation.");
        detail(format!("Parted: {}", binary.display()));
        return Ok(());
    }

    println!();
    info("Preparing GNU Parted...");

    if !dir.join("gnulib").is_dir() {
        println!();
        bail!(
            "GNU Parted gnulib directory not found:\n  {}\n\nGNU Parted requires gnulib to bootstrap.\nRun the Parted source setup script before building.",
            dir.join("gnulib").display()
        );
    }

    if !dir.join("configure").is_file() {
        info("Generating GNU Parted build system...");
        run(Command::new("./bootstrap").current_dir(&dir))?;
    }

    if !build_dir.join("Makefile").is_file() {
        info("Configuring GNU Parted...");
        run(Command::new(dir.join("configure"))
            .current_dir(&build_dir)
            .args([
                "--prefix=/usr",
                "--bindir=/usr/bin",
                "--sbindir=/usr/sbin",
                "--libdir=/usr/lib",
                "--disable-device-mapper",
                "--disable-debug",
            ]))?;
    }

    println!();
    let jobs = nproc();
    info("Building GNU Parted...");
    detail(format!("Jobs: {jobs}"));

    let spin = spinner("Compiling GNU Parted...");
    let result = run(Command::new("make").args(["-C", &build_dir.to_string_lossy(), &format!("-j{jobs}")]));
    spin.finish_and_clear();
    result?;

    println!();
    info("Installing GNU Parted...");
    let install_dir = build_dir.join("_install");
    let _ = std::fs::remove_dir_all(&install_dir);

    run(Command::new("make").args([
        "-C",
        &build_dir.to_string_lossy(),
        &format!("DESTDIR={}", install_dir.display()),
        "install",
    ]))?;

    if !binary.is_file() {
        bail!("GNU Parted binary was not produced:\n  {}", binary.display());
    }

    println!();
    println!("GNU Parted:");
    println!("  {}", binary.display());

    cache.mark_built("parted", &fingerprint)?;
    Ok(())
}
