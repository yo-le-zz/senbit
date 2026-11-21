//! `devtool build package` : les paquets de mise à jour de Senbit et leur dépôt APT.
//!
//! Le système se met à jour avec les VRAIES commandes (`apt update`, `apt upgrade`) :
//! les composants de Senbit sont des paquets Debian publiés dans un dépôt APT signé.
//!
//!   senbit-kernel      /boot/vmlinuz et /boot/initramfs.cpio.gz (noyau + senbit-init)
//!   senbit-userspace   senbit-login, les commandes natives (commands/), les unités
//!                      systemd, les scripts et la configuration de rootfs/
//!
//! Sortie : build/repo/ (à publier tel quel sur un serveur web ou une clé USB)
//!
//!   pool/main/<paquet>/<paquet>_<version>_amd64.deb
//!   dists/<suite>/main/binary-amd64/Packages(.gz)
//!   dists/<suite>/Release, Release.gpg, InRelease   (signés si `[update] gpg_key`)
//!
//! Les fichiers de `/etc` sont des « conffiles » : une mise à jour ne remplace jamais
//! une configuration modifiée par l'administrateur (`--force-confold` dans apt.conf.d/10senbit).
//!
//! Aucun outil Debian n'est nécessaire sur la machine de build (ni dpkg-deb ni
//! dpkg-scanpackages) : `tar`, `gzip` et `ar` suffisent.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{bail, Context, Result};
use sha2::{Digest, Sha256};

use crate::build::rootfs::copy_tree;
use crate::paths::Paths;
use crate::proc::run_quiet;
use crate::ui;

/// Les paquets de Senbit que le système déclare installés (voir apt.rs) : pour que
/// `apt upgrade` voie la nouvelle version.
pub const SENBIT_PACKAGES: &[&str] = &["senbit-kernel", "senbit-userspace"];

/* ------------------------------------------------------------------ */
/* Versions                                                           */
/* ------------------------------------------------------------------ */

/// Version de Senbit : celle du Cargo.toml racine.
fn parse_version(cargo_toml: &str) -> Option<String> {
    let doc: toml::Value = toml::from_str(cargo_toml).ok()?;
    doc.get("package")?
        .get("version")?
        .as_str()
        .map(str::to_string)
}

pub fn senbit_version(p: &Paths) -> Result<String> {
    let path = p.r("Cargo.toml");
    let text = fs::read_to_string(&path).with_context(|| format!("cannot read {}", path.display()))?;
    parse_version(&text).with_context(|| format!("no [package] version in {}", path.display()))
}

/// Version Debian d'un paquet : « 0.4.0 » -> « 0.4.0-1 ».
pub fn deb_version(senbit: &str) -> String {
    format!("{senbit}-1")
}

/* ------------------------------------------------------------------ */
/* Fichiers de contrôle                                               */
/* ------------------------------------------------------------------ */

struct Package<'a> {
    name: &'a str,
    version: &'a str,
    depends: &'a str,
    summary: &'a str,
    description: &'a str,
    installed_kib: u64,
}

/// Fichier `control` d'un paquet.
fn render_control(pkg: &Package) -> String {
    let mut text = String::new();
    text.push_str(&format!("Package: {}\n", pkg.name));
    text.push_str(&format!("Version: {}\n", pkg.version));
    text.push_str("Architecture: amd64\n");
    text.push_str("Maintainer: Senbit <senbit@localhost>\n");
    text.push_str(&format!("Installed-Size: {}\n", pkg.installed_kib));
    if !pkg.depends.is_empty() {
        text.push_str(&format!("Depends: {}\n", pkg.depends));
    }
    text.push_str("Section: admin\n");
    text.push_str("Priority: required\n");
    text.push_str(&format!("Description: {}\n", pkg.summary));
    for line in pkg.description.lines() {
        if line.trim().is_empty() {
            text.push_str(" .\n");
        } else {
            text.push_str(&format!(" {line}\n"));
        }
    }
    text
}

