//! systemd — dépôt épinglé dans external/systemd (tag config/systemd/version,
//! commit attendu config/systemd/sha). Compilé avec meson, installé dans
//! build/systemd/_install puis copié dans le rootfs avec ses bibliothèques.

use std::collections::BTreeSet;
use std::collections::hash_map::DefaultHasher;
use std::fs;
use std::hash::{Hash, Hasher};
use std::os::unix::fs::{symlink, PermissionsExt};
use std::path::Path;
use std::process::Command;

use anyhow::{bail, Context, Result};

use crate::cache::Cache;
use crate::paths::Paths;
use crate::proc::{capture, run_quiet};
use crate::{gitutil, ui};

const OPTIONS: &[&str] = &[
    "-Dmode=release", "-Dsplit-bin=true", "-Dlibdir=lib", "-Dsysconfdir=/etc",
    "-Dlocalstatedir=/var", "-Dfallback-default-target=multi-user.target",
    "-Dman=disabled", "-Dhtml=disabled", "-Dtranslations=false", "-Dtests=false",
    "-Dinstall-tests=false", "-Dbootloader=disabled", "-Defi=false", "-Dukify=disabled",
    "-Dkernel-install=false", "-Danalyze=false", "-Dseccomp=disabled",
    "-Dselinux=disabled", "-Dapparmor=disabled", "-Dpolkit=disabled", "-Dacl=disabled",
    "-Daudit=disabled", "-Dpam=disabled", "-Dkmod=disabled", "-Dmicrohttpd=disabled",
    "-Dlibcryptsetup=disabled", "-Dlibcurl=disabled", "-Dopenssl=disabled",
    "-Dgcrypt=disabled", "-Dgnutls=disabled", "-Dp11kit=disabled", "-Dlibfido2=disabled",
    "-Dtpm2=disabled", "-Delfutils=disabled", "-Dzlib=disabled", "-Dbzip2=disabled",
    "-Dxz=disabled", "-Dlz4=disabled", "-Dzstd=disabled", "-Dpcre2=disabled",
    "-Dglib=disabled", "-Ddbus=disabled", "-Dlibarchive=disabled", "-Dqrencode=disabled",
    "-Dxkbcommon=disabled", "-Dbpf-framework=disabled", "-Dlibidn2=disabled",
    "-Dpwquality=disabled", "-Dpasswdqc=disabled", "-Dremote=disabled", "-Dimds=disabled",
    "-Dhomed=disabled", "-Dnspawn=disabled", "-Dvmspawn=disabled", "-Dimportd=disabled",
    "-Drepart=disabled", "-Dsysupdate=disabled", "-Dfdisk=disabled", "-Dlibcrypt=disabled",
    "-Dblkid=disabled", "-Dnetworkd=false", "-Dresolve=false", "-Dtimesyncd=false",
    "-Dhwdb=false", "-Dfirstboot=false", "-Dportabled=false", "-Dmachined=false",
    "-Duserdb=false", "-Dcoredump=false", "-Doomd=false", "-Dsysext=false",
];

/// Unités à masquer : le login Senbit remplace getty, et busybox n'a pas agetty ;
/// systemd-logind exige un bus D-Bus système (absent) et rien ne l'utilise
/// (pas de PAM). Pour l'activer plus tard : installer dbus, puis supprimer les
/// liens /etc/systemd/system/systemd-logind*.
const MASKED_UNITS: &[&str] = &[
    "getty@.service",
    "serial-getty@.service",
    "console-getty.service",
    "systemd-logind.service",
    "systemd-logind-varlink.socket",
];

/// Outils de l'hôte copiés dans le rootfs avec leurs bibliothèques. busybox
/// ne sait créer que de l'ext2 sans journal (et mal) : l'installateur a besoin
/// des vrais mke2fs / e2fsck. (nom, obligatoire)
const HOST_TOOLS: &[(&str, bool)] = &[
    ("mke2fs", true),
    ("e2fsck", true),
    ("tune2fs", false),
    ("ldconfig.real", false),
    ("ldconfig", false),
];

fn read_trimmed(file: &Path, what: &str) -> Result<String> {
    if !file.is_file() {
        bail!("systemd {what} file not found:\n  {}", file.display());
    }
    let v: String = fs::read_to_string(file)?.chars().filter(|c| !c.is_whitespace()).collect();
    if v.is_empty() {
        bail!("systemd {what} is empty:\n  {}", file.display());
    }
    Ok(v)
}

