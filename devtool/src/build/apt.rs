//! apt + dpkg dans le rootfs, branchés sur un dépôt Debian DISTANT.
//!
//! `sudo apt install <paquet>` doit marcher dans Senbit. Plutôt que de
//! compiler apt et dpkg (des dizaines de dépendances de build qui diffèrent
//! entre Fedora, Ubuntu et Arch), devtool télécharge les .deb officiels du
//! dépôt configuré dans config.toml ([apt]), vérifie chaque fichier par
//! sha256 (via l'index Packages, lui-même vérifié par InRelease), les extrait
//! dans le rootfs et enregistre la base dpkg.
//!
//! Compatibilité :
//!   * la glibc, systemd et util-linux sont ceux de Senbit : ils sont déclarés
//!     « déjà installés » dans /var/lib/dpkg/status (paquets factices), donc
//!     apt ne tentera jamais de les remplacer ; /etc/apt/preferences.d/senbit
//!     le garantit aussi pour les mises à jour.
//!   * les binaires Debian 12 (glibc 2.36) tournent sur toute glibc plus récente.
//!   * les scripts de mainteneur (postinst...) ne sont PAS exécutés à la
//!     construction ; dpkg les exécutera normalement pour les paquets installés
//!     ensuite dans Senbit.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{anyhow, bail, Context, Result};

use crate::cache::Cache;
use crate::config::Apt;
use crate::get::{download, sha256_file};
use crate::paths::Paths;
use crate::proc::{capture, run_quiet};
use crate::{host, ui};

/* ------------------------------------------------------------------ */
/* Index Packages                                                     */
/* ------------------------------------------------------------------ */

#[derive(Debug, Default, Clone)]
pub struct Package {
    pub name: String,
    pub version: String,
    pub depends: String,
    pub provides: String,
    pub filename: String,
    pub sha256: String,
    pub size: u64,
}

/// Découpe un fichier Packages en stanzas (une par paquet).
pub fn parse_packages(text: &str) -> Vec<Package> {
    let mut out = Vec::new();
    for stanza in text.split("\n\n") {
        let mut pkg = Package::default();
        let mut pre_depends = String::new();
        let mut depends = String::new();
        for line in stanza.lines() {
            if line.starts_with(' ') || line.starts_with('\t') {
                continue; // champ multi-ligne (Description...)
            }
            let Some((key, value)) = line.split_once(':') else {
                continue;
            };
            let value = value.trim().to_string();
            match key {
                "Package" => pkg.name = value,
                "Version" => pkg.version = value,
                "Pre-Depends" => pre_depends = value,
                "Depends" => depends = value,
                "Provides" => pkg.provides = value,
                "Filename" => pkg.filename = value,
                "SHA256" => pkg.sha256 = value,
                "Size" => pkg.size = value.parse().unwrap_or(0),
                _ => {}
            }
        }
        if pkg.name.is_empty() || pkg.filename.is_empty() {
            continue;
        }
        pkg.depends = [pre_depends, depends]
            .into_iter()
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join(", ");
        out.push(pkg);
    }
    out
}

/// Noms de paquets d'un champ Depends/Provides : `[[alt1, alt2], [alt1], ...]`
/// (versions, architectures et `:any` retirés).
pub fn parse_relations(field: &str) -> Vec<Vec<String>> {
    field
        .split(',')
        .map(|group| {
            group
                .split('|')
                .filter_map(|alt| {
                    let name = alt.split(['(', '[', '<']).next()?.trim();
                    let name = name.split(':').next()?.trim();
                    (!name.is_empty()).then(|| name.to_string())
                })
                .collect::<Vec<_>>()
        })
        .filter(|g| !g.is_empty())
        .collect()
}

/// Clé de comparaison grossière d'une version Debian : ses suites de chiffres
/// ("2.36-9+deb12u13" -> [2, 36, 9, 12, 13]). Suffisant pour choisir la plus
/// récente des versions d'un même paquet.
fn version_key(version: &str) -> Vec<u64> {
    version
        .split(|c: char| !c.is_ascii_digit())
        .filter(|s| !s.is_empty())
        .map(|s| s.parse().unwrap_or(0))
        .collect()
}

/// Version de `libc6` dans le dépôt Debian (la plus récente de toutes les suites).
///
/// Senbit fournit lui-même la glibc (celle de la machine de build) et déclare
/// `libc6` installé. Avec la version du dépôt, les dépendances exactes de
/// `libc6-dev` (`libc6 (= 2.36-9+deb12uN)`) sont satisfaites : `apt install
/// gcc clang build-essential make` devient possible. Les en-têtes de Debian
/// (2.36) fonctionnent avec une glibc plus récente à l'exécution.
fn repo_libc6_version(index: &[Package]) -> String {
    index
        .iter()
        .filter(|p| p.name == "libc6")
        .map(|p| p.version.clone())
        .max_by_key(|v| version_key(v))
        .unwrap_or_else(|| "2.36-9+deb12u10".to_string())
}

/// Révision du format du stage (voir l'empreinte dans `build`).
const STAGE_REVISION: u32 = 3;