/// Avant l'installation : la version qui tourne devient l'entrée « previous version »
/// du menu de démarrage (voir rootfs/usr/lib/senbit/grub-config).
const KERNEL_PREINST: &str = r#"#!/bin/sh
set -e
if [ "$1" = install ] || [ "$1" = upgrade ]; then
    if [ -f /boot/vmlinuz ]; then
        cp -p /boot/vmlinuz /boot/vmlinuz.old
    fi
    if [ -f /boot/initramfs.cpio.gz ]; then
        cp -p /boot/initramfs.cpio.gz /boot/initramfs.cpio.gz.old
    fi
fi
exit 0
"#;

/// Après l'installation : une installation faite avant le canal de mise à jour n'a
/// pas de compteur de démarrage dans grub.cfg : on le crée.
const KERNEL_POSTINST: &str = r#"#!/bin/sh
set -e
if [ "$1" = configure ] && [ -x /usr/lib/senbit/grub-config ]; then
    if ! grep -q "senbit-boot-counter v2" /boot/grub/grub.cfg 2>/dev/null; then
        uuid="$(sed -n 's|^UUID=\([^ ]*\) / .*|\1|p' /etc/fstab | head -n 1)"
        if [ -n "$uuid" ]; then
            /usr/lib/senbit/grub-config "$uuid" || true
        fi
    fi
fi
sync
exit 0
"#;

const USERSPACE_POSTINST: &str = r#"#!/bin/sh
set -e
if [ "$1" = configure ]; then
    # /etc/senbit-release says which version is installed (it is also the marker of an
    # installed system): keep it in step with the package.
    version="$(dpkg-query -W -f='${Version}' senbit-userspace 2>/dev/null | sed 's/-[^-]*$//')"
    if [ -n "$version" ]; then
        printf 'NAME=Senbit\nVERSION=%s\n' "$version" > /etc/senbit-release
    fi
fi
if [ "$1" = configure ] && [ -d /run/systemd/system ]; then
    systemd-sysusers 2>/dev/null || true
    systemctl daemon-reload 2>/dev/null || true
    # A system installed before 0.4.0 does not have these units enabled yet.
    systemctl enable senbit-boot-ok.service senbit-update-check.timer 2>/dev/null || true
fi
exit 0
"#;

/* ------------------------------------------------------------------ */
/* Outils                                                             */
/* ------------------------------------------------------------------ */

fn dir_size_kib(dir: &Path) -> u64 {
    fn walk(dir: &Path) -> u64 {
        let mut total = 0;
        if let Ok(entries) = fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                match fs::symlink_metadata(&path) {
                    Ok(meta) if meta.is_dir() => total += walk(&path),
                    Ok(meta) => total += meta.len(),
                    Err(_) => {}
                }
            }
        }
        total
    }
    walk(dir).div_ceil(1024)
}

/// Fichiers réguliers de `etc/` : ce sont les fichiers de configuration (conffiles).
fn collect_conffiles(stage: &Path) -> Vec<String> {
    fn walk(root: &Path, dir: &Path, out: &mut Vec<String>) {
        let Ok(entries) = fs::read_dir(dir) else { return };
        for entry in entries.flatten() {
            let path = entry.path();
            let Ok(meta) = fs::symlink_metadata(&path) else { continue };
            if meta.is_dir() {
                walk(root, &path, out);
            } else if meta.is_file() {
                if let Ok(relative) = path.strip_prefix(root) {
                    out.push(format!("/{}", relative.display()));
                }
            }
        }
    }

    let mut out = Vec::new();
    walk(stage, &stage.join("etc"), &mut out);
    out.sort();
    out
}

fn sha256_hex(path: &Path) -> Result<String> {
    let bytes = fs::read(path).with_context(|| format!("cannot read {}", path.display()))?;
    Ok(format!("{:x}", Sha256::digest(&bytes)))
}

fn run(command: &mut Command) -> Result<()> {
    run_quiet(command)
}