/// Vérifie que external/systemd est sur le tag épinglé ET que son commit est
/// exactement celui de config/systemd/sha. Retourne (version, sha).
fn verify(p: &Paths) -> Result<(String, String)> {
    let dir = p.systemd_dir();
    if !gitutil::is_repo(&dir) {
        bail!("systemd source tree not found:\n  {}\n\nRun:\n  devtool get systemd", dir.display());
    }
    let version = read_trimmed(&p.systemd_version_file(), "version")?;
    let expected = read_trimmed(&p.systemd_sha_file(), "SHA")?.to_lowercase();
    let head = gitutil::head(&dir).to_lowercase();

    ui::info("systemd");
    ui::detail(format!("Requested: {version} ({expected})"));
    ui::detail(format!("Current:   {head}"));

    if head != expected {
        bail!(
            "systemd commit mismatch.\n  expected: {expected}\n  found:    {head}\n\nRun `devtool get systemd`; if {version} is intentionally new, update config/systemd/sha."
        );
    }
    if !gitutil::status_porcelain(&dir).is_empty() {
        ui::warn("external/systemd has local modifications.");
    }
    Ok((version, head))
}

/// `devtool get systemd` : clone / met à jour vers le tag épinglé, puis
/// vérifie le SHA.
pub(crate) fn fetch(p: &Paths) -> Result<()> {
    let dir = p.systemd_dir();
    let version = read_trimmed(&p.systemd_version_file(), "version")?;

    if !gitutil::is_repo(&dir) {
        if dir.exists() {
            bail!("{} exists but is not a Git repository.", dir.display());
        }
        ui::info(format!("Downloading systemd {version}..."));
        let spin = ui::spinner(&format!("Cloning systemd {version} (depth 1)..."));
        let result = gitutil::clone_tag(Paths::SYSTEMD_REPO, &version, &dir);
        spin.finish_and_clear();
        result?;
    } else if gitutil::describe_exact(&dir) != version {
        let status = gitutil::status_porcelain(&dir);
        if !status.is_empty() {
            bail!("systemd source tree contains local modifications:\n\n{status}");
        }
        ui::info(format!("Updating systemd to {version}..."));
        gitutil::fetch_tags(&dir)?;
        gitutil::checkout_detach(&dir, &version)?;
    }

    verify(p)?;
    ui::ok(format!("systemd {version} is ready."));
    Ok(())
}

fn fingerprint(sha: &str) -> String {
    let mut h = DefaultHasher::new();
    OPTIONS.hash(&mut h);
    format!("{sha}:opts={:016x}", h.finish())
}

fn meson_command() -> Command {
    let mut cmd = Command::new("/usr/bin/meson");

    cmd.env_clear()
        .env(
            "PATH",
            "/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin",
        )
        .env("CC", "/usr/bin/cc")
        .env("CXX", "/usr/bin/c++");

    cmd
}

pub fn build(p: &Paths, cache: &mut Cache, jobs: usize) -> Result<()> {
    let (_, sha) = verify(p)?;
    let dir = p.systemd_dir();
    let build_dir = p.systemd_build_dir();
    let install = p.systemd_install_dir();
    let binary = p.systemd_binary();
    let fp = fingerprint(&sha);

    if cache.is_fresh("systemd", &fp, &[&binary]) {
        println!();
        ui::skip("systemd already built for this version - skipping compilation.");
        ui::detail(format!("systemd: {}", binary.display()));
        return Ok(());
    }

    fs::create_dir_all(&build_dir)?;

    let mut setup = meson_command();
    
    setup
        .arg("setup")
        .env("CC", "cc")
        .env("CXX", "c++");
    
    if build_dir.join("build.ninja").is_file() {
        setup.arg("--reconfigure");
    }
    
    setup
        .arg(&build_dir)
        .arg(&dir)
        .arg("--prefix=/usr")
        .args(OPTIONS);
    
    ui::info("Configuring systemd...");
    run_quiet(&mut setup)?;

    let spin = ui::spinner("Compiling systemd...");
    let result = run_quiet(meson_command().args(["compile", "-C"]).arg(&build_dir).arg(format!("-j{jobs}")));
    spin.finish_and_clear();
    result?;

    let _ = fs::remove_dir_all(&install);
    run_quiet(
        meson_command()
            .args(["install", "--no-rebuild", "-C"])
            .arg(&build_dir)
            .arg("--destdir")
            .arg(&install),
    )?;

    if !binary.is_file() {
        bail!("systemd binary was not produced:\n  {}", binary.display());
    }

    println!();
    println!("systemd:");
    println!("  {}", binary.display());

    cache.record("systemd", &fp)?;
    Ok(())
}

