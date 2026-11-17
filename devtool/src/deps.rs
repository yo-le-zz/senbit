//! `devtool doctor` / `devtool install` : vérifie (et installe) les outils de
//! build sur Fedora, Ubuntu/Debian et Arch.
//!
//! Chaque dépendance est testée par ce qu'elle FOURNIT (une commande, un
//! module pkg-config, un module Python), pas par le nom d'un paquet : le même
//! test marche donc sur les trois distributions, et seul le nom du paquet à
//! installer change.

use std::process::Command;

use anyhow::{bail, Result};
use colored::Colorize;

use crate::host::{self, Distro};
use crate::paths::Paths;
use crate::proc::run;
use crate::ui;

#[derive(Clone, Copy)]
enum Check {
    /// Une commande (l'une des alternatives).
    Command(&'static [&'static str]),
    /// Un module pkg-config (en-têtes de développement).
    PkgConfig(&'static str),
    /// Un module Python.
    Python(&'static str),
    /// Microgiciel OVMF (UEFI) pour QEMU.
    Ovmf,
}

struct Dep {
    what: &'static str,
    check: Check,
    /// Indispensable pour `devtool build` (sinon : seulement pour `run`/ISO...).
    required: bool,
    debian: &'static str,
    fedora: &'static str,
    arch: &'static str,
}

const fn dep(
    what: &'static str,
    check: Check,
    required: bool,
    debian: &'static str,
    fedora: &'static str,
    arch: &'static str,
) -> Dep {
    Dep {
        what,
        check,
        required,
        debian,
        fedora,
        arch,
    }
}

use Check::{Command as Cmd, Ovmf, PkgConfig as Pkg, Python as Py};

const DEPS: &[Dep] = &[
    // --- chaîne de compilation ---
    dep(
        "C compiler",
        Cmd(&["cc", "gcc"]),
        true,
        "build-essential",
        "gcc gcc-c++",
        "base-devel",
    ),
    dep(
        "make",
        Cmd(&["make"]),
        true,
        "build-essential",
        "make",
        "base-devel",
    ),
    dep("git", Cmd(&["git"]), true, "git", "git", "git"),
    dep("curl", Cmd(&["curl"]), true, "curl", "curl", "curl"),
    dep("tar", Cmd(&["tar"]), true, "tar", "tar", "tar"),
    dep("xz", Cmd(&["xz"]), true, "xz-utils", "xz", "xz"),
    dep(
        "zstd (.deb data)",
        Cmd(&["zstd"]),
        false,
        "zstd",
        "zstd",
        "zstd",
    ),
    dep("gzip", Cmd(&["gzip"]), true, "gzip", "gzip", "gzip"),
    dep("cpio", Cmd(&["cpio"]), true, "cpio", "cpio", "cpio"),
    dep(
        "patch / git apply",
        Cmd(&["patch"]),
        true,
        "patch",
        "patch",
        "patch",
    ),
    dep("rsync", Cmd(&["rsync"]), false, "rsync", "rsync", "rsync"),
    dep(
        "ar (binutils)",
        Cmd(&["ar"]),
        true,
        "binutils",
        "binutils",
        "binutils",
    ),
    dep("file", Cmd(&["file"]), false, "file", "file", "file"),
    // --- noyau ---
    dep("bc", Cmd(&["bc"]), true, "bc", "bc", "bc"),
    dep("bison", Cmd(&["bison"]), true, "bison", "bison", "bison"),
    dep("flex", Cmd(&["flex"]), true, "flex", "flex", "flex"),
    dep("perl", Cmd(&["perl"]), true, "perl", "perl", "perl"),
    dep(
        "libelf (kernel)",
        Pkg("libelf"),
        true,
        "libelf-dev",
        "elfutils-libelf-devel",
        "libelf",
    ),
    dep(
        "openssl (kernel)",
        Pkg("openssl"),
        true,
        "libssl-dev",
        "openssl-devel",
        "openssl",
    ),
    dep(
        "pahole (BTF)",
        Cmd(&["pahole"]),
        false,
        "dwarves",
        "dwarves",
        "pahole",
    ),
    // --- BusyBox / util-linux / parted ---
    dep(
        "ncurses",
        Pkg("ncurses"),
        false,
        "libncurses-dev",
        "ncurses-devel",
        "ncurses",
    ),
    dep(
        "pkg-config",
        Cmd(&["pkg-config", "pkgconf"]),
        true,
        "pkg-config",
        "pkgconf-pkg-config",
        "pkgconf",
    ),
    dep(
        "gettext",
        Cmd(&["msgfmt"]),
        true,
        "gettext",
        "gettext",
        "gettext",
    ),
    dep(
        "autotools (util-linux)",
        Cmd(&["autoreconf"]),
        true,
        "autoconf automake libtool",
        "autoconf automake libtool",
        "autoconf automake libtool",
    ),
    dep("gperf", Cmd(&["gperf"]), true, "gperf", "gperf", "gperf"),
    dep(
        "libuuid",
        Pkg("uuid"),
        true,
        "uuid-dev",
        "libuuid-devel",
        "util-linux-libs",
    ),
    dep(
        "libblkid",
        Pkg("blkid"),
        true,
        "libblkid-dev",
        "libblkid-devel",
        "util-linux-libs",
    ),
    // --- systemd ---
    dep("meson", Cmd(&["meson"]), true, "meson", "meson", "meson"),
    dep(
        "ninja",
        Cmd(&["ninja", "ninja-build"]),
        true,
        "ninja-build",
        "ninja-build",
        "ninja",
    ),
    dep(
        "python3",
        Cmd(&["python3"]),
        true,
        "python3",
        "python3",
        "python",
    ),
    dep(
        "python jinja2",
        Py("jinja2"),
        true,
        "python3-jinja2",
        "python3-jinja2",
        "python-jinja",
    ),
    dep(
        "python pyelftools",
        Py("elftools"),
        false,
        "python3-pyelftools",
        "python3-pyelftools",
        "python-pyelftools",
    ),
    dep(
        "libcap",
        Pkg("libcap"),
        true,
        "libcap-dev",
        "libcap-devel",
        "libcap",
    ),
    dep(
        "libmount",
        Pkg("mount"),
        true,
        "libmount-dev",
        "libmount-devel",
        "util-linux-libs",
    ),
    dep(
        "libcrypt",
        Pkg("libxcrypt"),
        true,
        "libcrypt-dev",
        "libxcrypt-devel",
        "libxcrypt",
    ),
    dep(
        "glibc tools (ldd, ldconfig)",
        Cmd(&["ldd", "ldconfig"]),
        true,
        "libc-bin",
        "glibc-common",
        "glibc",
    ),
    dep(
        "e2fsprogs (mke2fs, e2fsck)",
        Cmd(&["mke2fs"]),
        true,
        "e2fsprogs",
        "e2fsprogs",
        "e2fsprogs",
    ),
    // --- rootfs / ISO / VM ---
    dep(
        "rustup (cible musl)",
        Cmd(&["rustup"]),
        true,
        "rustup",
        "rustup",
        "rustup",
    ),
    dep(
        "xorriso",
        Cmd(&["xorriso"]),
        true,
        "xorriso",
        "xorriso",
        "libisoburn",
    ),
    dep(
        "grub-mkrescue",
        Cmd(&["grub-mkrescue", "grub2-mkrescue"]),
        true,
        "grub-common grub-pc-bin grub-efi-amd64-bin",
        "grub2-tools-extra grub2-pc-modules grub2-efi-x64-modules",
        "grub",
    ),
    dep(
        "mtools",
        Cmd(&["mformat"]),
        true,
        "mtools",
        "mtools",
        "mtools",
    ),
    dep(
        "kbd (loadkeys)",
        Cmd(&["loadkeys"]),
        false,
        "kbd",
        "kbd",
        "kbd",
    ),
    dep(
        "QEMU",
        Cmd(&["qemu-system-x86_64"]),
        false,
        "qemu-system-x86",
        "qemu-system-x86-core",
        "qemu-system-x86",
    ),
    dep(
        "qemu-img",
        Cmd(&["qemu-img"]),
        false,
        "qemu-utils",
        "qemu-img",
        "qemu-img",
    ),
    dep("OVMF (UEFI)", Ovmf, false, "ovmf", "edk2-ovmf", "edk2-ovmf"),
];

fn is_present(d: &Dep) -> bool {
    match d.check {
        Cmd(names) => names.iter().any(|n| host::has_tool(n)),
        Pkg(module) => host::pkg_config_has(module),
        Py(module) => host::python_module_has(module),
        // Même détection que `devtool run` : un couple CODE/VARS cohérent.
        Ovmf => crate::ovmf::find_firmware().is_ok(),
    }
}

fn package(d: &Dep, distro: Distro) -> &'static str {
    match distro {
        Distro::Fedora => d.fedora,
        Distro::Arch => d.arch,
        _ => d.debian,
    }
}