/// Paquets fournis par Senbit lui-même (jamais téléchargés) : déclarés
/// « installés » dans /var/lib/dpkg/status pour que apt n'essaie pas de les
/// installer ni de les remplacer.
///
/// Règle : seulement ce que Senbit fournit VRAIMENT (glibc de la machine de
/// build, systemd compilé, partitionnement statique, adduser/useradd natifs).
/// Tout le reste vient de Debian — y compris `libgcc-s1`, `libmount1`,
/// `libblkid1`, `libuuid1` et `init-system-helpers` (qui apporte update-rc.d,
/// invoke-rc.d, deb-systemd-helper : les scripts de mainteneur les appellent).
const PROVIDED: &[&str] = &[
    "libc6",
    // libc-bin exige libc6 << 2.37 : impossible avec la glibc de la machine de
    // build. ldconfig est copié de l'hôte, `getent` est une commande native
    // (commands/getent).
    "libc-bin",
    "systemd",
    "libsystemd0",
    "libudev1",
    "udev",
    "util-linux",
    "mount",
    "e2fsprogs",
    // Les scripts de mainteneur font `adduser --system ...` / `useradd ...` :
    // fournis par les commandes natives commands/adduser et commands/useradd.
    "adduser",
    "passwd",
    // base-files écraserait /usr/lib/os-release (ID=debian) ; base-passwd et
    // login dépendent de PAM. Senbit a ses propres comptes (sysusers.d) et
    // son propre login.
    "base-files",
    "base-passwd",
    "login",
    "sysvinit-utils",
];

/// Calcule la fermeture des dépendances de `roots`. Les alternatives
/// (`a | b`) prennent la première possible ; les contraintes de version sont
/// ignorées (un seul suite = un seul jeu cohérent).
pub fn resolve(index: &[Package], roots: &[String]) -> Result<Vec<Package>> {
    let mut by_name: BTreeMap<&str, &Package> = BTreeMap::new();
    let mut providers: BTreeMap<String, &Package> = BTreeMap::new();
    for pkg in index {
        by_name.entry(pkg.name.as_str()).or_insert(pkg);
        for group in parse_relations(&pkg.provides) {
            for virt in group {
                providers.entry(virt).or_insert(pkg);
            }
        }
    }

    let provided: BTreeSet<&str> = PROVIDED.iter().copied().collect();
    let mut chosen: BTreeMap<String, Package> = BTreeMap::new();
    let mut queue: Vec<String> = roots.to_vec();

    while let Some(name) = queue.pop() {
        if chosen.contains_key(&name) || provided.contains(name.as_str()) {
            continue;
        }
        let pkg = by_name
            .get(name.as_str())
            .copied()
            .or_else(|| providers.get(&name).copied())
            .ok_or_else(|| anyhow!("package `{name}` not found in the repository"))?;
        if chosen.contains_key(&pkg.name) {
            continue;
        }
        chosen.insert(pkg.name.clone(), pkg.clone());

        for group in parse_relations(&pkg.depends) {
            if group
                .iter()
                .any(|alt| provided.contains(alt.as_str()) || chosen.contains_key(alt))
            {
                continue;
            }
            let pick = group
                .iter()
                .find(|alt| {
                    by_name.contains_key(alt.as_str()) || providers.contains_key(alt.as_str())
                })
                .ok_or_else(|| {
                    anyhow!(
                        "{}: no candidate for dependency `{}`",
                        pkg.name,
                        group.join(" | ")
                    )
                })?;
            queue.push(pick.clone());
        }
    }
    Ok(chosen.into_values().collect())
}

/* ------------------------------------------------------------------ */
/* Téléchargement                                                     */
/* ------------------------------------------------------------------ */

fn url(cfg: &Apt, tail: &str) -> String {
    format!(
        "{}/{}",
        cfg.mirror.trim_end_matches('/'),
        tail.trim_start_matches('/')
    )
}

/// Cherche dans InRelease la ligne SHA256 de `path` : (sha256, taille).
fn release_entry(inrelease: &str, path: &str) -> Option<(String, u64)> {
    let mut in_sha256 = false;
    for line in inrelease.lines() {
        if line.starts_with("SHA256:") {
            in_sha256 = true;
            continue;
        }
        if in_sha256 {
            if !line.starts_with(' ') {
                in_sha256 = false;
                continue;
            }
            let mut f = line.split_whitespace();
            if let (Some(hash), Some(size), Some(name)) = (f.next(), f.next(), f.next()) {
                if name == path {
                    return Some((hash.to_lowercase(), size.parse().ok()?));
                }
            }
        }
    }
    None
}

/// Vérifie la signature d'InRelease avec gpgv si la machine a gpgv ET le
/// trousseau Debian ; sinon prévient (l'intégrité repose alors sur HTTPS).
fn verify_inrelease(file: &Path) {
    let keyring = [
        "/usr/share/keyrings/debian-archive-keyring.gpg",
        "/usr/share/keyrings/debian-archive-keyring.pgp",
    ]
    .iter()
    .map(PathBuf::from)
    .find(|p| p.is_file());

    match (host::find_tool("gpgv"), keyring) {
        (Some(gpgv), Some(keyring)) => {
            let ok = Command::new(gpgv)
                .arg("--keyring")
                .arg(&keyring)
                .arg("--output")
                .arg("/dev/null")
                .arg(file)
                .output()
                .map(|o| o.status.success())
                .unwrap_or(false);
            if ok {
                ui::detail("InRelease signature verified (gpgv).");
            } else {
                ui::warn("InRelease signature could NOT be verified with the host keyring.");
            }
        }
        _ => ui::warn("gpgv or debian-archive-keyring missing on this machine: signature not checked (HTTPS + sha256 only)."),
    }
}

struct Fetched {
    index: Vec<Package>,
    /// Empreinte de l'index (sert au cache de build).
    fingerprint: String,
}