fn is_elf(path: &Path) -> bool {
    use std::io::Read;
    let mut magic = [0u8; 4];
    fs::File::open(path).and_then(|mut f| f.read_exact(&mut magic)).is_ok() && magic == *b"\x7fELF"
}

fn collect_elfs(dir: &Path, out: &mut Vec<std::path::PathBuf>) -> Result<()> {
    for entry in fs::read_dir(dir)? {
        let path = entry?.path();
        let meta = fs::symlink_metadata(&path)?;
        if meta.is_dir() {
            collect_elfs(&path, out)?;
        } else if meta.is_file() && is_elf(&path) {
            out.push(path);
        }
    }
    Ok(())
}

/// Libraries systemd loads with dlopen(): `ldd` cannot see them, and without
/// them PID 1 fails ("Failed to mangle mount options ...: Operation not
/// supported" = libmount.so.1 missing). (soname, required)
const DLOPEN_LIBS: &[(&str, bool)] = &[
    ("libmount.so.1", true),
    ("libblkid.so.1", false),
    ("libcap.so.2", false),
];

/// Absolute path of a x86-64 library known to the host's ld.so cache.
fn host_library(soname: &str) -> Option<String> {
    let out = Command::new("ldconfig").arg("-p").output().ok()?;
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter(|l| l.trim_start().starts_with(soname))
        .filter(|l| l.contains("x86-64"))
        .filter_map(|l| l.split_once("=>").map(|(_, p)| p.trim().to_string()))
        .next()
}

/// Bibliothèques partagées (glibc incluse, chargeur ELF compris) requises par
/// les binaires systemd : le rootfs Senbit n'a pas de libc par défaut.
fn shared_libs(install: &Path) -> Result<BTreeSet<String>> {
    let mut elfs = Vec::new();
    collect_elfs(install, &mut elfs)?;

    let mut extra = BTreeSet::new();
    for (soname, required) in DLOPEN_LIBS {
        match host_library(soname) {
            Some(path) => {
                elfs.push(std::path::PathBuf::from(&path));
                extra.insert(path);
            }
            None if *required => bail!(
                "{soname} not found on the build host (systemd loads it with dlopen).\n\n\
                 Install it, e.g.:\n  sudo apt install libmount1 libmount-dev"
            ),
            None => ui::warn(format!("{soname} not found on the build host: skipped")),
        }
    }

    let lib_path = format!(
        "{}:{}",
        install.join("usr/lib/systemd").display(),
        install.join("usr/lib").display()
    );
    let mut libs = extra;

    for elf in elfs {
        let out = Command::new("ldd")
            .env("LD_LIBRARY_PATH", &lib_path)
            .arg(&elf)
            .output()
            .context("failed to run ldd")?;
        let text = String::from_utf8_lossy(&out.stdout);
        if text.contains("not a dynamic executable") {
            continue;
        }
        for line in text.lines() {
            if line.contains("not found") {
                bail!("missing library for {}: {}", elf.display(), line.trim());
            }
            let target = match line.split_once("=>") {
                Some((_, rhs)) => rhs,
                None => line,
            };
            if let Some(path) = target.split_whitespace().next().filter(|t| t.starts_with('/')) {
                if !Path::new(path).starts_with(install) {
                    libs.insert(path.to_string());
                }
            }
        }
    }
    Ok(libs)
}

fn force_symlink(target: &str, link: &Path) -> Result<()> {
    if let Some(parent) = link.parent() {
        fs::create_dir_all(parent)?;
    }
    if fs::symlink_metadata(link).is_ok() {
        fs::remove_file(link)?;
    }
    symlink(target, link).with_context(|| format!("failed to create symlink {}", link.display()))
}

