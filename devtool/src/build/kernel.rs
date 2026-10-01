//! Noyau Linux — équivalent de update_linux()/build_kernel() dans build.sh.
//!
//! Depuis la dernière version de build.sh, update_linux() ne récupère plus la
//! dernière version toute seule : elle se contente de la version déjà clonée.
//! Seul `devtool get linux` (getlinux.sh) gère l'épinglage de version.

use std::process::Command;

use anyhow::{bail, Result};

use crate::cache::{git_fingerprint, Cache};
use crate::paths::Paths;
use crate::proc::run_quiet;
use crate::{gitutil, ui};

fn update(p: &Paths) -> Result<()> {
    let dir = p.kernel_dir();
    if !gitutil::is_repo(&dir) {
        bail!(
            "Linux source tree not found:\n  {}\n\nRun:\n  devtool get linux",
            dir.display()
        );
    }

    let current = gitutil::describe(&dir);
    ui::info("Linux version");
    ui::detail(format!("Current: {current}"));
    ui::detail("Using checked-out version.");
    Ok(())
}

pub fn build(p: &Paths, cache: &mut Cache, jobs: usize) -> Result<()> {
    let build_dir = p.kernel_build_dir();
    std::fs::create_dir_all(&build_dir)?;

    update(p)?;

    let dir = p.kernel_dir();
    let image = p.kernel_image();
    let fp = git_fingerprint(&dir);

    if cache.is_fresh("kernel", &fp, &[&image]) {
        println!();
        ui::skip("Kernel sources unchanged since last build - skipping compilation.");
        ui::detail(format!("Kernel: {}", image.display()));
        return Ok(());
    }

    println!();
    ui::info("Preparing Linux kernel...");
    let o = format!("O={}", build_dir.display());

    if !build_dir.join(".config").is_file() {
        ui::info("Creating x86_64 kernel configuration...");
        run_quiet(Command::new("make").arg("-C").arg(&dir).arg(&o).arg("x86_64_defconfig"))?;
    }

    println!();
    ui::info("Updating kernel configuration...");
    run_quiet(Command::new("make").arg("-C").arg(&dir).arg(&o).arg("olddefconfig"))?;

    println!();
    ui::info("Building Linux kernel...");
    ui::detail(format!("Jobs: {jobs}"));
    ui::detail("Incremental build enabled.");

    let spin = ui::spinner("Compiling Linux kernel (this can take a while)...");
    let result = run_quiet(Command::new("make").arg("-C").arg(&dir).arg(&o).arg(format!("-j{jobs}")));
    spin.finish_and_clear();
    result?;

    if !image.is_file() {
        bail!("kernel image was not produced:\n  {}", image.display());
    }

    println!();
    println!("Kernel:");
    println!("  {}", image.display());

    cache.record("kernel", &fp)?;
    Ok(())
}