fn fetch_index(p: &Paths) -> Result<Fetched> {
    let cfg = &p.cfg.apt;
    let dir = p.apt_cache_dir();
    fs::create_dir_all(&dir)?;

    let inrelease = dir.join("InRelease");
    download(
        &[url(cfg, &format!("dists/{}/InRelease", cfg.suite))],
        &inrelease,
    )?;
    verify_inrelease(&inrelease);
    let release_text = fs::read_to_string(&inrelease)?;

    let mut packages = Vec::new();
    let mut fingerprint = String::new();
    for component in &cfg.components {
        let rel = format!("{component}/binary-{}/Packages.xz", cfg.architecture);
        let (hash, _size) = release_entry(&release_text, &rel)
            .with_context(|| format!("{rel} not listed in InRelease"))?;
        let file = dir.join(format!("Packages-{component}.xz"));
        download(&[url(cfg, &format!("dists/{}/{rel}", cfg.suite))], &file)?;
        let actual = sha256_file(&file)?;
        if actual != hash {
            let _ = fs::remove_file(&file);
            bail!("{rel}: sha256 mismatch (index corrupted or changed during download).");
        }
        fingerprint.push_str(&hash);
        let text = capture(host::command("xz").arg("-dc").arg(&file))?;
        packages.extend(parse_packages(&text));
    }
    Ok(Fetched {
        index: packages,
        fingerprint,
    })
}

fn fetch_deb(p: &Paths, pkg: &Package) -> Result<PathBuf> {
    let name = Path::new(&pkg.filename)
        .file_name()
        .context("bad Filename")?;
    let dest = p.apt_cache_dir().join(name);
    if dest.is_file() && sha256_file(&dest)? == pkg.sha256.to_lowercase() {
        return Ok(dest);
    }
    download(&[url(&p.cfg.apt, &pkg.filename)], &dest)?;
    let actual = sha256_file(&dest)?;
    if actual != pkg.sha256.to_lowercase() {
        let _ = fs::remove_file(&dest);
        bail!(
            "{}: sha256 mismatch\n  expected {}\n  found    {actual}",
            pkg.name,
            pkg.sha256
        );
    }
    Ok(dest)
}

/* ------------------------------------------------------------------ */
/* Extraction                                                         */
/* ------------------------------------------------------------------ */

/// Fichiers d'un .deb : (liste data, champs control, scripts de mainteneur).
struct Unpacked {
    files: Vec<String>,
    control: String,
    md5sums: Option<String>,
    conffiles: Option<String>,
    scripts: Vec<(String, Vec<u8>)>,
}

fn unpack_deb(deb: &Path, stage: &Path, scratch: &Path) -> Result<Unpacked> {
    let _ = fs::remove_dir_all(scratch);
    fs::create_dir_all(scratch)?;
    run_quiet(host::command("ar").arg("x").arg(deb).current_dir(scratch))?;

    let find = |prefix: &str| -> Result<PathBuf> {
        fs::read_dir(scratch)?
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .find(|p| {
                p.file_name()
                    .map(|n| n.to_string_lossy().starts_with(prefix))
                    .unwrap_or(false)
            })
            .with_context(|| format!("{}: {prefix}* missing", deb.display()))
    };
    let data = find("data.tar")?;
    let control_tar = find("control.tar")?;

    let listing = capture(host::command("tar").arg("-tf").arg(&data)).with_context(|| {
        format!(
            "cannot read {} (compressed with xz or zstd: `devtool doctor` shows what is missing)",
            data.display()
        )
    })?;
    let files: Vec<String> = listing
        .lines()
        .map(|l| l.trim_start_matches("./").to_string())
        .filter(|l| !l.is_empty())
        .collect();

    run_quiet(
        // -p: a normal user would otherwise lose the setuid/setgid/sticky bits
        // of the package files (su, mount, crontab...).
        host::command("tar")
            .arg("-xpf")
            .arg(&data)
            .arg("-C")
            .arg(stage)
            .arg("--no-same-owner"),
    )?;

    let ctl = scratch.join("control");
    fs::create_dir_all(&ctl)?;
    run_quiet(
        host::command("tar")
            .arg("-xf")
            .arg(&control_tar)
            .arg("-C")
            .arg(&ctl),
    )?;

    let read = |n: &str| fs::read_to_string(ctl.join(n)).ok();
    let mut scripts = Vec::new();
    for s in [
        "preinst",
        "postinst",
        "prerm",
        "postrm",
        "triggers",
        "config",
        "templates",
        "shlibs",
    ] {
        if let Ok(bytes) = fs::read(ctl.join(s)) {
            scripts.push((s.to_string(), bytes));
        }
    }
    Ok(Unpacked {
        files,
        control: read("control").context("control file missing")?,
        md5sums: read("md5sums"),
        conffiles: read("conffiles"),
        scripts,
    })
}

/// Stanza de /var/lib/dpkg/status pour un paquet installé.
fn status_stanza(control: &str) -> String {
    let mut out = String::from("Status: install ok installed\n");
    out.push_str(control.trim_end());
    out.push_str("\n\n");
    out
}

fn stub_stanza(name: &str, version: &str) -> String {
    format!(
        "Package: {name}\nStatus: install ok installed\nPriority: required\nSection: admin\nInstalled-Size: 0\nMaintainer: Senbit <senbit@localhost>\nArchitecture: amd64\nVersion: {version}\nDescription: provided natively by Senbit (not a Debian package)\n\n"
    )
}

/// Entrée de /var/lib/dpkg/status d'un composant de Senbit (paquet du dépôt de mise à jour).
fn senbit_stanza(name: &str, version: &str) -> String {
    format!(
        "Package: {name}\nStatus: install ok installed\nPriority: required\nSection: admin\nInstalled-Size: 0\nMaintainer: Senbit <senbit@localhost>\nArchitecture: amd64\nVersion: {version}\nDescription: Senbit component, updated through the Senbit repository\n\n"
    )
}

/// Source APT du dépôt de mise à jour de Senbit (`[update]` de config.toml).
/// `signed` : la clé publique est dans l'image (Signed-By) ; sinon dépôt non signé,
/// pour des tests locaux seulement.
fn render_core_sources(repository: &str, suite: &str, component: &str, signed: bool) -> String {
    let trust = if signed {
        "Signed-By: /usr/share/keyrings/senbit-archive-keyring.gpg\n"
    } else {
        "# UNSIGNED repository (no key in config/update/): local tests only.\nTrusted: yes\n"
    };
    format!(
        "# Senbit system updates: senbit-kernel and senbit-userspace (docs/UPDATES.md).\nTypes: deb\nURIs: {}\nSuites: {suite}\nComponents: {component}\nArchitectures: amd64\n{trust}",
        repository.trim_end_matches('/')
    )
}