/// Real (non-script) executable named `name` on the build host.
fn host_tool(name: &str) -> Option<std::path::PathBuf> {
    let dirs = ["/usr/sbin", "/sbin", "/usr/bin", "/bin", "/usr/local/sbin", "/usr/local/bin"];
    for dir in dirs {
        let path = Path::new(dir).join(name);
        let Ok(real) = fs::canonicalize(&path) else { continue };
        let Ok(bytes) = fs::read(&real).map(|mut b| { b.truncate(4); b }) else { continue };
        if bytes == b"\x7fELF" {
            return Some(real);
        }
    }
    None
}

fn copy_file_exec(from: &Path, dest: &Path) -> Result<()> {
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent)?;
    }
    if fs::symlink_metadata(dest).is_ok() {
        fs::remove_file(dest)?;
    }
    fs::copy(from, dest).with_context(|| format!("failed to install {}", from.display()))?;
    fs::set_permissions(dest, fs::Permissions::from_mode(0o755))?;
    Ok(())
}

/// Installs HOST_TOOLS (and the libraries they need) into the rootfs.
fn install_host_tools(rootfs: &Path) -> Result<()> {
    let mut libs = BTreeSet::new();

    for (name, required) in HOST_TOOLS {
        let Some(tool) = host_tool(name) else {
            if *required {
                bail!(
                    "{name} not found on the build host (needed by the Senbit installer).\n\n\
                     Install it, e.g.:\n  sudo apt install e2fsprogs"
                );
            }
            continue;
        };

        let target = if *name == "ldconfig.real" { "ldconfig" } else { name };
        copy_file_exec(&tool, &rootfs.join("sbin").join(target))?;

        // systemd only searches /usr/sbin:/usr/bin (+ /usr/local) for commands
        // written without a path (ExecStart=ldconfig -X): make the tool
        // reachable there too. Senbit keeps /sbin and /usr/sbin apart.
        force_symlink(&format!("/sbin/{target}"), &rootfs.join("usr/sbin").join(target))?;

        let out = Command::new("ldd").arg(&tool).output().context("failed to run ldd")?;
        for line in String::from_utf8_lossy(&out.stdout).lines() {
            if line.contains("not found") {
                bail!("missing library for {}: {}", tool.display(), line.trim());
            }
            let rhs = line.split_once("=>").map(|(_, r)| r).unwrap_or(line);
            if let Some(path) = rhs.split_whitespace().next().filter(|t| t.starts_with('/')) {
                libs.insert(path.to_string());
            }
        }
    }

    for lib in libs {
        copy_file_exec(Path::new(&lib), &rootfs.join(lib.trim_start_matches('/')))?;
    }

    // mkfs.ext4 & co: the real names of mke2fs / e2fsck.
    for fs_name in ["ext2", "ext3", "ext4"] {
        force_symlink("mke2fs", &rootfs.join("sbin").join(format!("mkfs.{fs_name}")))?;
        force_symlink("e2fsck", &rootfs.join("sbin").join(format!("fsck.{fs_name}")))?;
    }

    // Dynamic linker configuration: packages (apt) drop their library
    // directories into /etc/ld.so.conf.d/.
    fs::create_dir_all(rootfs.join("etc/ld.so.conf.d"))?;
    let ld_conf = rootfs.join("etc/ld.so.conf");
    if fs::symlink_metadata(&ld_conf).is_err() {
        fs::write(&ld_conf, "include /etc/ld.so.conf.d/*.conf\n")?;
    }

    // Defaults of mke2fs (-t ext4: extents, journal, ...).
    let conf = rootfs.join("etc/mke2fs.conf");
    if fs::symlink_metadata(&conf).is_err() {
        match fs::read("/etc/mke2fs.conf") {
            Ok(bytes) => fs::write(&conf, bytes)?,
            Err(_) => fs::write(
                &conf,
                "[defaults]\n\tbase_features = sparse_super,large_file,filetype,resize_inode,dir_index,ext_attr\n\tdefault_mntopts = acl,user_xattr\n\tblocksize = 4096\n\tinode_size = 256\n\tinode_ratio = 16384\n\n[fs_types]\n\text4 = {\n\t\tfeatures = has_journal,extent,huge_file,flex_bg,metadata_csum,64bit,dir_nlink,extra_isize\n\t}\n",
            )?,
        }
    }

    Ok(())
}

