use std::time::Instant;

use anyhow::{bail, Result};

use crate::common::*;
use crate::steps::{busybox, iso, kernel, parted, rootfs, rust_build, tools, util_linux};

#[derive(Debug, Clone, Copy)]
pub enum BuildTarget {
    All,
    Kernel,
    Busybox,
    UtilLinux,
    Parted,
    Rust,
    Tools,
    Rootfs,
    Iso,
}

impl BuildTarget {
    pub fn parse(s: &str) -> Result<Self> {
        Ok(match s {
            "all" => Self::All,
            "kernel" => Self::Kernel,
            "busybox" => Self::Busybox,
            "util-linux" => Self::UtilLinux,
            "parted" => Self::Parted,
            "rust" => Self::Rust,
            "tools" => Self::Tools,
            "rootfs" => Self::Rootfs,
            "iso" => Self::Iso,
            other => bail!(
                "unknown build target: {other}\n\nUsage:\n  senbit build\n  senbit build all\n  senbit build kernel\n  senbit build busybox\n  senbit build util-linux\n  senbit build parted\n  senbit build rust\n  senbit build tools\n  senbit build rootfs\n  senbit build iso"
            ),
        })
    }
}

pub fn run(paths: &Paths, target: BuildTarget) -> Result<()> {
    let total_start = Instant::now();
    let mut cache = StateCache::open(paths)?;

    let result = (|| -> Result<()> {
        use BuildTarget::*;
        match target {
            Kernel => run_step("Linux Kernel", || kernel::build_kernel(paths, &mut cache)),
            Busybox => run_step("BusyBox", || busybox::build_busybox(paths, &mut cache)),
            UtilLinux => run_step("util-linux", || util_linux::build_util_linux(paths, &mut cache)),
            Parted => run_step("GNU Parted", || parted::build_parted(paths, &mut cache)),
            Rust => run_step("Senbit Rust Userspace", || rust_build::build_rust(paths, &mut cache)),
            Tools => run_step("Senbit Build Tools", || tools::build_tools(paths)),
            Rootfs => run_step("Senbit Root Filesystem", || rootfs::build_rootfs(paths)),
            Iso => run_step("Senbit ISO", || iso::build_iso(paths)),
            All => {
                run_step("Linux Kernel", || kernel::build_kernel(paths, &mut cache))?;
                run_step("BusyBox", || busybox::build_busybox(paths, &mut cache))?;
                run_step("util-linux", || util_linux::build_util_linux(paths, &mut cache))?;
                run_step("GNU Parted", || parted::build_parted(paths, &mut cache))?;
                run_step("Senbit Rust Userspace", || rust_build::build_rust(paths, &mut cache))?;
                run_step("Senbit Build Tools", || tools::build_tools(paths))?;
                run_step("Senbit Root Filesystem", || rootfs::build_rootfs(paths))?;
                run_step("Senbit ISO", || iso::build_iso(paths))?;
                Ok(())
            }
        }
    })();

    let total_elapsed = total_start.elapsed().as_secs();

    banner("Senbit Build Complete");
    println!();
    println!("Total time: {}", format_time(total_elapsed));
    println!();

    result
}