/// Installe la source APT des mises à jour et la clé publique. Sans `[update] repository`
/// il n'y a pas de canal de mise à jour dans l'image.
fn install_update_channel(p: &Paths, rootfs: &Path) -> Result<()> {
    let update = &p.cfg.update;
    if update.repository.trim().is_empty() {
        ui::info("No update repository configured (`[update] repository`): no update channel in this image.");
        return Ok(());
    }

    let key = p.r(&update.keyring);
    let signed = key.is_file();
    if signed {
        let target = rootfs.join("usr/share/keyrings/senbit-archive-keyring.gpg");
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::copy(&key, &target)
            .with_context(|| format!("failed to install the update key {}", key.display()))?;
    } else {
        ui::warn(format!(
            "Update key {} not found: the update repository is declared UNSIGNED.",
            key.display()
        ));
    }

    fs::write(
        rootfs.join("etc/apt/sources.list.d/senbit-core.sources"),
        render_core_sources(&update.repository, &update.suite, &update.component, signed),
    )?;
    Ok(())
}

/// Version de la glibc de la machine de build (copiée dans le rootfs).
fn host_glibc_version() -> String {
    host::command("ldd")
        .arg("--version")
        .output()
        .ok()
        .and_then(|o| {
            String::from_utf8_lossy(&o.stdout)
                .lines()
                .next()
                .map(str::to_string)
        })
        .and_then(|l| l.split_whitespace().last().map(str::to_string))
        .filter(|v| {
            v.chars()
                .next()
                .map(|c| c.is_ascii_digit())
                .unwrap_or(false)
        })
        .unwrap_or_else(|| "2.36".to_string())
}

/* ------------------------------------------------------------------ */
/* Étapes publiques                                                   */
/* ------------------------------------------------------------------ */

fn stage_dir(p: &Paths) -> PathBuf {
    p.r("build/apt/stage")
}

/// `devtool build apt` : télécharge, vérifie et extrait les paquets amorces
/// dans build/apt/stage (rootfs partiel prêt à être fusionné).
pub fn build(p: &Paths, cache: &mut Cache) -> Result<()> {
    for tool in ["ar", "tar", "xz", "curl"] {
        if !host::has_tool(tool) {
            bail!("`{tool}` not found. Run `devtool doctor`.");
        }
    }
    let cfg = &p.cfg.apt;
    let stage = stage_dir(p);

    ui::info(format!(
        "Repository: {} {} ({})",
        cfg.mirror, cfg.suite, cfg.architecture
    ));
    let fetched = fetch_index(p)?;
    ui::detail(format!("{} packages in the index.", fetched.index.len()));

    // The fingerprint covers everything that changes the staged files: the
    // repository state, the packages asked for, the list of packages Senbit
    // provides itself, and the extraction method (STAGE_REVISION: bump it when
    // unpack_deb changes, e.g. `tar -p` keeping the setuid bits).
    let fp = format!(
        "{}|{}|{}|{}|rev{}",
        fetched.fingerprint,
        cfg.bootstrap.join(","),
        cfg.suite,
        PROVIDED.join(","),
        STAGE_REVISION
    );
    if cache.is_fresh("apt", &fp, &[&stage.join("var/lib/dpkg/status.senbit")]) {
        ui::skip("apt/dpkg already staged for this repository state - skipping.");
        return Ok(());
    }

    let packages = resolve(&fetched.index, &cfg.bootstrap)?;
    ui::info(format!(
        "{} packages to install: {}",
        packages.len(),
        packages
            .iter()
            .map(|p| p.name.as_str())
            .collect::<Vec<_>>()
            .join(" ")
    ));

    let _ = fs::remove_dir_all(&stage);
    fs::create_dir_all(&stage)?;
    let info = stage.join("var/lib/dpkg/info");
    fs::create_dir_all(&info)?;
    fs::write(
        stage.join("var/lib/dpkg/libc6.version"),
        format!("{}\n", repo_libc6_version(&fetched.index)),
    )?;

    let mut status = String::new();
    let scratch = p.r("build/apt/scratch");
    for pkg in &packages {
        let spin = ui::spinner(&format!("{} {}", pkg.name, pkg.version));
        let deb = fetch_deb(p, pkg)?;
        let unpacked = unpack_deb(&deb, &stage, &scratch)?;
        spin.finish_and_clear();

        let mut list = String::new();
        for f in &unpacked.files {
            list.push_str(&format!("/{}\n", f.trim_end_matches('/')));
        }
        // dpkg wants "/." for the root directory.
        fs::write(
            info.join(format!("{}.list", pkg.name)),
            format!("/.\n{list}"),
        )?;
        if let Some(m) = &unpacked.md5sums {
            fs::write(info.join(format!("{}.md5sums", pkg.name)), m)?;
        }
        if let Some(c) = &unpacked.conffiles {
            fs::write(info.join(format!("{}.conffiles", pkg.name)), c)?;
        }
        for (name, bytes) in &unpacked.scripts {
            let path = info.join(format!("{}.{name}", pkg.name));
            fs::write(&path, bytes)?;
            fs::set_permissions(&path, fs::Permissions::from_mode(0o755))?;
        }
        status.push_str(&status_stanza(&unpacked.control));
        ui::detail(format!("{} {}", pkg.name, pkg.version));
    }
    let _ = fs::remove_dir_all(&scratch);

    fs::write(stage.join("var/lib/dpkg/status.senbit"), status)?;
    cache.record("apt", &fp)?;
    ui::ok("apt and dpkg staged.");
    Ok(())
}