/// Assemble un .deb : `ar` de debian-binary, control.tar.gz et data.tar.gz.
fn build_deb(
    work: &Path,
    stage: &Path,
    control: &str,
    scripts: &[(&str, &str)],
    conffiles: &[String],
    out: &Path,
) -> Result<()> {
    let control_dir = work.join("control");
    fs::create_dir_all(&control_dir)?;
    fs::write(control_dir.join("control"), control)?;
    for (name, body) in scripts {
        let path = control_dir.join(name);
        fs::write(&path, body)?;
        set_executable(&path)?;
    }
    if !conffiles.is_empty() {
        fs::write(control_dir.join("conffiles"), conffiles.join("\n") + "\n")?;
    }
    fs::write(work.join("debian-binary"), "2.0\n")?;

    let owner = ["--owner=0", "--group=0", "--numeric-owner"];

    run(Command::new("tar")
        .args(owner)
        .args(["-I", "gzip -9", "-cf"])
        .arg(work.join("control.tar.gz"))
        .arg("-C")
        .arg(&control_dir)
        .arg("."))
    .context("failed to create control.tar.gz")?;

    // gzip -1: the initramfs is already compressed, a stronger level only costs time.
    run(Command::new("tar")
        .args(owner)
        .args(["-I", "gzip -1", "-cf"])
        .arg(work.join("data.tar.gz"))
        .arg("-C")
        .arg(stage)
        .arg("."))
    .context("failed to create data.tar.gz")?;

    let _ = fs::remove_file(out);
    if let Some(parent) = out.parent() {
        fs::create_dir_all(parent)?;
    }
    // `ar` runs inside `work`: a relative `out` would not point where we expect.
    let out = std::path::absolute(out).with_context(|| format!("invalid path {}", out.display()))?;
    run(Command::new("ar")
        .current_dir(work)
        .arg("rc")
        .arg(&out)
        .args(["debian-binary", "control.tar.gz", "data.tar.gz"]))
    .context("failed to assemble the .deb with ar")?;
    Ok(())
}

#[cfg(unix)]
fn set_executable(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o755))?;
    Ok(())
}

/* ------------------------------------------------------------------ */
/* Contenu des paquets                                                */
/* ------------------------------------------------------------------ */

fn copy_file(from: &Path, to: &Path, what: &str) -> Result<()> {
    if !from.is_file() {
        bail!("{what} not found: {}\n\nBuild it first: devtool build", from.display());
    }
    if let Some(parent) = to.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::copy(from, to).with_context(|| format!("cannot copy {}", from.display()))?;
    Ok(())
}

fn stage_kernel(p: &Paths, stage: &Path) -> Result<()> {
    copy_file(&p.kernel_image(), &stage.join("boot/vmlinuz"), "kernel image")?;
    copy_file(&p.initramfs(), &stage.join("boot/initramfs.cpio.gz"), "initramfs")?;
    Ok(())
}

fn stage_userspace(p: &Paths, stage: &Path) -> Result<()> {
    // Scripts, units and configuration of the rootfs overlay.
    copy_tree(&p.senbit_rootfs_overlay(), stage)?;

    // Scripts must be executable whatever the way the sources reached the build host
    // (a zip archive does not always keep the mode): same list as build/systemd.rs.
    for script in [
        "usr/lib/systemd/systemd-sysv-install",
        "usr/bin/ldd",
        "usr/bin/getconf",
        "usr/lib/senbit/grub-config",
        "usr/lib/senbit/boot-ok",
        "usr/lib/senbit/update-check",
        "usr/lib/senbit/update-snapshot",
        "usr/sbin/rollback",
    ] {
        let path = stage.join(script);
        if path.is_file() {
            set_executable(&path)?;
        }
    }

    // The login.
    let login = p.login_binary();
    copy_file(&login, &stage.join("usr/sbin/senbit-login"), "senbit-login")?;
    set_executable(&stage.join("usr/sbin/senbit-login"))?;

    // The native commands: the stage has bin/ and sbin/, a package installs in /usr.
    let commands = p.commands_stage_dir();
    if !commands.is_dir() {
        bail!("native commands not built: {}\n\nRun: devtool build commands", commands.display());
    }
    for (from, to) in [("bin", "usr/bin"), ("sbin", "usr/sbin"), ("usr/share", "usr/share")] {
        let source = commands.join(from);
        if source.is_dir() {
            copy_tree(&source, &stage.join(to))?;
        }
    }
    Ok(())
}

