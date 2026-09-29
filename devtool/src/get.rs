//! `devtool get <linux|busybox|util-linux>` — équivalent natif de
//! getlinux.sh / getutil.sh, et de la logique de mise à jour BusyBox de
//! build.sh (il n'existait pas de "getbusy.sh" qui récupère réellement les
//! sources : l'ancien getbusy.sh était en fait une ancienne version de
//! build.sh, remplacée depuis).

use anyhow::{bail, Result};
use clap::ValueEnum;

use crate::paths::Paths;
use crate::{gitutil, ui};

#[derive(Debug, Clone, Copy, ValueEnum)]
#[value(rename_all = "kebab-case")]
pub enum Component {
    Linux,
    Busybox,
    UtilLinux,
}

/// Équivalent de getlinux.sh : clone/checkout la version épinglée dans
/// config/kernel/version.
fn get_linux(p: &Paths) -> Result<()> {
    let dir = p.kernel_dir();
    let version_file = p.kernel_version_file();

    if !version_file.is_file() {
        bail!("missing kernel version configuration:\n  {}", version_file.display());
    }
    let version: String = std::fs::read_to_string(&version_file)?.chars().filter(|c| !c.is_whitespace()).collect();
    if version.is_empty() {
        bail!("kernel version configuration is empty.");
    }

    ui::banner("Senbit Linux");
    println!();
    ui::detail(format!("Requested version: {version}"));
    println!();

    if !gitutil::is_repo(&dir) {
        ui::info("Linux source tree not found.");
        ui::info("Cloning Linux...");
        let spin = ui::spinner(&format!("Cloning Linux {version} (depth 1)..."));
        let result = gitutil::clone_tag(Paths::KERNEL_REPO, &version, &dir);
        spin.finish_and_clear();
        result?;
        println!();
        ui::ok(format!("Linux {version} downloaded."));
        return Ok(());
    }

    let current = gitutil::describe(&dir);
    if current == version {
        ui::ok(format!("Linux {version} is already installed."));
        return Ok(());
    }

    ui::info(format!("Current kernel: {current}"));
    ui::info(format!("Updating to:    {version}"));
    println!();

    gitutil::fetch_tags(&dir)?;
    gitutil::checkout_detach(&dir, &version)?;

    println!();
    ui::ok("Linux kernel updated.");
    ui::detail(format!("Version: {}", gitutil::describe(&dir)));
    ui::detail(format!("Commit:  {}", gitutil::head(&dir)));
    Ok(())
}

/// Équivalent de getutil.sh : clone/checkout la version épinglée dans
/// config/util-linux/version (indépendant du sous-module attendu par
/// `devtool build util-linux`).
fn get_util_linux(p: &Paths) -> Result<()> {
    let dir = p.util_linux_dir();
    let version_file = p.util_linux_version_file();

    if !version_file.is_file() {
        bail!("util-linux version file not found:\n  {}", version_file.display());
    }
    let version: String = std::fs::read_to_string(&version_file)?.chars().filter(|c| !c.is_whitespace()).collect();
    if version.is_empty() {
        bail!("util-linux version is empty.");
    }

    ui::info("util-linux");
    ui::detail(format!("Version: {version}"));
    ui::detail(format!("Directory: {}", dir.display()));

    if gitutil::is_repo(&dir) {
        let current = gitutil::describe(&dir);
        ui::detail(format!("Current: {current}"));

        if current == format!("v{version}") {
            ui::ok("util-linux is already up to date.");
            println!();
            println!("util-linux:");
            println!("  {}", dir.display());
            return Ok(());
        }

        println!();
        ui::info("Updating util-linux...");
        ui::detail(format!("{current} -> v{version}"));

        let status = gitutil::status_porcelain(&dir);
        if !status.is_empty() {
            bail!("util-linux source tree contains local modifications:\n\n{status}");
        }

        gitutil::fetch_tags(&dir)?;
        gitutil::checkout_detach(&dir, &format!("v{version}"))?;

        ui::ok("util-linux updated.");
        println!();
        println!("util-linux:");
        println!("  {}", dir.display());
        return Ok(());
    }

    if dir.exists() {
        bail!(
            "util-linux directory already exists but is not a Git repository:\n  {}",
            dir.display()
        );
    }

    println!();
    ui::info("Downloading util-linux...");
    ui::detail(format!("Repository: {}", Paths::UTIL_LINUX_REPO));
    ui::detail(format!("Version:    v{version}"));

    let spin = ui::spinner(&format!("Cloning util-linux v{version} (depth 1)..."));
    let result = gitutil::clone_tag(Paths::UTIL_LINUX_REPO, &format!("v{version}"), &dir);
    spin.finish_and_clear();
    result?;

    let exact = gitutil::describe(&dir);
    if exact != format!("v{version}") {
        bail!("downloaded util-linux version does not match requested version.");
    }

    println!();
    ui::ok("util-linux downloaded.");
    println!();
    println!("util-linux:");
    println!("  {}", dir.display());
    Ok(())
}

pub fn run(p: &Paths, component: Component) -> Result<()> {
    match component {
        Component::Linux => get_linux(p),
        Component::UtilLinux => get_util_linux(p),
        // Réutilise exactement la logique de mise à jour utilisée par
        // `devtool build busybox` (récupère toujours la dernière version),
        // sans lancer la compilation.
        Component::Busybox => crate::build::busybox::fetch(p),
    }
}