/// Fusionne un fichier ou lien du stage dans le rootfs sans JAMAIS écrire à
/// travers un lien symbolique (ex. /bin/tar -> busybox).
fn merge_tree(src: &Path, dst: &Path) -> Result<()> {
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let from = entry.path();
        let to = dst.join(entry.file_name());
        let meta = fs::symlink_metadata(&from)?;

        if meta.file_type().is_symlink() {
            if fs::symlink_metadata(&to).is_ok() {
                if fs::symlink_metadata(&to)?.is_dir() {
                    continue; // keep Senbit's directory
                }
                fs::remove_file(&to)?;
            }
            std::os::unix::fs::symlink(fs::read_link(&from)?, &to)?;
        } else if meta.is_dir() {
            // A symlinked directory in the rootfs (e.g. lib/systemd) is kept.
            if !to.is_dir() {
                if fs::symlink_metadata(&to).is_ok() {
                    fs::remove_file(&to)?;
                }
                fs::create_dir_all(&to)?;
            }
            fs::set_permissions(
                &to,
                fs::Permissions::from_mode(meta.permissions().mode() & 0o7777),
            )
            .ok();
            merge_tree(&from, &to)?;
        } else if meta.is_file() {
            if fs::symlink_metadata(&to).is_ok() {
                fs::remove_file(&to)?;
            }
            fs::copy(&from, &to).with_context(|| format!("install {}", to.display()))?;
        }
    }
    Ok(())
}

const SOURCES_TEMPLATE: &str = "\
# Senbit: Debian repository used by `apt` (generated by devtool from config.toml).
Types: deb
URIs: {mirror}
Suites: {suite} {suite}-updates
Components: {components}
Architectures: {arch}
Signed-By: /usr/share/keyrings/debian-archive-keyring.gpg

Types: deb
URIs: {security}
Suites: {suite}-security
Components: {components}
Architectures: {arch}
Signed-By: /usr/share/keyrings/debian-archive-keyring.gpg
";

