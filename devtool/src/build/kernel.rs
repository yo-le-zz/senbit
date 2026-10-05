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

/// Options forcées par-dessus `x86_64_defconfig`.
///
/// Le defconfig amont n'active plus aucun pilote d'affichage généraliste
/// (ni CONFIG_FB, ni FRAMEBUFFER_CONSOLE, ni SIMPLEDRM). En BIOS le texte VGA
/// sauve la mise, mais en UEFI il n'y a PAS de mode texte VGA : sans
/// framebuffer le noyau tourne "à l'aveugle" (écran noir, aucun println!).
const REQUIRED_CONFIG: &[&str] = &[
    // console virtuelle + console sur framebuffer
    "VT",
    "VT_CONSOLE",
    "FB",
    "FRAMEBUFFER_CONSOLE",
    "FONTS",
    "FONT_8x16",
    // framebuffer laissé par le firmware UEFI (GOP)
    "FB_EFI",
    "SYSFB_SIMPLEFB",
    "DRM_SIMPLEDRM",
    "DRM_FBDEV_EMULATION",
    // GPU émulés courants (QEMU std VGA, VirtualBox)
    "DRM_BOCHS",
    "DRM_VBOXVIDEO",
    // requis par systemd (PID 1) - déjà dans le defconfig actuel, épinglés
    // ici pour qu'un changement amont ne les retire pas en silence
    // `devtool run --debug` passes its flag to the guest through QEMU fw_cfg
    "FW_CFG_SYSFS",
    "DEVTMPFS",
    "CGROUPS",
    "CGROUP_PIDS",
    "INOTIFY_USER",
    "SIGNALFD",
    "TIMERFD",
    "EPOLL",
    "FHANDLE",
    "TMPFS",
    "TMPFS_XATTR",
    "TMPFS_POSIX_ACL",
    "AUTOFS_FS",
    "NAMESPACES",
    // utilisés par l'installeur
    "ISO9660_FS",
    "VFAT_FS",
    "NLS_CODEPAGE_437",
    "NLS_ISO8859_1",
];

fn apply_required_config(p: &Paths) -> Result<()> {
    let build_dir = p.kernel_build_dir();
    let config = build_dir.join(".config");
    let script = p.kernel_dir().join("scripts/config");

    let mut cmd = Command::new(&script);
    cmd.arg("--file").arg(&config);
    for opt in REQUIRED_CONFIG {
        cmd.arg("-e").arg(opt);
    }
    run_quiet(&mut cmd)?;

    let o = format!("O={}", build_dir.display());
    run_quiet(Command::new("make").arg("-C").arg(p.kernel_dir()).arg(&o).arg("olddefconfig"))?;

    // olddefconfig supprime silencieusement une option dont une dépendance
    // manque : on vérifie qu'elles sont toutes réellement à "=y".
    let text = std::fs::read_to_string(&config)?;
    let missing: Vec<&str> = REQUIRED_CONFIG
        .iter()
        .copied()
        .filter(|opt| !text.lines().any(|l| l == format!("CONFIG_{opt}=y")))
        .collect();

    if !missing.is_empty() {
        bail!(
            "kernel options could not be enabled (missing dependency?):\n  {}",
            missing.join("\n  ")
        );
    }

    Ok(())
}

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
    // La liste d'options fait partie de l'empreinte : la modifier relance la
    // compilation même si les sources du noyau n'ont pas bougé.
    let fp = format!("{}|cfg:{}", git_fingerprint(&dir), REQUIRED_CONFIG.join(","));

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
    ui::info("Updating kernel configuration (framebuffer console for UEFI)...");
    apply_required_config(p)?;

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