/// Copie systemd + bibliothèques dans le rootfs et active le service de
/// login Senbit (l'unité elle-même vient de l'overlay rootfs/).
pub fn install_into_rootfs(p: &Paths, rootfs: &Path) -> Result<()> {
    let install = p.systemd_install_dir();
    if !p.systemd_binary().is_file() {
        bail!("systemd has not been built: {}\n\nRun:\n  devtool build systemd", p.systemd_binary().display());
    }

    crate::build::rootfs::copy_tree(&install, rootfs)?;

    for lib in shared_libs(&install)? {
        let dest = rootfs.join(lib.trim_start_matches('/'));
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent)?;
        }
        if fs::symlink_metadata(&dest).is_ok() {
            fs::remove_file(&dest)?;
        }
        fs::copy(&lib, &dest).with_context(|| format!("failed to install library {lib}"))?;
        fs::set_permissions(&dest, fs::Permissions::from_mode(0o755))?;
    }

    install_host_tools(rootfs)?;

    // systemd's mount units run /usr/bin/mount and /usr/bin/umount (path fixed
    // at build time). Senbit's busybox provides them elsewhere: point the
    // expected paths at them. A real util-linux installed later by apt simply
    // replaces these links.
    for name in ["mount", "umount"] {
        let expected = rootfs.join("usr/bin").join(name);
        if fs::symlink_metadata(&expected).is_ok() {
            continue;
        }
        let found = ["bin", "sbin", "usr/sbin"]
            .iter()
            .find(|dir| fs::symlink_metadata(rootfs.join(dir).join(name)).is_ok());
        match found {
            Some(dir) => force_symlink(&format!("/{dir}/{name}"), &expected)?,
            None => bail!("{name} not found in the rootfs (busybox applet missing?): systemd's mount units need it"),
        }
    }

    let unit = "usr/lib/systemd/system/senbit-login.service";
    if !rootfs.join(unit).is_file() {
        bail!("senbit-login.service missing from rootfs overlay: rootfs/{unit}");
    }
    // An EMPTY unit file is how systemd represents a masked unit: the
    // service would silently never start.
    if fs::metadata(rootfs.join(unit))?.len() == 0 {
        bail!("senbit-login.service is empty: rootfs/{unit}");
    }

    let system = rootfs.join("etc/systemd/system");
    force_symlink("/usr/lib/systemd/system/multi-user.target", &system.join("default.target"))?;
    force_symlink(
        &format!("/{unit}"),
        &system.join("multi-user.target.wants/senbit-login.service"),
    )?;
    for name in MASKED_UNITS {
        force_symlink("/dev/null", &system.join(name))?;
    }

    // /etc/os-release -> /usr/lib/os-release (the file itself comes from the
    // rootfs overlay).
    force_symlink("../usr/lib/os-release", &rootfs.join("etc/os-release"))?;

    // Package compatibility (apt/dpkg): Debian-style packages install their
    // units in /lib/systemd/system. Senbit keeps /lib and /usr/lib apart, so
    // make that path land where systemd looks. Other places packages and
    // administrators use are plain directories created here.
    if fs::symlink_metadata(rootfs.join("lib/systemd")).is_err() {
        force_symlink("../usr/lib/systemd", &rootfs.join("lib/systemd"))?;
    }
    for dir in [
        "etc/systemd/system",
        "etc/systemd/system/multi-user.target.wants",
        "usr/lib/systemd/system",
        "usr/local/lib/systemd/system",
        "var/lib/systemd",
        "var/log",
    ] {
        fs::create_dir_all(rootfs.join(dir))?;
    }

    // Vérification finale : le chargeur ELF doit exister sinon PID 1 ne démarre pas.
    let out = capture(Command::new("file").arg(rootfs.join("usr/lib/systemd/systemd")))?;
    if !out.contains("ELF") {
        bail!("installed systemd is not an ELF binary: {out}");
    }
    Ok(())
}

// ----------------------------------------------------------------------
// Vérification du rootfs final
// ----------------------------------------------------------------------

fn exists_in(rootfs: &Path, path: &str) -> bool {
    // symlink_metadata: a symlink is valid even if its (absolute) target only
    // makes sense inside the final system.
    fs::symlink_metadata(rootfs.join(path.trim_start_matches('/'))).is_ok()
}

