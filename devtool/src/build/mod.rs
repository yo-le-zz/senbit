pub mod busybox;
pub mod iso;
pub mod kernel;
pub mod parted;
pub mod rootfs;
pub mod rust_build;
pub mod tools;
pub mod util_linux;
pub mod initramfs;

use std::time::Instant;

use anyhow::Result;
use clap::ValueEnum;

use crate::cache::Cache;
use crate::paths::Paths;
use crate::proc::nproc;
use crate::ui::{self, format_time, run_step};

/// Targets of `devtool build`, identical to those of build.sh, plus `fast`
/// (devtool addition: only rebuilds the Rust userspace, the tools, the
/// rootfs and the ISO - without touching the kernel/BusyBox/util-linux/
/// Parted/GRUB).
#[derive(Debug, Clone, Copy, ValueEnum, PartialEq, Eq)]
#[value(rename_all = "kebab-case")]
pub enum Target {
    All,
    Fast,
    Kernel,
    Busybox,
    UtilLinux,
    Parted,
    Rust,
    Tools,
    Rootfs,
    Iso,
}

/// Resolves the number of make/cargo jobs to use: the value given by the
/// user (`--jobs`), otherwise the detected CPU count (like the `nproc()`
/// of the original scripts).
pub fn resolve_jobs(jobs: Option<usize>) -> usize {
    jobs.filter(|j| *j > 0).unwrap_or_else(nproc)
}

pub fn run(p: &Paths, target: Target, jobs: Option<usize>, force: bool) -> Result<()> {
    let jobs = resolve_jobs(jobs);

    if force {
        ui::warn("--force: clearing the build cache, everything will be recompiled.");
        Cache::clear(&p.state_file())?;
    }

    let total_start = Instant::now();
    let mut cache = Cache::open(p.state_file());

    use Target::*;
    let result = (|| -> Result<()> {
        match target {
            Kernel => run_step("Linux Kernel", || kernel::build(p, &mut cache, jobs)),
            Busybox => run_step("BusyBox", || busybox::build(p, &mut cache, jobs)),
            UtilLinux => run_step("util-linux", || util_linux::build(p, &mut cache, jobs)),
            Parted => run_step("GNU Parted", || parted::build(p, &mut cache, jobs)),
            Rust => run_step("Senbit Rust Userspace", || rust_build::build(p, &mut cache, jobs)),
            Tools => run_step("Senbit Build Tools", || tools::build(p, jobs)),
            Rootfs => run_step("Senbit Root Filesystem", || rootfs::build(p)),
            Iso => run_step("Senbit ISO", || iso::build(p)),
            All => {
                run_step("Linux Kernel", || kernel::build(p, &mut cache, jobs))?;
                run_step("BusyBox", || busybox::build(p, &mut cache, jobs))?;
                run_step("util-linux", || util_linux::build(p, &mut cache, jobs))?;
                run_step("GNU Parted", || parted::build(p, &mut cache, jobs))?;
                run_step("Senbit Rust Userspace", || rust_build::build(p, &mut cache, jobs))?;
                run_step("Senbit Build Tools", || tools::build(p, jobs))?;
                run_step("Senbit Root Filesystem", || rootfs::build(p))?;
                run_step("Senbit ISO", || iso::build(p))?;
                Ok(())
            }
            Fast => {
                ui::banner("Fast build: userspace + tools + rootfs + ISO only");
                ui::warn("Skipping kernel/BusyBox/util-linux/Parted/GRUB checks and rebuilds.");
                run_step("Senbit Rust Userspace", || rust_build::build(p, &mut cache, jobs))?;
                run_step("Senbit Build Tools", || tools::build(p, jobs))?;
                run_step("Senbit Root Filesystem", || rootfs::build(p))?;
                run_step("Senbit ISO", || iso::build(p))?;
                Ok(())
            }
        }
    })();

    let total = total_start.elapsed().as_secs();
    ui::banner("Senbit Build Complete");
    println!();
    println!("Total time: {}", format_time(total));
    println!();

    result
}