const APT_CONF: &str = "\
// Senbit apt defaults.
APT::Architecture \"amd64\";
APT::Install-Recommends \"false\";
Acquire::Languages \"none\";
// Sandboxed downloads run as this user (created by sysusers.d/senbit.conf).
APT::Sandbox::User \"_apt\";
// Keep the library cache fresh after every dpkg run.
DPkg::Post-Invoke { \"/sbin/ldconfig 2>/dev/null || true\"; };
Dpkg::Options { \"--force-confold\"; };
";

/// Ne jamais remplacer ce que Senbit fournit lui-même.
const APT_PREFERENCES: &str = "\
# Senbit provides these natively: apt must never install or upgrade them.
Package: libc6 libc-bin locales systemd systemd-sysv systemd-timesyncd libsystemd0 libsystemd-shared libpam-systemd udev libudev1 util-linux mount sysvinit-core upstart login passwd base-files base-passwd sysvinit-utils adduser e2fsprogs
Pin: release *
Pin-Priority: -1
";

/// Fusionne le stage dans le rootfs et écrit toute la configuration d'apt.
pub fn install_into_rootfs(p: &Paths, rootfs: &Path) -> Result<()> {
    let stage = stage_dir(p);
    if !stage.join("var/lib/dpkg/status.senbit").is_file() {
        bail!("apt has not been staged.\n\nRun:\n  devtool build apt");
    }
    let cfg = &p.cfg.apt;

    merge_tree(&stage, rootfs)?;

    // --- base dpkg ---
    let dpkg = rootfs.join("var/lib/dpkg");
    for d in ["info", "updates", "triggers", "alternatives", "parts"] {
        fs::create_dir_all(dpkg.join(d))?;
    }
    let _ = fs::remove_file(dpkg.join("status.senbit"));
    fs::write(dpkg.join("arch"), format!("{}\n", cfg.architecture))?;
    fs::write(dpkg.join("available"), "")?;

    let systemd_version = fs::read_to_string(p.systemd_version_file())
        .map(|v| v.trim().trim_start_matches('v').to_string())
        .unwrap_or_else(|_| "257".into());
    let util_version = fs::read_to_string(p.util_linux_version_file())
        .map(|v| v.trim().to_string())
        .unwrap_or_else(|_| "2.41".into());
    let glibc = host_glibc_version();

    // Version declared for libc6 / libc-bin (see repo_libc6_version).
    let libc6_version = fs::read_to_string(stage.join("var/lib/dpkg/libc6.version"))
        .map(|v| v.trim().to_string())
        .ok()
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| format!("{glibc}-senbit1"));
    let _ = fs::remove_file(dpkg.join("libc6.version"));

    // The glibc really shipped (read by /usr/bin/ldd and /usr/bin/getconf).
    fs::create_dir_all(rootfs.join("usr/share/senbit"))?;
    fs::write(
        rootfs.join("usr/share/senbit/glibc-version"),
        format!("{glibc}\n"),
    )?;

    let mut status = fs::read_to_string(stage.join("var/lib/dpkg/status.senbit"))?;
    for name in PROVIDED {
        let version = match *name {
            "libc6" | "libc-bin" => libc6_version.clone(),
            "libgcc-s1" => "14.2.0-senbit1".to_string(),
            "systemd" | "libsystemd0" | "libudev1" | "udev" => format!("{systemd_version}-senbit1"),
            "util-linux" | "mount" | "libmount1" | "libblkid1" | "libuuid1" => {
                format!("{util_version}-senbit1")
            }
            "e2fsprogs" => "1.47.0-senbit1".to_string(),
            // Debian numbers these with an epoch: `login (>= 1:4.13)` would
            // not be satisfied by a version without it.
            "login" | "passwd" => "1:4.13+dfsg1-1+senbit1".to_string(),
            "base-passwd" => "3.6.1+senbit1".to_string(),
            _ => "999-senbit1".to_string(),
        };
        status.push_str(&stub_stanza(name, &version));

        // dpkg warns "files list file for package 'x' missing; assuming package
        // has no files currently installed" for every package without a .list
        // file: a provided package owns no file, its list is empty.
        let list = dpkg.join("info").join(format!("{name}.list"));
        if !list.exists() {
            fs::write(&list, "/.\n")?;
        }
    }

    // The Senbit components themselves (kernel/init and userspace) are real packages
    // of the update repository: declared installed at the version of this build, so
    // that `apt upgrade` offers the next one (docs/UPDATES.md).
    let senbit_version = crate::build::package::deb_version(&crate::build::package::senbit_version(p)?);
    for name in crate::build::package::SENBIT_PACKAGES {
        status.push_str(&senbit_stanza(name, &senbit_version));
        let list = dpkg.join("info").join(format!("{name}.list"));
        if !list.exists() {
            fs::write(&list, "/.\n")?;
        }
    }
    fs::write(dpkg.join("status"), status)?;

    // --- configuration d'apt / dpkg ---
    for d in [
        "etc/apt/sources.list.d",
        "etc/apt/apt.conf.d",
        "etc/apt/preferences.d",
        "etc/apt/trusted.gpg.d",
        "etc/dpkg/dpkg.cfg.d",
        "var/lib/apt/lists/partial",
        "var/cache/apt/archives/partial",
        "var/log/apt",
    ] {
        fs::create_dir_all(rootfs.join(d))?;
    }
    fs::write(
        rootfs.join("etc/apt/sources.list.d/senbit.sources"),
        SOURCES_TEMPLATE
            .replace("{mirror}", cfg.mirror.trim_end_matches('/'))
            .replace("{security}", cfg.security_mirror.trim_end_matches('/'))
            .replace("{suite}", &cfg.suite)
            .replace("{components}", &cfg.components.join(" "))
            .replace("{arch}", &cfg.architecture),
    )?;
    install_update_channel(p, rootfs)?;
    fs::write(rootfs.join("etc/apt/apt.conf.d/10senbit"), APT_CONF)?;
    fs::write(rootfs.join("etc/apt/preferences.d/senbit"), APT_PREFERENCES)?;
    fs::write(
        rootfs.join("etc/dpkg/dpkg.cfg.d/senbit"),
        "no-debsig\nforce-unsafe-io\n",
    )?;
    // Minimal dpkg configuration: the files that dpkg and its tools expect to find.
    let dpkg_cfg = rootfs.join("etc/dpkg/dpkg.cfg");
    if !dpkg_cfg.is_file() {
        fs::write(
            &dpkg_cfg,
            "# dpkg configuration (Senbit).\n# The options are in /etc/dpkg/dpkg.cfg.d/ ; see dpkg(1).\n",
        )?;
    }
    for name in ["diversions", "statoverride"] {
        let file = dpkg.join(name);
        if !file.exists() {
            fs::write(&file, "")?;
        }
    }
    // The legacy file must exist (empty) or apt warns at every run.
    fs::write(
        rootfs.join("etc/apt/sources.list"),
        "# See /etc/apt/sources.list.d/senbit.sources\n",
    )?;

    // --- bibliothèques Debian (multiarch) ---
    fs::create_dir_all(rootfs.join("etc/ld.so.conf.d"))?;
    fs::write(
        rootfs.join("etc/ld.so.conf.d/x86_64-linux-gnu.conf"),
        "/lib/x86_64-linux-gnu\n/usr/lib/x86_64-linux-gnu\n/usr/local/lib\n",
    )?;
    // Senbit's own libraries (libsystemd.so.0, libudev.so.1... live in
    // /usr/lib) and the host glibc layout (/usr/lib64 on Fedora): not in the
    // default directories of every ldconfig, so list them explicitly.
    fs::write(
        rootfs.join("etc/ld.so.conf.d/senbit.conf"),
        "/usr/lib\n/usr/lib64\n",
    )?;

    // --- glibc complète : libm, libresolv, libnss_*... ---
    // Seules les bibliothèques liées par un binaire de Senbit sont copiées
    // ailleurs ; les programmes Debian (python, postgresql...) chargent aussi
    // libm.so.6, libresolv.so.2, libnss_*.so.2, libthread_db...
    install_glibc_runtime(rootfs)?;
    check_host_glibc();

    // --- compatibilité des scripts de mainteneur ---
    for d in [
        "etc/alternatives",
        "var/lib/dpkg/alternatives",
        "var/cache/debconf",
        "var/lib/systemd",
        "usr/share/doc",
    ] {
        fs::create_dir_all(rootfs.join(d))?;
    }
    // dpkg-vendor / dpkg-buildflags read /etc/dpkg/origins/default (its
    // postinst normally creates the link, and postinst scripts are not run at
    // build time). Senbit installs Debian packages: vendor = debian.
    let origins = rootfs.join("etc/dpkg/origins");
    if origins.join("debian").is_file() {
        let default = origins.join("default");
        if fs::symlink_metadata(&default).is_ok() {
            fs::remove_file(&default)?;
        }
        std::os::unix::fs::symlink("debian", &default)?;
    }

    // --- D-Bus : systemctl pour un utilisateur normal ---
    // Sans bus système, `systemctl status` / `--failed` / `is-system-running`
    // répondent « Failed to connect to system scope bus » à tout utilisateur
    // non root (root passe par le socket privé de systemd). Le paquet `dbus`
    // arrive avec le bootstrap ; son postinst n'est pas exécuté au build, donc
    // on active ici le socket (dbus.service est lancé à la première connexion).
    // L'utilisateur `messagebus` vient de rootfs/usr/lib/sysusers.d/senbit.conf.
    let dbus_socket = rootfs.join("usr/lib/systemd/system/dbus.socket");
    if dbus_socket.is_file() {
        let wants = rootfs.join("etc/systemd/system/sockets.target.wants");
        fs::create_dir_all(&wants)?;
        let link = wants.join("dbus.socket");
        if fs::symlink_metadata(&link).is_err() {
            std::os::unix::fs::symlink("/usr/lib/systemd/system/dbus.socket", &link)?;
        }
    }

    // --- certificats TLS (le postinst de ca-certificates n'a pas tourné) ---
    build_ca_bundle(rootfs)?;

    // --- cache du chargeur dynamique, sans root : ldconfig -r <rootfs> ---
    match host::find_tool("ldconfig") {
        Some(ldconfig) => {
            let ok = Command::new(ldconfig)
                .arg("-r")
                .arg(rootfs)
                .output()
                .map(|o| o.status.success())
                .unwrap_or(false);
            if !ok {
                ui::warn("ldconfig -r failed: the library cache will be built at first boot (ldconfig.service).");
            }
        }
        None => ui::warn(
            "ldconfig not found on this machine: the library cache will be built at first boot.",
        ),
    }

    ui::ok("apt and dpkg installed in the rootfs.");
    Ok(())
}

/// Bibliothèques de la glibc de la machine de build que les programmes Debian
/// chargent mais qu'aucun binaire Senbit ne lie (donc jamais vues par `ldd`).
/// (soname, obligatoire)
const GLIBC_SONAMES: &[(&str, bool)] = &[
    ("libc.so.6", true),
    ("ld-linux-x86-64.so.2", true),
    ("libm.so.6", true),
    ("libdl.so.2", false),
    ("libpthread.so.0", false),
    ("librt.so.1", false),
    ("libutil.so.1", false),
    ("libresolv.so.2", false),
    ("libanl.so.1", false),
    ("libnss_files.so.2", false),
    ("libnss_dns.so.2", false),
    ("libnss_compat.so.2", false),
    ("libnss_hesiod.so.2", false),
    ("libBrokenLocale.so.1", false),
    ("libthread_db.so.1", false),
    ("libmvec.so.1", false),
];

/// Copie ces bibliothèques de l'hôte au même chemin dans le rootfs.
fn install_glibc_runtime(rootfs: &Path) -> Result<()> {
    for (soname, required) in GLIBC_SONAMES {
        let Some(path) = crate::build::systemd::host_library(soname) else {
            if *required {
                bail!(
                    "{soname} not found on the build host (glibc runtime).\n\n\
                     Run `devtool doctor` to see what to install."
                );
            }
            continue;
        };

        let dest = rootfs.join(path.trim_start_matches('/'));
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent)?;
        }
        if fs::symlink_metadata(&dest).is_ok() {
            fs::remove_file(&dest)?;
        }
        fs::copy(&path, &dest).with_context(|| format!("failed to install {path}"))?;
        fs::set_permissions(&dest, fs::Permissions::from_mode(0o755))?;

        // Debian's libc6-dev linker scripts (libc.so, libm.so) name
        // /lib/x86_64-linux-gnu/libc.so.6 and friends: make the libraries
        // visible there too, whatever the layout of the build host (Fedora keeps
        // them in /usr/lib64). Without it `gcc` fails to link.
        let multiarch = rootfs.join("usr/lib/x86_64-linux-gnu");
        fs::create_dir_all(&multiarch)?;
        let alias = multiarch.join(soname);
        if fs::symlink_metadata(&alias).is_err() {
            std::os::unix::fs::symlink(format!("/{}", path.trim_start_matches('/')), &alias)?;
        }
    }
    Ok(())
}