/// Binaries / directories systemd needs to boot as PID 1 and to make
/// `systemctl` usable, plus everything Senbit's own units point to.
const REQUIRED_PATHS: &[&str] = &[
    "usr/lib/systemd/systemd",
    "usr/lib/systemd/systemd-journald",
    "usr/bin/systemctl",
    "usr/lib/systemd/system/sysinit.target",
    "usr/lib/systemd/system/basic.target",
    "usr/lib/systemd/system/multi-user.target",
    "usr/lib/systemd/system/shutdown.target",
    "usr/lib/systemd/system/reboot.target",
    "usr/lib/systemd/system/poweroff.target",
    "usr/lib/systemd/system/halt.target",
    "usr/lib/systemd/system/senbit-login.service",
    "etc/systemd/system/default.target",
    "etc/systemd/system/multi-user.target.wants/senbit-login.service",
    "usr/lib/os-release",
    "etc/os-release",
    "lib/systemd",
    "sbin/senbit-init",
    "sbin/senbit-login",
    "bin/sh",
];

/// ELF interpreter ("/lib64/ld-linux-x86-64.so.2") of a binary, from `file`.
fn elf_interpreter(binary: &Path) -> Result<Option<String>> {
    let out = capture(Command::new("file").arg("-L").arg(binary))?;
    Ok(out
        .split("interpreter ")
        .nth(1)
        .and_then(|rest| rest.split(',').next())
        .map(|s| s.trim().to_string()))
}

/// Commands run by a unit: first word of every Exec*= line, without the
/// systemd prefixes ('-', '@', ':', '+', '!').
fn unit_executables(unit: &str) -> Vec<String> {
    unit.lines()
        .filter_map(|line| {
            let (key, value) = line.trim().split_once('=')?;
            if !key.starts_with("Exec") {
                return None;
            }
            let value = value.trim().trim_start_matches(['-', '@', ':', '+', '!']);
            let program = value.split_whitespace().next()?;
            program.starts_with('/').then(|| program.to_string())
        })
        .collect()
}

/// Checks the FINAL rootfs (called once everything is installed): the paths
/// systemd and Senbit's units rely on must exist in it, not just on the host.
pub fn verify_rootfs(rootfs: &Path) -> Result<()> {
    ui::info("Verifying the systemd integration in the rootfs...");

    let missing: Vec<&str> = REQUIRED_PATHS
        .iter()
        .copied()
        .filter(|path| !exists_in(rootfs, path))
        .collect();

    if !missing.is_empty() {
        bail!(
            "missing from the rootfs (systemd cannot boot Senbit without them):\n  {}",
            missing.join("\n  ")
        );
    }

    // The dynamic loader of systemd must exist at its absolute path, or the
    // kernel answers ENOENT to exec() and PID 1 never starts.
    if let Some(interpreter) = elf_interpreter(&rootfs.join("usr/lib/systemd/systemd"))? {
        if !exists_in(rootfs, &interpreter) {
            bail!("systemd's ELF interpreter is missing from the rootfs: {interpreter}");
        }
    }

    // Every absolute program used by a Senbit unit must exist and be
    // executable in the rootfs.
    let units = rootfs.join("usr/lib/systemd/system");
    for entry in fs::read_dir(&units)? {
        let path = entry?.path();
        let name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();

        if !name.starts_with("senbit-") || !name.ends_with(".service") {
            continue;
        }

        let text = fs::read_to_string(&path)?;
        if !text.contains("[Service]") {
            bail!("{name} has no [Service] section");
        }

        for program in unit_executables(&text) {
            let full = rootfs.join(program.trim_start_matches('/'));
            let meta = fs::symlink_metadata(&full)
                .with_context(|| format!("{name}: {program} does not exist in the rootfs"))?;

            if meta.is_file() && meta.permissions().mode() & 0o111 == 0 {
                bail!("{name}: {program} is not executable");
            }
        }
    }

    // senbit-login is a musl binary and must stay fully static: its shell
    // sessions and the installed system do not ship the glibc libraries it
    // would otherwise need.
    let login = capture(Command::new("file").arg(rootfs.join("sbin/senbit-login")))?;
    if !(login.contains("statically linked") || login.contains("static-pie")) {
        ui::warn(format!("senbit-login does not look static: {login}"));
    }

    ui::ok("systemd integration verified.");
    Ok(())
}