/* ------------------------------------------------------------------ */
/* Dépôt APT                                                          */
/* ------------------------------------------------------------------ */

struct Built {
    control: String,
    file: PathBuf,
    relative: String,
}

/// Une entrée du fichier `Packages` : le `control` suivi de Filename, Size, SHA256.
fn packages_stanza(control: &str, relative: &str, size: u64, sha256: &str) -> String {
    let mut text = control.trim_end().to_string();
    text.push_str(&format!("\nFilename: {relative}\nSize: {size}\nSHA256: {sha256}\n\n"));
    text
}

/// Fichier `Release` ; `files` = (chemin relatif à dists/<suite>/, taille, SHA256).
fn render_release(suite: &str, component: &str, date: &str, files: &[(String, u64, String)]) -> String {
    let mut text = String::new();
    text.push_str("Origin: Senbit\n");
    text.push_str("Label: Senbit\n");
    text.push_str(&format!("Suite: {suite}\n"));
    text.push_str(&format!("Codename: {suite}\n"));
    text.push_str(&format!("Date: {date}\n"));
    text.push_str("Architectures: amd64\n");
    text.push_str(&format!("Components: {component}\n"));
    text.push_str("Description: Senbit system updates (kernel, init, userspace)\n");
    text.push_str("SHA256:\n");
    for (path, size, hash) in files {
        text.push_str(&format!(" {hash} {size:>16} {path}\n"));
    }
    text
}

fn utc_date() -> String {
    Command::new("date")
        .args(["-u", "+%a, %d %b %Y %H:%M:%S UTC"])
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .filter(|d| !d.is_empty())
        .unwrap_or_else(|| "Thu, 01 Jan 1970 00:00:00 UTC".to_string())
}