/// Vrai si `version` ("2.39", "2.36-9"...) est au moins 2.36 (glibc de Debian 12).
fn glibc_at_least_2_36(version: &str) -> bool {
    let mut parts = version
        .split(|c: char| !c.is_ascii_digit())
        .filter(|s| !s.is_empty())
        .map(|s| s.parse::<u32>().unwrap_or(0));
    let major = parts.next().unwrap_or(0);
    let minor = parts.next().unwrap_or(0);
    (major, minor) >= (2, 36)
}

/// Les paquets Debian 12 demandent `libc6 (>= 2.36)` : avec une glibc plus
/// ancienne sur la machine de build, apt refuserait de les installer.
fn check_host_glibc() {
    let version = host_glibc_version();
    if !glibc_at_least_2_36(&version) {
        ui::warn(format!(
            "The build host has glibc {version}, older than Debian 12's 2.36: \
             `apt install` will refuse most packages. Build on Ubuntu 24.04, \
             Fedora, Arch or Debian 12+."
        ));
    }
}

/// /etc/ssl/certs/ca-certificates.crt à partir des certificats Mozilla du paquet.
fn build_ca_bundle(rootfs: &Path) -> Result<()> {
    let src = rootfs.join("usr/share/ca-certificates");
    if !src.is_dir() {
        return Ok(());
    }
    let mut files = Vec::new();
    collect_crt(&src, &mut files)?;
    files.sort();
    let mut bundle = String::new();
    for f in &files {
        bundle.push_str(&fs::read_to_string(f)?);
        if !bundle.ends_with('\n') {
            bundle.push('\n');
        }
    }
    let certs = rootfs.join("etc/ssl/certs");
    fs::create_dir_all(&certs)?;
    fs::write(certs.join("ca-certificates.crt"), bundle)?;
    Ok(())
}

