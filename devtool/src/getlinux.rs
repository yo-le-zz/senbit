use std::process::Command;

use anyhow::Result;

use crate::common::*;

const KERNEL_REPO: &str = "https://git.kernel.org/pub/scm/linux/kernel/git/torvalds/linux.git";

pub fn run(p: &Paths) -> Result<()> {
    let kernel_dir = p.join("kernel/linux");
    let config = p.join("config/kernel/version");

    let version = read_version_file(&config)?;

    banner("Senbit Linux");
    detail(format!("Requested version: {version}"));
    println!();

    if !kernel_dir.join(".git").exists() {
        info("Linux source tree not found.");
        info("Cloning Linux...");

        std::fs::create_dir_all(p.join("kernel"))?;

        let spin = spinner(&format!("Cloning Linux {version} (depth 1)..."));
        let result = run_git_clone(&version, &kernel_dir);
        spin.finish_and_clear();
        result?;

        println!();
        ok(format!("Linux {version} downloaded."));
        return Ok(());
    }

    let current = describe_tags(&kernel_dir);

    if current == version {
        ok(format!("Linux {version} is already installed."));
        return Ok(());
    }

    info(format!("Current kernel: {current}"));
    info(format!("Updating to:    {version}"));
    println!();

    crate::common::run(Command::new("git").args(["-C", &kernel_dir.to_string_lossy(), "fetch", "--tags", "--prune", "origin"]))?;
    crate::common::run(Command::new("git").args(["-C", &kernel_dir.to_string_lossy(), "checkout", "--detach", &version]))?;

    println!();
    ok("Linux kernel updated.");
    detail(format!("Version: {}", describe_tags(&kernel_dir)));
    let commit = capture(Command::new("git").args(["-C", &kernel_dir.to_string_lossy(), "rev-parse", "HEAD"]))?;
    detail(format!("Commit:  {commit}"));

    Ok(())
}

fn run_git_clone(version: &str, dest: &std::path::Path) -> Result<()> {
    crate::common::run(Command::new("git").args([
        "clone",
        "--branch",
        version,
        "--depth",
        "1",
        KERNEL_REPO,
        &dest.to_string_lossy(),
    ]))
}
