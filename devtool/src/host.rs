//! Détection de la machine de build : distribution, outils présents, paquets
//! à installer. Rien n'est codé en dur sur `/usr/bin/...` : Fedora, Ubuntu /
//! Debian et Arch n'ont pas la même disposition (`/usr/sbin`, `/usr/lib64`...).

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Distro {
    Debian,
    Fedora,
    Arch,
    Unknown,
}

impl Distro {
    pub fn name(self) -> &'static str {
        match self {
            Distro::Debian => "Debian / Ubuntu (apt)",
            Distro::Fedora => "Fedora / RHEL (dnf)",
            Distro::Arch => "Arch Linux (pacman)",
            Distro::Unknown => "unknown distribution",
        }
    }
}

/// Lit ID= et ID_LIKE= de /etc/os-release.
pub fn detect_distro() -> Distro {
    let text = std::fs::read_to_string("/etc/os-release")
        .or_else(|_| std::fs::read_to_string("/usr/lib/os-release"))
        .unwrap_or_default();
    parse_distro(&text)
}

pub fn parse_distro(os_release: &str) -> Distro {
    let mut ids: Vec<String> = Vec::new();
    for line in os_release.lines() {
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        if key == "ID" || key == "ID_LIKE" {
            let value = value.trim().trim_matches('"');
            ids.extend(value.split_whitespace().map(|v| v.to_lowercase()));
        }
    }
    let has = |names: &[&str]| ids.iter().any(|i| names.contains(&i.as_str()));
    if has(&["arch", "manjaro", "endeavouros", "cachyos"]) {
        Distro::Arch
    } else if has(&["fedora", "rhel", "centos", "rocky", "almalinux", "nobara"]) {
        Distro::Fedora
    } else if has(&["debian", "ubuntu", "linuxmint", "pop", "neon", "kali"]) {
        Distro::Debian
    } else {
        Distro::Unknown
    }
}

/// Dossiers cherchés EN PLUS de $PATH : `ldconfig`, `mke2fs`, `grub-install`...
/// vivent dans /sbin ou /usr/sbin, absents du PATH d'un utilisateur Debian.
const EXTRA_DIRS: &[&str] = &[
    "/usr/local/sbin",
    "/usr/sbin",
    "/sbin",
    "/usr/local/bin",
    "/usr/bin",
    "/bin",
];

fn is_executable_file(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(path)
        .map(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
        .unwrap_or(false)
}

/// Chemin absolu de la commande `name` (PATH puis dossiers sbin), si elle existe.
pub fn find_tool(name: &str) -> Option<PathBuf> {
    if name.contains('/') {
        let p = PathBuf::from(name);
        return is_executable_file(&p).then_some(p);
    }
    let path_var = std::env::var_os("PATH").unwrap_or_default();
    let mut dirs: Vec<PathBuf> = std::env::split_paths(&path_var).collect();
    dirs.extend(EXTRA_DIRS.iter().map(PathBuf::from));
    dirs.into_iter()
        .map(|d| d.join(name))
        .find(|p| is_executable_file(p))
}

pub fn has_tool(name: &str) -> bool {
    find_tool(name).is_some()
}

/// Premier de `names` présent sur la machine.
pub fn find_first(names: &[&str]) -> Option<PathBuf> {
    names.iter().find_map(|n| find_tool(n))
}

/// `Command` pour `name`, résolu comme `find_tool` (repli : le nom seul).
pub fn command(name: &str) -> Command {
    Command::new(find_tool(name).unwrap_or_else(|| PathBuf::from(name)))
}

fn quiet_ok(cmd: &mut Command) -> bool {
    cmd.stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

pub fn pkg_config_has(module: &str) -> bool {
    has_tool("pkg-config") && quiet_ok(command("pkg-config").arg("--exists").arg(module))
}

pub fn python_module_has(module: &str) -> bool {
    has_tool("python3") && quiet_ok(command("python3").args(["-c", &format!("import {module}")]))
}

/// Dossiers possibles des microgiciels OVMF (UEFI) pour QEMU.
pub const OVMF_DIRS: &[&str] = &[
    "/usr/share/OVMF",
    "/usr/share/ovmf",
    "/usr/share/edk2/ovmf",
    "/usr/share/edk2/x64",
    "/usr/share/edk2-ovmf/x64",
    "/usr/share/qemu",
    "/usr/share/edk2",
];

/// Cherche le premier fichier de `names` dans les dossiers OVMF connus.
pub fn find_ovmf(names: &[&str]) -> Option<PathBuf> {
    OVMF_DIRS
        .iter()
        .flat_map(|d| names.iter().map(move |n| Path::new(d).join(n)))
        .find(|p| p.is_file())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_the_three_families() {
        assert_eq!(parse_distro("ID=fedora\n"), Distro::Fedora);
        assert_eq!(parse_distro("ID=ubuntu\nID_LIKE=debian\n"), Distro::Debian);
        assert_eq!(parse_distro("ID=\"arch\"\n"), Distro::Arch);
        assert_eq!(parse_distro("ID=garuda\nID_LIKE=\"arch\"\n"), Distro::Arch);
        assert_eq!(parse_distro("ID=nixos\n"), Distro::Unknown);
    }

    #[test]
    fn finds_sh_on_any_distro() {
        assert!(find_tool("sh").is_some());
        assert!(find_tool("definitely-not-a-tool-xyz").is_none());
    }
}