fn collect_crt(dir: &Path, out: &mut Vec<PathBuf>) -> Result<()> {
    for entry in fs::read_dir(dir)? {
        let path = entry?.path();
        if path.is_dir() {
            collect_crt(&path, out)?;
        } else if path.extension().and_then(|e| e.to_str()) == Some("crt") {
            out.push(path);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn update_source_is_signed_when_the_key_is_there() {
        let signed = render_core_sources("https://updates.example.org/senbit/", "stable", "main", true);
        assert!(signed.contains("URIs: https://updates.example.org/senbit\n"));
        assert!(signed.contains("Suites: stable\n"));
        assert!(signed.contains("Signed-By: /usr/share/keyrings/senbit-archive-keyring.gpg\n"));
        assert!(!signed.contains("Trusted"));

        let unsigned = render_core_sources("http://10.0.2.2:8000", "stable", "main", false);
        assert!(unsigned.contains("Trusted: yes\n"));
        assert!(!unsigned.contains("Signed-By"));
    }

    #[test]
    fn senbit_components_are_installed_packages() {
        let stanza = senbit_stanza("senbit-kernel", "0.4.0-1");
        assert!(stanza.contains("Package: senbit-kernel\n"));
        assert!(stanza.contains("Status: install ok installed\n"));
        assert!(stanza.contains("Version: 0.4.0-1\n"));
    }

    #[test]
    fn libc6_version_is_the_newest_of_the_repository() {
        let index = vec![
            Package { name: "libc6".into(), version: "2.36-9+deb12u7".into(), ..Default::default() },
            Package { name: "libc6".into(), version: "2.36-9+deb12u13".into(), ..Default::default() },
            Package { name: "libc6".into(), version: "2.36-9+deb12u10".into(), ..Default::default() },
            Package { name: "other".into(), version: "9.9".into(), ..Default::default() },
        ];
        assert_eq!(repo_libc6_version(&index), "2.36-9+deb12u13");
        assert_eq!(repo_libc6_version(&[]), "2.36-9+deb12u10");
        assert_eq!(version_key("2.36-9+deb12u13"), vec![2, 36, 9, 12, 13]);
    }

    #[test]
    fn libc_dev_is_no_longer_pinned_away() {
        assert!(!APT_PREFERENCES.contains("libc6-dev"));
        assert!(APT_PREFERENCES.contains("Package: libc6 libc-bin"));
    }

    #[test]
    fn glibc_version_check() {
        assert!(glibc_at_least_2_36("2.36"));
        assert!(glibc_at_least_2_36("2.41-senbit1"));
        assert!(glibc_at_least_2_36("2.39"));
        assert!(!glibc_at_least_2_36("2.35"));
        assert!(!glibc_at_least_2_36("1.99"));
    }

    #[test]
    fn provided_packages_are_not_downloaded() {
        // libgcc-s1 / init-system-helpers must come from Debian.
        assert!(!PROVIDED.contains(&"libgcc-s1"));
        assert!(!PROVIDED.contains(&"init-system-helpers"));
        assert!(PROVIDED.contains(&"adduser"));
    }

    use super::*;

    const SAMPLE: &str = "\
Package: apt
Version: 2.6.1
Depends: adduser, gpgv | gpgv2, libapt-pkg6.0 (>= 2.6.1), libc6 (>= 2.34), libgnutls30 (>= 3.7.5)
Filename: pool/main/a/apt/apt_2.6.1_amd64.deb
SHA256: aa
Size: 10
Description: commandline package manager
 long text

Package: libapt-pkg6.0
Version: 2.6.1
Depends: libc6, libgnutls30
Filename: pool/main/a/apt/libapt-pkg6.0_2.6.1_amd64.deb
SHA256: bb
Size: 20

Package: libgnutls30
Version: 3.7.9
Depends: libc6
Provides: libgnutls-virtual
Filename: pool/main/g/gnutls28/libgnutls30_amd64.deb
SHA256: cc
Size: 30

Package: gpgv
Version: 2.2
Filename: pool/main/g/gnupg2/gpgv.deb
SHA256: dd
Size: 5
";

    #[test]
    fn parses_stanzas() {
        let pkgs = parse_packages(SAMPLE);
        assert_eq!(pkgs.len(), 4);
        assert_eq!(pkgs[0].name, "apt");
        assert_eq!(pkgs[0].size, 10);
    }

    #[test]
    fn parses_relations() {
        let rel = parse_relations("gpgv | gpgv2, libc6 (>= 2.34), python3:any");
        assert_eq!(rel[0], vec!["gpgv", "gpgv2"]);
        assert_eq!(rel[1], vec!["libc6"]);
        assert_eq!(rel[2], vec!["python3"]);
    }

    #[test]
    fn resolves_and_skips_provided() {
        let pkgs = parse_packages(SAMPLE);
        let resolved = resolve(&pkgs, &["apt".to_string()]).unwrap();
        let names: Vec<&str> = resolved.iter().map(|p| p.name.as_str()).collect();
        assert!(names.contains(&"apt"));
        assert!(names.contains(&"libapt-pkg6.0"));
        assert!(names.contains(&"libgnutls30"));
        assert!(names.contains(&"gpgv"));
        // provided by Senbit: never downloaded
        assert!(!names.contains(&"libc6"));
        assert!(!names.contains(&"adduser"));
    }

    #[test]
    fn missing_package_is_an_error() {
        let pkgs = parse_packages(SAMPLE);
        assert!(resolve(&pkgs, &["nope".to_string()]).is_err());
    }

    #[test]
    fn finds_release_entry() {
        let text = "Suite: x\nSHA256:\n abc 123 main/binary-amd64/Packages.xz\n def 5 main/Contents-amd64.gz\nOther: y\n";
        assert_eq!(
            release_entry(text, "main/binary-amd64/Packages.xz"),
            Some(("abc".into(), 123))
        );
        assert_eq!(release_entry(text, "missing"), None);
    }
}
