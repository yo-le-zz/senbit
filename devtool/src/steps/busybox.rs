use std::path::PathBuf;
use std::process::Command;

use anyhow::{bail, Result};

use crate::common::*;

const BUSYBOX_REPO: &str = "https://git.busybox.net/busybox";

fn busybox_dir(p: &Paths) -> PathBuf {
    p.join("third_party/busybox")
}
fn busybox_build_dir(p: &Paths) -> PathBuf {
    p.join("build/busybox")
}
fn busybox_binary(p: &Paths) -> PathBuf {
    busybox_build_dir(p).join("_install/bin/busybox")
}
fn busybox_version_file(p: &Paths) -> PathBuf {
    p.join("config/busybox/version")
}
fn busybox_config(p: &Paths) -> PathBuf {
    p.join("config/busybox.config")
}
fn busybox_patch_dir(p: &Paths) -> PathBuf {
    p.join("third_party/patches/busybox")
}

fn get_latest_busybox_version() -> Result<String> {
    let output = capture(Command::new("git").args(["ls-remote", "--tags", "--refs", BUSYBOX_REPO]))?;

    let mut versions: Vec<String> = output
        .lines()
        .filter_map(|line| line.split_whitespace().nth(1))
        .filter_map(|r| r.strip_prefix("refs/tags/"))
        .filter(|v| is_busybox_tag(v))
        .map(|v| v.to_string())
        .collect();

    if versions.is_empty() {
        bail!("unable to determine latest BusyBox version");
    }

    versions.sort_by(|a, b| version_key(a).cmp(&version_key(b)));
    Ok(versions.pop().unwrap())
}

fn is_busybox_tag(v: &str) -> bool {
    // format attendu : 1_36_1
    let parts: Vec<&str> = v.split('_').collect();
    parts.len() == 3 && parts.iter().all(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_digit()))
}

fn version_key(v: &str) -> Vec<u32> {
    v.split('_').map(|p| p.parse().unwrap_or(0)).collect()
}

fn update_busybox(p: &Paths) -> Result<()> {
    let dir = busybox_dir(p);
    require_git_repo(&dir, "./scripts/getbusy.sh (or `senbit build busybox`)")?;

    let latest = get_latest_busybox_version()?;
    let current = describe_tags(&dir);

    info("BusyBox version");
    detail(format!("Current: {current}"));
    detail(format!("Latest:  {latest}"));

    if current == latest {
        ok("BusyBox is already up to date.");
        return Ok(());
    }

    println!();
    info("Updating BusyBox...");
    detail(format!("{current} -> {latest}"));

    let status = git_status_porcelain(&dir);
    if !status.is_empty() {
        println!();
        error("BusyBox source tree contains local modifications:");
        println!();
        println!("{status}");
        println!();
        println!("Senbit patches must be stored in:");
        println!("  third_party/patches/busybox/");
        println!();
        println!("The working tree must be clean before updating BusyBox.");
        std::process::exit(1);
    }

    run(Command::new("git").args(["-C", &dir.to_string_lossy(), "fetch", "--tags", "--prune", "origin"]))?;
    run(Command::new("git").args(["-C", &dir.to_string_lossy(), "checkout", "--detach", &latest]))?;

    let version = latest.replace('_', ".");
    std::fs::write(busybox_version_file(p), format!("{version}\n"))?;
    ok("BusyBox updated.");
    Ok(())
}

fn apply_busybox_patches(p: &Paths) -> Result<()> {
    let patch_dir = busybox_patch_dir(p);
    if !patch_dir.is_dir() {
        info("No BusyBox patches directory.");
        return Ok(());
    }

    let mut patches: Vec<PathBuf> = std::fs::read_dir(&patch_dir)?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("patch"))
        .collect();
    patches.sort();

    if patches.is_empty() {
        info("No BusyBox patches to apply.");
        return Ok(());
    }

    println!();
    info("Applying Senbit BusyBox patches...");

    let dir = busybox_dir(p);

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
        error(format!("BusyBox patch cannot be applied:\n  {}", patch.display()));
        println!();
        println!("BusyBox source version:");
        println!("{}", describe_tags(&dir));
        println!();
        println!("The patch may need to be updated for this BusyBox version.");
        std::process::exit(1);
    }

    println!();
    ok("BusyBox patches applied.");
    Ok(())
}

pub fn build_busybox(p: &Paths, cache: &mut StateCache) -> Result<()> {
    let build_dir = busybox_build_dir(p);
    std::fs::create_dir_all(&build_dir)?;

    update_busybox(p)?;
    apply_busybox_patches(p)?;

    let config = busybox_config(p);
    if !config.is_file() {
        bail!("BusyBox configuration not found:\n  {}", config.display());
    }

    let dir = busybox_dir(p);
    let fingerprint = tree_fingerprint(&dir);
    let binary = busybox_binary(p);

    if cache.is_up_to_date("busybox", &fingerprint, &binary) {
        println!();
        ok("BusyBox sources unchanged since last build - skipping compilation.");
        detail(format!("BusyBox: {}", binary.display()));
        return Ok(());
    }

    println!();
    info("Preparing BusyBox...");

    if !build_dir.join(".config").is_file() {
        info("Installing Senbit BusyBox configuration...");
        std::fs::copy(&config, build_dir.join(".config"))?;
    }

    println!();
    let jobs = nproc();
    info("Building BusyBox...");
    detail(format!("Jobs: {jobs}"));
    detail("Incremental build enabled.");

    let spin = spinner("Compiling BusyBox...");
    let result = run(Command::new("make").args([
        "-C",
        &dir.to_string_lossy(),
        &format!("O={}", build_dir.display()),
        &format!("-j{jobs}"),
    ]));
    spin.finish_and_clear();
    result?;

    println!();
    info("Installing BusyBox...");
    run(Command::new("make").args([
        "-C",
        &dir.to_string_lossy(),
        &format!("O={}", build_dir.display()),
        &format!("CONFIG_PREFIX={}", build_dir.join("_install").display()),
        "install",
    ]))?;

    if !binary.is_file() {
        bail!("BusyBox binary was not produced:\n  {}", binary.display());
    }

    println!();
    println!("BusyBox:");
    println!("  {}", binary.display());

    cache.mark_built("busybox", &fingerprint)?;
    Ok(())
}

pub fn busybox_install_dir(p: &Paths) -> PathBuf {
    busybox_build_dir(p).join("_install")
}
