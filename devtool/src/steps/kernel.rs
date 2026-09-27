use std::process::Command;

use anyhow::{bail, Result};

use crate::common::*;

const KERNEL_REPO: &str = "https://git.kernel.org/pub/scm/linux/kernel/git/torvalds/linux.git";

fn kernel_dir(p: &Paths) -> std::path::PathBuf {
    p.join("kernel/linux")
}

fn kernel_build_dir(p: &Paths) -> std::path::PathBuf {
    p.join("build/kernel")
}

fn kernel_image(p: &Paths) -> std::path::PathBuf {
    kernel_build_dir(p).join("arch/x86/boot/bzImage")
}

fn kernel_version_file(p: &Paths) -> std::path::PathBuf {
    p.join("config/kernel/version")
}

/// Équivalent de get_latest_linux_version() : interroge les tags distants et
/// garde le plus récent au format vX.Y[.Z].
fn get_latest_linux_version() -> Result<String> {
    let output = capture(Command::new("git").args([
        "ls-remote",
        "--tags",
        "--refs",
        KERNEL_REPO,
        "refs/tags/v[0-9]*",
    ]))?;

    let mut versions: Vec<String> = output
        .lines()
        .filter_map(|line| line.split_whitespace().nth(1))
        .filter_map(|r| r.strip_prefix("refs/tags/"))
        .filter(|v| is_kernel_version_tag(v))
        .map(|v| v.to_string())
        .collect();

    if versions.is_empty() {
        bail!("unable to determine latest Linux version");
    }

    versions.sort_by(|a, b| version_key(a).cmp(&version_key(b)));
    Ok(versions.pop().unwrap())
}

fn is_kernel_version_tag(v: &str) -> bool {
    // vX.Y ou vX.Y.Z uniquement
    let Some(rest) = v.strip_prefix('v') else { return false };
    let parts: Vec<&str> = rest.split('.').collect();
    if parts.len() != 2 && parts.len() != 3 {
        return false;
    }
    parts.iter().all(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_digit()))
}

fn version_key(v: &str) -> Vec<u32> {
    v.trim_start_matches('v')
        .split('.')
        .map(|p| p.parse().unwrap_or(0))
        .collect()
}

fn update_linux(p: &Paths) -> Result<()> {
    let dir = kernel_dir(p);
    require_git_repo(&dir, "./scripts/getlinux.sh (or `senbit getlinux`)")?;

    let latest = get_latest_linux_version()?;
    let current = describe_tags(&dir);

    info("Linux version");
    detail(format!("Current: {current}"));
    detail(format!("Latest:  {latest}"));

    if current == latest {
        ok("Linux is already up to date.");
        return Ok(());
    }

    println!();
    info("Updating Linux kernel...");
    detail(format!("{current} -> {latest}"));

    run(Command::new("git").args(["-C", &dir.to_string_lossy(), "fetch", "--tags", "--prune", "origin"]))?;
    run(Command::new("git").args(["-C", &dir.to_string_lossy(), "checkout", "--detach", &latest]))?;

    std::fs::write(kernel_version_file(p), format!("{latest}\n"))?;
    ok("Linux updated.");
    Ok(())
}

pub fn build_kernel(p: &Paths, cache: &mut StateCache) -> Result<()> {
    let build_dir = kernel_build_dir(p);
    std::fs::create_dir_all(&build_dir)?;

    update_linux(p)?;

    let dir = kernel_dir(p);
    let fingerprint = tree_fingerprint(&dir);
    let image = kernel_image(p);

    if cache.is_up_to_date("kernel", &fingerprint, &image) {
        println!();
        ok("Kernel sources unchanged since last build - skipping compilation.");
        detail(format!("Kernel: {}", image.display()));
        return Ok(());
    }

    println!();
    info("Preparing Linux kernel...");

    if !build_dir.join(".config").is_file() {
        info("Creating x86_64 kernel configuration...");
        run(Command::new("make")
            .args(["-C", &dir.to_string_lossy(), &format!("O={}", build_dir.display()), "x86_64_defconfig"]))?;
    }

    println!();
    info("Updating kernel configuration...");
    run(Command::new("make")
        .args(["-C", &dir.to_string_lossy(), &format!("O={}", build_dir.display()), "olddefconfig"]))?;

    println!();
    let jobs = nproc();
    info("Building Linux kernel...");
    detail(format!("Jobs: {jobs}"));
    detail("Incremental build enabled.");

    let spin = spinner("Compiling Linux kernel (this can take a while)...");
    let result = run(Command::new("make").args([
        "-C",
        &dir.to_string_lossy(),
        &format!("O={}", build_dir.display()),
        &format!("-j{jobs}"),
    ]));
    spin.finish_and_clear();
    result?;

    if !image.is_file() {
        bail!("kernel image was not produced:\n  {}", image.display());
    }

    println!();
    println!("Kernel:");
    println!("  {}", image.display());

    cache.mark_built("kernel", &fingerprint)?;
    Ok(())
}

pub fn kernel_image_path(p: &Paths) -> std::path::PathBuf {
    kernel_image(p)
}