fn gpg_available() -> bool {
    Command::new("gpg")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Signe `Release` : Release.gpg (signature détachée) et InRelease (signature intégrée).
fn sign_release(dist: &Path, key: &str) -> Result<()> {
    let release = dist.join("Release");

    run(Command::new("gpg")
        .args(["--batch", "--yes", "--default-key", key, "--armor", "--detach-sign", "-o"])
        .arg(dist.join("Release.gpg"))
        .arg(&release))
    .context("gpg could not sign Release (is the key available? `gpg --list-secret-keys`)")?;

    run(Command::new("gpg")
        .args(["--batch", "--yes", "--default-key", key, "--clearsign", "-o"])
        .arg(dist.join("InRelease"))
        .arg(&release))
    .context("gpg could not create InRelease")?;
    Ok(())
}

pub fn build(p: &Paths) -> Result<()> {
    let senbit = senbit_version(p)?;
    let version = deb_version(&senbit);
    let update = &p.cfg.update;

    let repo = p.r("build/repo");
    let work_root = p.r("build/repo-work");
    let _ = fs::remove_dir_all(&repo);
    let _ = fs::remove_dir_all(&work_root);
    fs::create_dir_all(&repo)?;

    ui::info(format!("Building the update packages for Senbit {senbit} ({version})..."));

    let mut built: Vec<Built> = Vec::new();

    for name in SENBIT_PACKAGES {
        let work = work_root.join(name);
        let stage = work.join("stage");
        fs::create_dir_all(&stage)?;

        let spin = ui::spinner(&format!("Packaging {name}..."));
        let result = (|| -> Result<Built> {
            let (summary, description, depends, scripts, conffiles): (&str, String, &str, Vec<(&str, &str)>, Vec<String>) =
                if *name == "senbit-kernel" {
                    stage_kernel(p, &stage)?;
                    (
                        "Senbit kernel and init (boot files)",
                        "The Linux kernel (/boot/vmlinuz) and the initramfs holding senbit-init\n(/boot/initramfs.cpio.gz).\nThe previous version is kept as /boot/vmlinuz.old and is booted again\nautomatically when the new one does not reach a working system."
                            .to_string(),
                        "senbit-userspace",
                        vec![("preinst", KERNEL_PREINST), ("postinst", KERNEL_POSTINST)],
                        Vec::new(),
                    )
                } else {
                    stage_userspace(p, &stage)?;
                    let conffiles = collect_conffiles(&stage);
                    (
                        "Senbit userspace (login, commands, services, configuration)",
                        "senbit-login, the native commands, the systemd units, the maintenance\nscripts and the configuration files of Senbit.\nFiles under /etc are configuration files: an update never overwrites\nyour changes."
                            .to_string(),
                        "",
                        vec![("postinst", USERSPACE_POSTINST)],
                        conffiles,
                    )
                };

            let control = render_control(&Package {
                name,
                version: &version,
                depends,
                summary,
                description: &description,
                installed_kib: dir_size_kib(&stage),
            });

            let file_name = format!("{name}_{version}_amd64.deb");
            let relative = format!("pool/{}/{}/{file_name}", update.component, name);
            let file = repo.join(&relative);
            build_deb(&work, &stage, &control, &scripts, &conffiles, &file)?;

            Ok(Built {
                control,
                file,
                relative,
            })
        })();
        spin.finish_and_clear();

        let package = result.with_context(|| format!("failed to build {name}"))?;
        let size = fs::metadata(&package.file)?.len();
        ui::detail(format!("{} ({:.1} MiB)", package.relative, size as f64 / 1_048_576.0));
        built.push(package);
    }

    // ---- index ----
    let binary = format!("dists/{}/{}/binary-amd64", update.suite, update.component);
    let binary_dir = repo.join(&binary);
    fs::create_dir_all(&binary_dir)?;

    let mut packages = String::new();
    for package in &built {
        let size = fs::metadata(&package.file)?.len();
        packages.push_str(&packages_stanza(
            &package.control,
            &package.relative,
            size,
            &sha256_hex(&package.file)?,
        ));
    }
    fs::write(binary_dir.join("Packages"), &packages)?;
    run(Command::new("gzip").args(["-9", "-k", "-f"]).arg(binary_dir.join("Packages")))
        .context("failed to compress Packages")?;

    // ---- Release (+ signature) ----
    let dist = repo.join(format!("dists/{}", update.suite));
    let mut files = Vec::new();
    for name in ["Packages", "Packages.gz"] {
        let path = binary_dir.join(name);
        files.push((
            format!("{}/binary-amd64/{name}", update.component),
            fs::metadata(&path)?.len(),
            sha256_hex(&path)?,
        ));
    }
    fs::write(
        dist.join("Release"),
        render_release(&update.suite, &update.component, &utc_date(), &files),
    )?;

    if update.gpg_key.is_empty() {
        ui::warn("`[update] gpg_key` is empty: the repository is NOT signed (local tests only).");
    } else if !gpg_available() {
        bail!("gpg is needed to sign the repository (`[update] gpg_key = \"{}\"`).", update.gpg_key);
    } else {
        sign_release(&dist, &update.gpg_key)?;
        ui::detail("Release signed (Release.gpg, InRelease).");
    }

    let _ = fs::remove_dir_all(&work_root);

    ui::ok(format!("Update repository ready: {}", repo.display()));
    ui::info("Publish it (web server, USB stick), or test it in the VM:");
    ui::info(format!("  (cd {} && python3 -m http.server 8000)", repo.display()));
    ui::info("  and in Senbit (QEMU): see docs/UPDATES.md");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_comes_from_cargo_toml() {
        let text = "[package]\nname = \"senbit\"\nversion = \"0.4.0\"\nedition = \"2024\"\n";
        assert_eq!(parse_version(text).as_deref(), Some("0.4.0"));
        assert_eq!(parse_version("[package]\nname = \"x\"\n"), None);
        assert_eq!(deb_version("0.4.0"), "0.4.0-1");
    }

    #[test]
    fn control_file_is_complete() {
        let control = render_control(&Package {
            name: "senbit-kernel",
            version: "0.4.0-1",
            depends: "senbit-userspace",
            summary: "Senbit kernel",
            description: "line one\n\nline three",
            installed_kib: 123,
        });
        assert!(control.starts_with("Package: senbit-kernel\nVersion: 0.4.0-1\nArchitecture: amd64\n"));
        assert!(control.contains("Installed-Size: 123\n"));
        assert!(control.contains("Depends: senbit-userspace\n"));
        assert!(control.contains("Description: Senbit kernel\n line one\n .\n line three\n"));

        let no_depends = render_control(&Package {
            name: "x",
            version: "1",
            depends: "",
            summary: "s",
            description: "d",
            installed_kib: 1,
        });
        assert!(!no_depends.contains("Depends:"));
    }

    #[test]
    fn packages_stanza_carries_the_hash_and_the_path() {
        let stanza = packages_stanza("Package: x\nVersion: 1\n", "pool/main/x/x_1_amd64.deb", 42, "abc123");
        assert!(stanza.contains("Filename: pool/main/x/x_1_amd64.deb\n"));
        assert!(stanza.contains("Size: 42\n"));
        assert!(stanza.contains("SHA256: abc123\n"));
        assert!(stanza.ends_with("\n\n"));
    }

    #[test]
    fn release_lists_the_index_files() {
        let files = vec![
            ("main/binary-amd64/Packages".to_string(), 100u64, "aa".to_string()),
            ("main/binary-amd64/Packages.gz".to_string(), 50u64, "bb".to_string()),
        ];
        let release = render_release("stable", "main", "Mon, 01 Jan 2029 00:00:00 UTC", &files);
        assert!(release.contains("Suite: stable\n"));
        assert!(release.contains("Components: main\n"));
        assert!(release.contains("Architectures: amd64\n"));
        assert!(release.contains("SHA256:\n aa "));
        assert!(release.contains("main/binary-amd64/Packages.gz\n"));
    }

    #[test]
    fn conffiles_are_the_regular_files_of_etc() {
        let dir = std::env::temp_dir().join(format!("devtool-conffiles-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("etc/profile.d")).unwrap();
        fs::create_dir_all(dir.join("usr/bin")).unwrap();
        fs::write(dir.join("etc/hosts"), "x").unwrap();
        fs::write(dir.join("etc/profile.d/senbit.sh"), "x").unwrap();
        fs::write(dir.join("usr/bin/tool"), "x").unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink("hosts", dir.join("etc/link")).unwrap();

        assert_eq!(
            collect_conffiles(&dir),
            vec!["/etc/hosts".to_string(), "/etc/profile.d/senbit.sh".to_string()]
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_deb_can_be_assembled() {
        if Command::new("ar").arg("--version").output().is_err() {
            return; // no binutils on this machine
        }
        let dir = std::env::temp_dir().join(format!("devtool-deb-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let stage = dir.join("stage");
        fs::create_dir_all(stage.join("usr/bin")).unwrap();
        fs::write(stage.join("usr/bin/hello"), "#!/bin/sh\n").unwrap();

        let out = dir.join("out/hello_1_amd64.deb");
        build_deb(
            &dir,
            &stage,
            "Package: hello\nVersion: 1\nArchitecture: amd64\nMaintainer: x\nDescription: x\n",
            &[("postinst", "#!/bin/sh\nexit 0\n")],
            &[],
            &out,
        )
        .unwrap();

        let bytes = fs::read(&out).unwrap();
        assert!(bytes.starts_with(b"!<arch>\n"));
        let _ = fs::remove_dir_all(&dir);
    }
}
