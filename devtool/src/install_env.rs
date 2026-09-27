use std::process::Command;
use std::time::Instant;

use anyhow::{bail, Result};

use crate::common::*;
use crate::getlinux;

const PACKAGES: &[&str] = &[
    "build-essential",
    "bc",
    "bison",
    "flex",
    "libelf-dev",
    "libssl-dev",
    "libncurses-dev",
    "libreadline-dev",
    "libuuid-dev",
    "pkg-config",
    "gettext",
    "gperf",
    "texinfo",
    "dwarves",
    "cpio",
    "gzip",
    "xz-utils",
    "bzip2",
    "git",
    "wget",
    "curl",
    "rsync",
    "file",
    "kbd",
    "python3",
    "xorriso",
    "grub-pc-bin",
    "grub-common",
    "mtools",
    "qemu-system-x86",
    "qemu-utils",
    "rustc",
    "cargo",
];

pub fn run(paths: &Paths) -> Result<()> {
    let start = Instant::now();

    banner("Senbit Build Environment");
    println!();

    // # équivalent de `if [[ "${EUID}" -eq 0 ]]`
    let euid = unsafe { libc_geteuid() };
    if euid == 0 {
        error("do not run install as root.");
        println!("Run it as your normal user.");
        std::process::exit(1);
    }

    info("Updating package lists...");
    crate::common::run(Command::new("sudo").args(["apt-get", "update"]))?;

    println!();
    info("Installing/updating build dependencies...");

    let mut cmd = Command::new("sudo");
    cmd.args(["apt-get", "install", "-y"]);
    cmd.args(PACKAGES);
    crate::common::run(&mut cmd)?;

    println!();
    info("Checking Linux kernel...");

    getlinux::run(paths)?;

    let elapsed = start.elapsed().as_secs();

    banner("Senbit environment ready");
    println!();
    println!("Time: {}", format_time(elapsed));
    println!();
    println!("No compilation was performed.");
    println!();
    println!("Build Senbit with:");
    println!("  senbit build");
    println!();

    Ok(())
}

// petit wrapper pour éviter une dépendance directe à `libc` juste pour geteuid()
unsafe fn libc_geteuid() -> u32 {
    #[cfg(unix)]
    {
        extern "C" {
            fn geteuid() -> u32;
        }
        geteuid()
    }
    #[cfg(not(unix))]
    {
        1000
    }
}

#[allow(dead_code)]
fn _never_used() -> Result<()> {
    bail!("unused")
}