/// Paquets à installer pour `missing`, dédoublonnés, dans l'ordre.
fn packages_for(missing: &[&Dep], distro: Distro) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for d in missing {
        for pkg in package(d, distro).split_whitespace() {
            if !out.iter().any(|p| p == pkg) {
                out.push(pkg.to_string());
            }
        }
    }
    out
}

fn install_command(distro: Distro) -> Option<(&'static str, Vec<&'static str>)> {
    match distro {
        Distro::Debian => Some(("apt-get", vec!["install", "-y"])),
        Distro::Fedora => Some(("dnf", vec!["install", "-y"])),
        Distro::Arch => Some(("pacman", vec!["-S", "--needed", "--noconfirm"])),
        Distro::Unknown => None,
    }
}

fn scan(distro: Distro) -> (Vec<&'static Dep>, usize) {
    let mut missing = Vec::new();
    let mut found = 0;
    for d in DEPS {
        let present = is_present(d);
        let mark = if present {
            "✓".green().bold()
        } else if d.required {
            "✗".red().bold()
        } else {
            "–".yellow().bold()
        };
        let note = if present {
            String::new()
        } else if d.required {
            format!("  missing → {}", package(d, distro).red())
        } else {
            format!("  optional → {}", package(d, distro).yellow())
        };
        println!("  {mark} {}{note}", d.what);
        if present {
            found += 1;
        } else {
            missing.push(d);
        }
    }
    (missing, found)
}

