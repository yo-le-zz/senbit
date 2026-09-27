use std::process::Command;

use anyhow::Result;

use crate::common::*;

const UTIL_LINUX_REPO: &str = "https://github.com/util-linux/util-linux.git";

pub fn run(p: &Paths) -> Result<()> {
    let dir = p.join("third_party/util-linux");
    let config = p.join("config/util-linux/version");

    let version = read_version_file(&config)?;

    info("util-linux");
    detail(format!("Version: {version}"));
    detail(format!("Directory: {}", dir.display()));

    if dir.join(".git").exists() {
        let current = describe_tags(&dir);
        detail(format!("Current: {current}"));

        if current == format!("v{version}") {
            ok("util-linux is already up to date.");
            println!();
            println!("util-linux:");
            println!("  {}", dir.display());
            return Ok(());
        }

        println!();
        info("Updating util-linux...");
        detail(format!("{current} -> v{version}"));

        let status = git_status_porcelain(&dir);
        if !status.is_empty() {
            println!();
            error("util-linux source tree contains local modifications:");
            println!();
            println!("{status}");
            std::process::exit(1);
        }

        crate::common::run(Command::new("git").args(["-C", &dir.to_string_lossy(), "fetch", "--tags", "--prune", "origin"]))?;
        crate::common::run(Command::new("git").args(["-C", &dir.to_string_lossy(), "checkout", "--detach", &format!("v{version}")]))?;

        ok("util-linux updated.");
        println!();
        println!("util-linux:");
        println!("  {}", dir.display());
        return Ok(());
    }

    if dir.exists() {
        error(format!(
            "util-linux directory already exists but is not a Git repository:\n  {}",
            dir.display()
        ));
        std::process::exit(1);
    }

    println!();
    info("Downloading util-linux...");
    detail(format!("Repository: {UTIL_LINUX_REPO}"));
    detail(format!("Version:    v{version}"));

    let spin = spinner(&format!("Cloning util-linux v{version} (depth 1)..."));
    let result = crate::common::run(Command::new("git").args([
        "clone",
        "--branch",
        &format!("v{version}"),
        "--depth",
        "1",
        UTIL_LINUX_REPO,
        &dir.to_string_lossy(),
    ]));
    spin.finish_and_clear();
    result?;

    let exact = describe_tags_exact(&dir);
    if exact != format!("v{version}") {
        error("downloaded util-linux version does not match requested version.");
        std::process::exit(1);
    }

    println!();
    ok("util-linux downloaded.");
    println!();
    println!("util-linux:");
    println!("  {}", dir.display());

    Ok(())
}
