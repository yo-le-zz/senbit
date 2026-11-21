//! `devtool get <composant|all>` : télécharge les sources À LA VERSION
//! ÉPINGLÉE (config/<composant>/version, vérifiée par commit ou sha256).
//! Jamais « la dernière version » : tout est reproductible.

use std::path::Path;
use std::process::Command;

use anyhow::{bail, Context, Result};
use clap::ValueEnum;
use sha2::{Digest, Sha256};

use crate::paths::Paths;
use crate::proc::run;
use crate::{gitutil, host, ui};

#[derive(Debug, Clone, Copy, ValueEnum, PartialEq, Eq)]
#[value(rename_all = "kebab-case")]
pub enum Component {
    /// Tout télécharger.
    All,
    Linux,
    Busybox,
    UtilLinux,
    Parted,
    Systemd,
}

pub fn read_pinned(file: &Path, what: &str) -> Result<String> {
    if !file.is_file() {
        bail!("{what} pin file not found:\n  {}", file.display());
    }
    let v: String = std::fs::read_to_string(file)?
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect();
    if v.is_empty() {
        bail!("{what} pin file is empty:\n  {}", file.display());
    }
    Ok(v)
}

fn read_optional(file: &Path) -> Option<String> {
    read_pinned(file, "pin").ok().map(|s| s.to_lowercase())
}

/// Clone (profondeur 1) ou met à jour `dir` sur `tag`, puis vérifie le commit
/// attendu s'il est épinglé. Essaie `mirror` si le dépôt principal échoue.
pub fn ensure_git(
    name: &str,
    repos: &[&str],
    dir: &Path,
    tag: &str,
    sha: Option<&str>,
) -> Result<()> {
    ui::info(format!("{name} {tag}"));

    if !gitutil::is_repo(dir) {
        // An EMPTY directory is the placeholder that `git clone` of Senbit leaves
        // (the folder is there, its content is not): clone into it. A directory
        // with something else in it is never touched.
        if dir.exists() && !gitutil::is_empty_dir(dir) {
            bail!(
                "{} exists, is not empty and is not a Git repository.\n\nMove it away (or delete it) and run `devtool get` again.",
                dir.display()
            );
        }
        let mut last = None;
        for repo in repos.iter().filter(|r| !r.is_empty()) {
            let spin = ui::spinner(&format!("Cloning {name} {tag} from {repo}..."));
            let result = gitutil::clone_tag(repo, tag, dir);
            spin.finish_and_clear();
            match result {
                Ok(()) => {
                    last = None;
                    break;
                }
                Err(e) => {
                    ui::warn(format!("{repo}: {e:#}"));
                    let _ = std::fs::remove_dir_all(dir);
                    last = Some(e);
                }
            }
        }
        if let Some(e) = last {
            return Err(e.context(format!("could not download {name} {tag}")));
        }
    } else if gitutil::describe_exact(dir) != tag {
        let status = gitutil::status_porcelain(dir);
        if !status.is_empty() {
            let shown = dir.display();
            bail!("{name} source tree has local modifications:\n\n{status}\n\nSenbit changes belong in patches/ (see patches/README.md).\nTo drop the applied patches and update: git -C {shown} checkout -- .");
        }
        ui::info(format!("Updating {name} to {tag}..."));
        gitutil::fetch_tags(dir)?;
        gitutil::checkout_detach(dir, tag)?;
    }

    let head = gitutil::head(dir).to_lowercase();
    match sha {
        Some(expected) if head != expected => bail!(
            "{name} {tag}: commit mismatch.\n  expected: {expected}\n  found:    {head}\n\nThe tag was moved or the source is corrupted: refusing to build."
        ),
        Some(_) => ui::ok(format!("{name} {tag} ready (commit verified).")),
        None => ui::ok(format!("{name} {tag} ready (no commit pinned in config/).")),
    }
    Ok(())
}

pub fn sha256_file(path: &Path) -> Result<String> {
    let mut file = std::fs::File::open(path)?;
    let mut hasher = Sha256::new();
    std::io::copy(&mut file, &mut hasher)?;
    Ok(hasher
        .finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect())
}

/// Télécharge `urls` (premier qui marche) dans `dest` avec curl.
pub fn download(urls: &[String], dest: &Path) -> Result<()> {
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut last = None;
    for url in urls {
        let tmp = dest.with_extension("part");
        let spin = ui::spinner(&format!("Downloading {url}"));
        let result = run_curl(url, &tmp);
        spin.finish_and_clear();
        match result {
            Ok(()) => {
                std::fs::rename(&tmp, dest)?;
                return Ok(());
            }
            Err(e) => {
                ui::warn(format!("{url}: {e:#}"));
                last = Some(e);
            }
        }
    }
    Err(last.unwrap_or_else(|| anyhow::anyhow!("no download URL"))).context("download failed")
}

fn run_curl(url: &str, out: &Path) -> Result<()> {
    crate::proc::run_quiet(
        host::command("curl")
            .args([
                "--fail",
                "--location",
                "--silent",
                "--show-error",
                "--retry",
                "3",
                "--proto",
                "=https",
                "-o",
            ])
            .arg(out)
            .arg(url),
    )
}