/// Vérifie aussi la cible Rust musl (rustup).
fn check_rust_target(p: &Paths) -> bool {
    let target = p.rust_target();
    let Ok(out) = host::command("rustup")
        .args(["target", "list", "--installed"])
        .output()
    else {
        return false;
    };
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .any(|l| l.trim() == target)
}

/// `devtool doctor` : rapport seul, ne modifie rien.
pub fn doctor(p: &Paths) -> Result<()> {
    let distro = host::detect_distro();
    ui::banner("Senbit doctor");
    println!();
    ui::info(format!("Host: {}", distro.name()));
    ui::info(format!("Project root: {}", p.root.display()));
    println!();

    let (missing, found) = scan(distro);

    if host::has_tool("rustup") {
        if check_rust_target(p) {
            println!("  {} Rust target {}", "✓".green().bold(), p.rust_target());
        } else {
            println!(
                "  {} Rust target {} → rustup target add {}",
                "✗".red().bold(),
                p.rust_target(),
                p.rust_target()
            );
        }
    }

    let required_missing: Vec<&Dep> = missing.iter().copied().filter(|d| d.required).collect();
    println!();
    ui::info(format!("{found}/{} checks passed.", DEPS.len()));

    if required_missing.is_empty() {
        ui::ok("Everything required to build Senbit is installed.");
        return Ok(());
    }

    let packages = packages_for(&missing, distro);
    println!();
    ui::warn(format!(
        "{} required tool(s) missing.",
        required_missing.len()
    ));
    match install_command(distro) {
        Some((tool, args)) => {
            println!();
            println!("  Install everything with:");
            println!("    devtool install");
            println!("  or by hand:");
            println!("    sudo {tool} {} {}", args.join(" "), packages.join(" "));
        }
        None => println!("  Unknown distribution: install the tools listed above."),
    }
    bail!("missing build dependencies")
}

/// `devtool install` : installe les paquets manquants avec le gestionnaire
/// de la distribution, puis prépare le noyau.
pub fn install(p: &Paths) -> Result<()> {
    let distro = host::detect_distro();
    ui::banner("Senbit build environment");
    println!();
    ui::info(format!("Host: {}", distro.name()));

    if unsafe { geteuid() } == 0 {
        bail!("do not run `devtool install` as root.\nRun it as your normal user (sudo is used where needed).");
    }
    let Some((tool, args)) = install_command(distro) else {
        bail!("unknown distribution: run `devtool doctor` and install the listed tools by hand.");
    };

    let missing: Vec<&Dep> = DEPS.iter().filter(|d| !is_present(d)).collect();
    let packages = packages_for(&missing, distro);

    if packages.is_empty() {
        ui::ok("All build dependencies are already installed.");
    } else {
        if distro == Distro::Debian {
            ui::info("Updating package lists...");
            run(Command::new("sudo").args(["apt-get", "update"]))?;
        }
        println!();
        ui::info(format!("Installing: {}", packages.join(" ")));
        let mut cmd = Command::new("sudo");
        cmd.arg(tool).args(&args).args(&packages);
        run(&mut cmd)?;
    }

    // rustup : toolchain du projet + cible musl.
    if host::has_tool("rustup") {
        println!();
        ui::info(format!("Adding the Rust target {}...", p.rust_target()));
        run(Command::new("rustup").args(["target", "add", p.rust_target()]))?;
    } else {
        ui::warn(
            "rustup not found: install it from https://rustup.rs then run `devtool install` again.",
        );
    }

    println!();
    ui::ok("Build environment ready. Next:");
    println!("    devtool get all     # download the pinned sources");
    println!("    devtool build       # build everything");
    println!("    devtool run         # boot it in QEMU");
    Ok(())
}

unsafe fn geteuid() -> u32 {
    extern "C" {
        fn geteuid() -> u32;
    }
    geteuid()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_dependency_names_a_package_for_each_distro() {
        for d in DEPS {
            assert!(
                !d.debian.is_empty() && !d.fedora.is_empty() && !d.arch.is_empty(),
                "{}",
                d.what
            );
        }
    }

    #[test]
    fn packages_are_deduplicated() {
        let refs: Vec<&Dep> = DEPS
            .iter()
            .filter(|d| d.what == "C compiler" || d.what == "make")
            .collect();
        let pkgs = packages_for(&refs, Distro::Debian);
        assert_eq!(pkgs, vec!["build-essential".to_string()]);
    }
}
