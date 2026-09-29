//! `devtool install` — équivalent natif de install.sh.

use std::process::Command;
use std::time::Instant;

use anyhow::{bail, Result};

use crate::paths::Paths;
use crate::proc::run;
use crate::ui::{self, format_time};

const PACKAGES: &[&str] = &[
    "build-essential", "bc", "bison", "flex", "libelf-dev", "libssl-dev",
    "libncurses-dev", "libreadline-dev", "libuuid-dev", "pkg-config",
    "gettext", "gperf", "texinfo", "dwarves", "cpio", "gzip", "xz-utils",
    "bzip2", "git", "wget", "curl", "rsync", "file", "kbd", "python3",
    "xorriso", "grub-pc-bin", "grub-common", "mtools", "qemu-system-x86",
    "qemu-utils", "rustc", "cargo",
];

pub fn run_install(p: &Paths) -> Result<()> {
    let start = Instant::now();

    ui::banner("Senbit Build Environment");
    println!();

    if unsafe { geteuid() } == 0 {
        bail!("do not run `devtool install` as root.\nRun it as your normal user.");
    }

    ui::info("Updating package lists...");
    run(Command::new("sudo").args(["apt-get", "update"]))?;

    println!();
    ui::info("Installing/updating build dependencies...");
    let mut cmd = Command::new("sudo");
    cmd.args(["apt-get", "install", "-y"]).args(PACKAGES);
    run(&mut cmd)?;

    println!();
    ui::info("Checking Linux kernel...");
    crate::get::run(p, crate::get::Component::Linux)?;

    let elapsed = start.elapsed().as_secs();
    ui::banner("Senbit environment ready");
    println!();
    println!("Time: {}", format_time(elapsed));
    println!();
    println!("No compilation was performed.");
    println!();
    println!("Build Senbit with:");
    println!("  devtool build");
    println!();

    Ok(())
}

unsafe fn geteuid() -> u32 {
    extern "C" {
        fn geteuid() -> u32;
    }
    geteuid()
}