fn get_linux(p: &Paths) -> Result<()> {
    let version = read_pinned(&p.kernel_version_file(), "kernel version")?;
    let sha = read_optional(&p.kernel_sha_file());
    ensure_git(
        "Linux",
        &[p.kernel_repo(), p.kernel_mirror()],
        &p.kernel_dir(),
        &version,
        sha.as_deref(),
    )
}

fn get_busybox(p: &Paths) -> Result<()> {
    // config/busybox/version : "1.38.0" ; tag git : "1_38_0".
    let version = read_pinned(&p.busybox_version_file(), "BusyBox version")?;
    let tag = version.replace('.', "_");
    ensure_git("BusyBox", &[p.busybox_repo()], &p.busybox_dir(), &tag, None)
}

fn get_util_linux(p: &Paths) -> Result<()> {
    let version = read_pinned(&p.util_linux_version_file(), "util-linux version")?;
    let sha = read_optional(&p.util_linux_sha_file());
    ensure_git(
        "util-linux",
        &[p.util_linux_repo()],
        &p.util_linux_dir(),
        &format!("v{version}"),
        sha.as_deref(),
    )
}

fn get_systemd(p: &Paths) -> Result<()> {
    let version = read_pinned(&p.systemd_version_file(), "systemd version")?;
    let sha = read_optional(&p.systemd_sha_file());
    ensure_git(
        "systemd",
        &[p.systemd_repo()],
        &p.systemd_dir(),
        &version,
        sha.as_deref(),
    )
}

/// Parted : archive de release GNU (configure et gnulib déjà générés),
/// vérifiée par sha256. Remplace l'ancien dépôt Git, qui exigeait `bootstrap`.
fn get_parted(p: &Paths) -> Result<()> {
    let version = read_pinned(&p.parted_version_file(), "Parted version")?;
    let dir = p.parted_dir();
    let marker = dir.join(".tarball-version");

    ui::info(format!("GNU Parted {version}"));

    if dir.join("configure").is_file()
        && std::fs::read_to_string(&marker)
            .map(|v| v.trim() == version)
            .unwrap_or(false)
    {
        ui::ok(format!("Parted {version} already downloaded."));
        return Ok(());
    }
    if dir.exists() {
        // Ancien clone Git ou autre version : on repart proprement.
        std::fs::remove_dir_all(&dir)?;
    }

    let urls: Vec<String> = p
        .cfg
        .parted
        .urls
        .iter()
        .map(|u| u.replace("{version}", &version))
        .collect();
    let archive = p.r(&format!("build/downloads/parted-{version}.tar.xz"));
    download(&urls, &archive)?;

    let expected = read_pinned(&p.parted_sha256_file(), "Parted sha256")?.to_lowercase();
    let actual = sha256_file(&archive)?;
    if actual != expected {
        let _ = std::fs::remove_file(&archive);
        bail!("Parted archive sha256 mismatch.\n  expected: {expected}\n  found:    {actual}");
    }
    ui::detail("sha256 verified.");

    std::fs::create_dir_all(&dir)?;
    run(Command::new("tar")
        .arg("-xf")
        .arg(&archive)
        .arg("-C")
        .arg(&dir)
        .args(["--strip-components=1"]))?;
    std::fs::write(&marker, format!("{version}\n"))?;
    ui::ok(format!("Parted {version} extracted to {}", dir.display()));
    Ok(())
}

/// Downloads a component only when its sources are missing (absent or empty
/// folder). `devtool build` calls it before each compilation, so a fresh
/// `git clone` builds without a separate `devtool get`.
pub fn ensure(p: &Paths, component: Component) -> Result<()> {
    let (name, missing) = match component {
        Component::Linux => ("Linux", !gitutil::is_repo(&p.kernel_dir())),
        Component::Busybox => ("BusyBox", !gitutil::is_repo(&p.busybox_dir())),
        Component::UtilLinux => ("util-linux", !gitutil::is_repo(&p.util_linux_dir())),
        Component::Parted => ("GNU Parted", !p.parted_dir().join("configure").is_file()),
        Component::Systemd => ("systemd", !gitutil::is_repo(&p.systemd_dir())),
        Component::All => return run_get(p, component),
    };

    if missing {
        ui::warn(format!(
            "{name} sources are missing or empty: downloading them (devtool get)."
        ));
        run_get(p, component)
    } else {
        Ok(())
    }
}

pub fn run_get(p: &Paths, component: Component) -> Result<()> {
    match component {
        Component::Linux => get_linux(p),
        Component::Busybox => get_busybox(p),
        Component::UtilLinux => get_util_linux(p),
        Component::Parted => get_parted(p),
        Component::Systemd => get_systemd(p),
        Component::All => {
            for c in [
                Component::Linux,
                Component::Busybox,
                Component::UtilLinux,
                Component::Parted,
                Component::Systemd,
            ] {
                ui::section(&format!("{c:?}"));
                run_get(p, c)?;
            }
            println!();
            ui::ok("All sources are downloaded at their pinned versions.");
            Ok(())
        }
    }
}
