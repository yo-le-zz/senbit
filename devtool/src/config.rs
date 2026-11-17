//! Lecture de `config.toml` (racine du projet).
//!
//! Tout y a une valeur par défaut : un `config.toml` partiel, ou absent,
//! donne la configuration d'origine. Les chemins sont relatifs à la racine du
//! projet (jamais absolus) pour que le dépôt fonctionne cloné n'importe où.

use std::path::Path;

use anyhow::{Context, Result};
use serde::Deserialize;

#[derive(Debug, Deserialize, Default, Clone)]
#[serde(default)]
pub struct Config {
    pub kernel: Kernel,
    pub busybox: Busybox,
    pub util_linux: UtilLinux,
    pub parted: Parted,
    pub systemd: Systemd,
    pub apt: Apt,
    pub rust: Rust,
    pub commands: Commands,
}

fn s(value: &str) -> String {
    value.to_string()
}

#[derive(Debug, Deserialize, Clone)]
#[serde(default)]
pub struct Kernel {
    pub repository: String,
    pub mirror: String,
    pub directory: String,
}

impl Default for Kernel {
    fn default() -> Self {
        Self {
            repository: s("https://git.kernel.org/pub/scm/linux/kernel/git/stable/linux.git"),
            mirror: s("https://github.com/gregkh/linux.git"),
            directory: s("kernel/linux"),
        }
    }
}

#[derive(Debug, Deserialize, Clone)]
#[serde(default)]
pub struct Busybox {
    pub repository: String,
    pub directory: String,
}

impl Default for Busybox {
    fn default() -> Self {
        Self {
            repository: s("https://git.busybox.net/busybox"),
            directory: s("third_party/busybox"),
        }
    }
}

#[derive(Debug, Deserialize, Clone)]
#[serde(default)]
pub struct UtilLinux {
    pub repository: String,
    pub directory: String,
}

impl Default for UtilLinux {
    fn default() -> Self {
        Self {
            repository: s("https://github.com/util-linux/util-linux.git"),
            directory: s("third_party/util-linux"),
        }
    }
}

#[derive(Debug, Deserialize, Clone)]
#[serde(default)]
pub struct Parted {
    pub urls: Vec<String>,
    pub directory: String,
}

impl Default for Parted {
    fn default() -> Self {
        Self {
            urls: vec![
                s("https://ftp.gnu.org/gnu/parted/parted-{version}.tar.xz"),
                s("https://ftpmirror.gnu.org/parted/parted-{version}.tar.xz"),
                s("https://mirrors.kernel.org/gnu/parted/parted-{version}.tar.xz"),
            ],
            directory: s("third_party/parted"),
        }
    }
}

#[derive(Debug, Deserialize, Clone)]
#[serde(default)]
pub struct Systemd {
    pub repository: String,
    pub directory: String,
}

impl Default for Systemd {
    fn default() -> Self {
        Self {
            repository: s("https://github.com/systemd/systemd.git"),
            directory: s("external/systemd"),
        }
    }
}

/// Paquets Debian amorcés dans le rootfs : apt/dpkg + ce que les scripts de
/// mainteneur (preinst/postinst...) des autres paquets supposent présent
/// (paquets « Essential » de Debian et leurs outils : perl, debconf,
/// init-system-helpers, GNU coreutils/sed/grep/findutils/tar...).
pub const DEFAULT_BOOTSTRAP: &[&str] = &[
    "apt",
    "dpkg",
    "debian-archive-keyring",
    "gpgv",
    "ca-certificates",
    "init-system-helpers",
    "perl-base",
    "debconf",
    "bash",
    "coreutils",
    "sed",
    "grep",
    "findutils",
    "gzip",
    "tar",
    "diffutils",
    "debianutils",
    "hostname",
    "ncurses-base",
    "ncurses-bin",
    "netbase",
    "dbus",
    "less",
];

#[derive(Debug, Deserialize, Clone)]
#[serde(default)]
pub struct Apt {
    pub mirror: String,
    pub security_mirror: String,
    pub suite: String,
    pub components: Vec<String>,
    pub architecture: String,
    pub bootstrap: Vec<String>,
}

impl Default for Apt {
    fn default() -> Self {
        Self {
            mirror: s("https://deb.debian.org/debian"),
            security_mirror: s("https://deb.debian.org/debian-security"),
            suite: s("bookworm"),
            components: vec![s("main")],
            architecture: s("amd64"),
            bootstrap: DEFAULT_BOOTSTRAP.iter().map(|p| p.to_string()).collect(),
        }
    }
}

#[derive(Debug, Deserialize, Clone)]
#[serde(default)]
pub struct Rust {
    pub target: String,
}

impl Default for Rust {
    fn default() -> Self {
        Self {
            target: s("x86_64-unknown-linux-musl"),
        }
    }
}

/// Commandes natives Senbit : un dossier `commands/<nom>/` = une crate Rust,
/// détectée et compilée automatiquement (voir `commands/README.md`).
#[derive(Debug, Deserialize, Clone)]
#[serde(default)]
pub struct Commands {
    pub directory: String,
}

impl Default for Commands {
    fn default() -> Self {
        Self {
            directory: s("commands"),
        }
    }
}

impl Config {
    /// Lit `<root>/config.toml` ; un fichier absent donne les valeurs par
    /// défaut, un fichier invalide est une erreur claire.
    pub fn load(root: &Path) -> Result<Self> {
        let file = root.join("config.toml");
        if !file.is_file() {
            return Ok(Self::default());
        }
        let text = std::fs::read_to_string(&file)
            .with_context(|| format!("cannot read {}", file.display()))?;
        toml::from_str(&text).with_context(|| format!("invalid {}", file.display()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn partial_config_keeps_defaults() {
        let cfg: Config = toml::from_str("[apt]\nsuite = \"trixie\"\n").unwrap();
        assert_eq!(cfg.apt.suite, "trixie");
        assert_eq!(cfg.apt.architecture, "amd64");
        assert_eq!(cfg.rust.target, "x86_64-unknown-linux-musl");
    }

    #[test]
    fn project_config_toml_parses() {
        let text = include_str!("../../config.toml");
        let cfg: Config = toml::from_str(text).expect("config.toml must parse");
        assert_eq!(cfg.apt.suite, "bookworm");
        assert_eq!(cfg.commands.directory, "commands");
        assert!(cfg.apt.bootstrap.iter().any(|p| p == "perl-base"));
        assert!(cfg.parted.urls.iter().all(|u| u.contains("{version}")));
    }
}
