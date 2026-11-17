//! systemd — dépôt épinglé dans external/systemd (tag config/systemd/version,
//! commit attendu config/systemd/sha). Compilé avec meson, installé dans
//! build/systemd/_install puis copié dans le rootfs avec ses bibliothèques.

use std::collections::hash_map::DefaultHasher;
use std::collections::BTreeSet;
use std::fs;
use std::hash::{Hash, Hasher};
use std::os::unix::fs::{symlink, PermissionsExt};
use std::path::Path;
use std::process::Command;

use anyhow::{bail, Context, Result};

use crate::cache::Cache;
use crate::host;
use crate::paths::Paths;
use crate::proc::run_quiet;
use crate::{gitutil, ui};

const OPTIONS: &[&str] = &[
    "-Dmode=release",
    "-Dsplit-bin=true",
    "-Dlibdir=lib",
    "-Dsysconfdir=/etc",
    "-Dlocalstatedir=/var",
    "-Dman=disabled",
    "-Dhtml=disabled",
    "-Dtranslations=false",
    "-Dtests=false",
    "-Dinstall-tests=false",
    "-Dbootloader=disabled",
    "-Defi=false",
    "-Dukify=disabled",
    "-Dkernel-install=false",
    "-Danalyze=false",
    "-Dseccomp=disabled",
    "-Dselinux=disabled",
    "-Dapparmor=disabled",
    "-Dpolkit=disabled",
    "-Dacl=disabled",
    "-Daudit=disabled",
    "-Dpam=disabled",
    "-Dkmod=disabled",
    "-Dmicrohttpd=disabled",
    "-Dlibcryptsetup=disabled",
    "-Dlibcurl=disabled",
    "-Dopenssl=disabled",
    "-Dgcrypt=disabled",
    "-Dgnutls=disabled",
    "-Dp11kit=disabled",
    "-Dlibfido2=disabled",
    "-Dtpm2=disabled",
    "-Delfutils=disabled",
    "-Dzlib=disabled",
    "-Dbzip2=disabled",
    "-Dxz=disabled",
    "-Dlz4=disabled",
    "-Dzstd=disabled",
    "-Dpcre2=disabled",
    "-Dglib=disabled",
    "-Ddbus=disabled",
    "-Dlibarchive=disabled",
    "-Dqrencode=disabled",
    "-Dxkbcommon=disabled",
    "-Dbpf-framework=disabled",
    "-Dlibidn2=disabled",
    "-Dpwquality=disabled",
    "-Dpasswdqc=disabled",
    "-Dremote=disabled",
    "-Dimds=disabled",
    "-Dhomed=disabled",
    "-Dnspawn=disabled",
    "-Dvmspawn=disabled",
    "-Dimportd=disabled",
    "-Drepart=disabled",
    "-Dsysupdate=disabled",
    "-Dfdisk=disabled",
    "-Dlibcrypt=disabled",
    "-Dblkid=disabled",
    "-Dnetworkd=false",
    "-Dresolve=false",
    "-Dtimesyncd=false",
    "-Dhwdb=false",
    "-Dfirstboot=false",
    "-Dportabled=false",
    "-Dmachined=false",
    "-Duserdb=false",
    "-Dcoredump=false",
    "-Doomd=false",
    "-Dsysext=false",
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
    // The keyboard layout is loaded by senbit-init (BusyBox loadkmap, .bmap
    // files in /usr/share/keymaps) before systemd starts. systemd's own unit
    // needs a `loadkeys` that BusyBox does not provide: it failed at every boot.
    "systemd-vconsole-setup.service",
];

/// Units enabled at build time: (".wants" directory, link name, unit file).
/// A template instance (`senbit-dhcp@eth0`) links to the template file.
const ENABLED_UNITS: &[(&str, &str, &str)] = &[
    (
        "multi-user.target.wants",
        "senbit-dhcp@eth0.service",
        "senbit-dhcp@.service",
    ),
    // systemd's own ldconfig.service, made unconditional by the drop-in
    // ldconfig.service.d/10-senbit.conf: the library cache is rebuilt at every
    // boot and the service shows as "active (exited)". (It replaces Senbit's
    // senbit-ldconfig.service: both ran at once and one failed renaming
    // ld.so.cache~.)
    (
        "sysinit.target.wants",
        "ldconfig.service",
        "ldconfig.service",
    ),
    // Downloads the apt package lists as soon as the network is up (and retries
    // until it is): `apt install curl` fails with "Unable to locate package"
    // while /var/lib/apt/lists is empty. Never blocks the boot.
    (
        "multi-user.target.wants",
        "senbit-apt-update.service",
        "senbit-apt-update.service",
    ),
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
    let v: String = fs::read_to_string(file)?
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect();
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
        bail!(
            "systemd source tree not found:\n  {}\n\nRun:\n  devtool get systemd",
            dir.display()
        );
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

/// Names of the meson options the pinned systemd actually defines
/// (`meson.options`, or `meson_options.txt` before v257).
///
/// OPTIONS below was written for one systemd version; options are added and
/// removed between releases and meson fails on an unknown one. Filtering by
/// what the source declares makes changing the pinned version safe.
fn known_options(source: &Path) -> BTreeSet<String> {
    let text = ["meson.options", "meson_options.txt"]
        .iter()
        .find_map(|f| fs::read_to_string(source.join(f)).ok())
        .unwrap_or_default();

    let mut names = BTreeSet::new();
    for line in text.lines() {
        if let Some(rest) = line.trim_start().strip_prefix("option(") {
            let rest = rest.trim_start().trim_start_matches(['\'', '"']);
            if let Some(end) = rest.find(['\'', '"']) {
                names.insert(rest[..end].to_string());
            }
        }
    }
    names
}

/// Options built into meson itself: never declared in systemd's meson options.
const MESON_BUILTIN: &[&str] = &[
    "libdir",
    "sysconfdir",
    "localstatedir",
    "prefix",
    "bindir",
    "sbindir",
    "datadir",
];

/// (options to pass, options skipped because this systemd does not know them).
fn usable_options(source: &Path) -> (Vec<&'static str>, Vec<&'static str>) {
    let known = known_options(source);
    if known.is_empty() {
        // Could not read the declaration: pass everything, meson will complain.
        return (OPTIONS.to_vec(), Vec::new());
    }
    OPTIONS.iter().copied().partition(|opt| {
        let name = opt.trim_start_matches("-D").split('=').next().unwrap_or("");
        MESON_BUILTIN.contains(&name) || known.contains(name)
    })
}

fn fingerprint(sha: &str, opts: &[&str], patches: &str) -> String {
    let mut h = DefaultHasher::new();
    opts.hash(&mut h);
    format!("{sha}:opts={:016x}:patches={patches}", h.finish())
}

/// meson on a clean environment, tools found on the host (no /usr/bin/...).
fn meson_command() -> Result<Command> {
    let meson = host::find_tool("meson")
        .context("meson not found. Run `devtool doctor` to see what to install.")?;
    let cc = host::find_first(&["cc", "gcc"]).context("no C compiler found (cc/gcc)")?;
    let cxx = host::find_first(&["c++", "g++"]).unwrap_or_else(|| cc.clone());

    let mut cmd = Command::new(meson);
    cmd.env_clear()
        .env(
            "PATH",
            "/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin",
        )
        .env(
            "HOME",
            std::env::var("HOME").unwrap_or_else(|_| "/tmp".into()),
        )
        .env("LANG", "C.UTF-8")
        .env("CC", cc)
        .env("CXX", cxx);
    Ok(cmd)
}

pub fn build(p: &Paths, cache: &mut Cache, jobs: usize) -> Result<()> {
    let (_, sha) = verify(p)?;
    crate::patches::apply(p, "systemd", "systemd", &p.systemd_dir())?;
    let (options, skipped) = usable_options(&p.systemd_dir());
    if !skipped.is_empty() {
        ui::detail(format!(
            "meson options unknown to this systemd (skipped): {}",
            skipped.join(" ")
        ));
    }
    let dir = p.systemd_dir();
    let build_dir = p.systemd_build_dir();
    let install = p.systemd_install_dir();
    let binary = p.systemd_binary();
    let fp = fingerprint(&sha, &options, &crate::patches::fingerprint(p, "systemd"));

    if cache.is_fresh("systemd", &fp, &[&binary]) {
        println!();
        ui::skip("systemd already built for this version - skipping compilation.");
        ui::detail(format!("systemd: {}", binary.display()));
        return Ok(());
    }

    fs::create_dir_all(&build_dir)?;

    let mut setup = meson_command()?;

    setup.arg("setup");

    if build_dir.join("build.ninja").is_file() {
        setup.arg("--reconfigure");
    }

    setup
        .arg(&build_dir)
        .arg(&dir)
        .arg("--prefix=/usr")
        .args(&options);

    ui::info("Configuring systemd...");
    run_quiet(&mut setup)?;

    let spin = ui::spinner("Compiling systemd...");
    let result = run_quiet(
        meson_command()?
            .args(["compile", "-C"])
            .arg(&build_dir)
            .arg(format!("-j{jobs}")),
    );
    spin.finish_and_clear();
    result?;

    let _ = fs::remove_dir_all(&install);
    run_quiet(
        meson_command()?
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
    fs::File::open(path)
        .and_then(|mut f| f.read_exact(&mut magic))
        .is_ok()
        && magic == *b"\x7fELF"
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
pub(crate) fn host_library(soname: &str) -> Option<String> {
    let out = host::command("ldconfig").arg("-p").output().ok()?;
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
        let out = host::command("ldd")
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
            if let Some(path) = target
                .split_whitespace()
                .next()
                .filter(|t| t.starts_with('/'))
            {
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

/// Vrai si les deux chemins désignent le même dossier (liens suivis).
fn same_dir(a: &Path, b: &Path) -> bool {
    match (fs::canonicalize(a), fs::canonicalize(b)) {
        (Ok(x), Ok(y)) => x == y,
        _ => false,
    }
}

/// Real (non-script) executable named `name` on the build host.
fn host_tool(name: &str) -> Option<std::path::PathBuf> {
    let dirs = [
        "/usr/sbin",
        "/sbin",
        "/usr/bin",
        "/bin",
        "/usr/local/sbin",
        "/usr/local/bin",
    ];
    for dir in dirs {
        let path = Path::new(dir).join(name);
        let Ok(real) = fs::canonicalize(&path) else {
            continue;
        };
        let Ok(bytes) = fs::read(&real).map(|mut b| {
            b.truncate(4);
            b
        }) else {
            continue;
        };
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

        let target = if *name == "ldconfig.real" {
            "ldconfig"
        } else {
            name
        };
        copy_file_exec(&tool, &rootfs.join("sbin").join(target))?;

        // systemd only searches /usr/sbin:/usr/bin (+ /usr/local) for commands
        // written without a path (ExecStart=ldconfig -X): make the tool
        // reachable there too. Senbit keeps /sbin and /usr/sbin apart.
        //
        // With the merged-/usr layout /sbin IS /usr/sbin: the tool is already
        // there, and a link to itself would delete it.
        if !same_dir(&rootfs.join("sbin"), &rootfs.join("usr/sbin")) {
            force_symlink(
                &format!("/sbin/{target}"),
                &rootfs.join("usr/sbin").join(target),
            )?;
        }

        let out = host::command("ldd")
            .arg(&tool)
            .output()
            .context("failed to run ldd")?;
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

    if fs::symlink_metadata(rootfs.join("sbin/ldconfig")).is_err() {
        bail!(
            "ldconfig not found on the build host (glibc tools).\n\nRun `devtool doctor` to see what to install."
        );
    }

    // mkfs.ext4 & co: the real names of mke2fs / e2fsck.
    for fs_name in ["ext2", "ext3", "ext4"] {
        force_symlink(
            "mke2fs",
            &rootfs.join("sbin").join(format!("mkfs.{fs_name}")),
        )?;
        force_symlink(
            "e2fsck",
            &rootfs.join("sbin").join(format!("fsck.{fs_name}")),
        )?;
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
        bail!(
            "systemd has not been built: {}\n\nRun:\n  devtool build systemd",
            p.systemd_binary().display()
        );
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

    // systemd's mount units run /usr/sbin/mount and /usr/bin/mount (the path is
    // fixed when systemd is built and depends on the build host: the boot log
    // said "Failed at step EXEC spawning /usr/sbin/mount"). Merged-/usr only
    // merges /bin into /usr/bin and /sbin into /usr/sbin: /usr/bin and
    // /usr/sbin stay two directories. Senbit's busybox provides the applets in
    // one of them: make both paths work. A real util-linux installed later by
    // apt simply replaces these links.
    for name in ["mount", "umount"] {
        for expected_dir in ["usr/bin", "usr/sbin"] {
            let expected = rootfs.join(expected_dir).join(name);
            if fs::symlink_metadata(&expected).is_ok() {
                continue;
            }
            let found = ["bin", "sbin", "usr/bin", "usr/sbin"].iter().find(|dir| {
                fs::symlink_metadata(rootfs.join(dir).join(name)).is_ok()
            });
            match found {
                Some(dir) => force_symlink(&format!("/{dir}/{name}"), &expected)?,
                None => bail!("{name} not found in the rootfs (busybox applet missing?): systemd's mount units need it"),
            }
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
    force_symlink(
        "/usr/lib/systemd/system/multi-user.target",
        &system.join("default.target"),
    )?;
    force_symlink(
        &format!("/{unit}"),
        &system.join("multi-user.target.wants/senbit-login.service"),
    )?;
    for (wants, link, unit_file) in ENABLED_UNITS {
        let unit_path = format!("usr/lib/systemd/system/{unit_file}");
        if !rootfs.join(&unit_path).is_file() {
            bail!("unit missing from rootfs overlay: rootfs/{unit_path}");
        }
        force_symlink(&format!("/{unit_path}"), &system.join(wants).join(link))?;
    }
    for name in MASKED_UNITS {
        force_symlink("/dev/null", &system.join(name))?;
    }

    // Scripts of the overlay must be executable whatever the way the sources
    // reached the build host (a zip archive does not always keep the mode).
    for script in [
        "usr/lib/systemd/systemd-sysv-install",
        "usr/bin/ldd",
        "usr/bin/getconf",
    ] {
        let path = rootfs.join(script);
        if path.is_file() {
            fs::set_permissions(&path, fs::Permissions::from_mode(0o755))?;
        }
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
        "etc/systemd/system/sysinit.target.wants",
        "usr/lib/systemd/system",
        "usr/local/lib/systemd/system",
        "var/lib/systemd",
        "var/log",
    ] {
        fs::create_dir_all(rootfs.join(dir))?;
    }

    // Vérification finale : le chargeur ELF doit exister sinon PID 1 ne démarre pas.
    if !is_elf(&rootfs.join("usr/lib/systemd/systemd")) {
        bail!("installed systemd is not an ELF binary");
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
    "usr/lib/systemd/system/senbit-dhcp@.service",
    "usr/lib/systemd/system/ldconfig.service.d/10-senbit.conf",
    "usr/lib/systemd/system/ldconfig.service",
    "usr/lib/systemd/systemd-sysv-install",
    "usr/bin/ldd",
    "usr/bin/getconf",
    "usr/lib/sysusers.d/senbit.conf",
    "usr/lib/tmpfiles.d/senbit.conf",
    "etc/nsswitch.conf",
    "etc/shells",
    "sbin/ldconfig",
    "etc/systemd/system/default.target",
    "etc/systemd/system/multi-user.target.wants/senbit-login.service",
    "usr/lib/os-release",
    "etc/os-release",
    "lib/systemd",
    "sbin/senbit-init",
    "sbin/senbit-login",
    "bin/sh",
    // sysusers / link overrides of the overlay (accounts for apt, interface names)
    "usr/lib/systemd/system/systemd-sysusers.service.d/10-senbit.conf",
    "etc/systemd/network/00-senbit.link",
    "usr/lib/systemd/system/senbit-apt-update.service",
    "etc/profile.d/senbit-bootreport.sh",
    // service management (`systemctl`, `journalctl`)
    "usr/bin/journalctl",
    // mount units
    "usr/bin/mount",
    "usr/bin/umount",
    "usr/sbin/mount",
    "usr/sbin/umount",
    // apt / dpkg and what maintainer scripts call
    "usr/bin/apt-get",
    "usr/bin/apt",
    "usr/bin/dpkg",
    "usr/bin/dpkg-deb",
    "usr/bin/update-alternatives",
    "usr/lib/apt/methods/https",
    "usr/lib/apt/methods/http",
    "usr/bin/perl",
    "usr/bin/deb-systemd-helper",
    "usr/bin/deb-systemd-invoke",
    "usr/share/keyrings/debian-archive-keyring.gpg",
    "etc/ssl/certs/ca-certificates.crt",
    "etc/apt/sources.list.d/senbit.sources",
    "var/lib/dpkg/status",
    // Senbit commands (commands/)
    "usr/bin/sudo",
    "usr/bin/bootreport",
    "usr/bin/getent",
    "usr/sbin/adduser",
    "usr/sbin/useradd",
    "usr/sbin/reboot",
    "usr/sbin/poweroff",
];

/// Merged-/usr links: (link, target). The rootfs must have them, or Debian
/// packages install their files in the wrong place and systemd reports
/// "unmerged-usr".
const MERGED_USR_LINKS: &[(&str, &str)] = &[
    ("bin", "usr/bin"),
    ("sbin", "usr/sbin"),
    ("lib", "usr/lib"),
    ("lib64", "usr/lib64"),
];

/// ELF interpreter ("/lib64/ld-linux-x86-64.so.2") of a 64-bit binary, read
/// from its PT_INTERP program header (no dependency on the `file` command).
/// `None` for a static binary. Symlinks are followed.
pub(crate) fn elf_interpreter(binary: &Path) -> Result<Option<String>> {
    let data = fs::read(binary).with_context(|| format!("cannot read {}", binary.display()))?;
    parse_elf_interpreter(&data)
}

fn parse_elf_interpreter(data: &[u8]) -> Result<Option<String>> {
    let u16_at = |o: usize| {
        data.get(o..o + 2)
            .map(|b| u16::from_le_bytes([b[0], b[1]]) as usize)
    };
    let u32_at = |o: usize| {
        data.get(o..o + 4)
            .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    };
    let u64_at = |o: usize| {
        data.get(o..o + 8)
            .map(|b| u64::from_le_bytes(b.try_into().expect("8 bytes")) as usize)
    };

    if data.get(..4) != Some(b"\x7fELF") || data.get(4) != Some(&2) || data.get(5) != Some(&1) {
        bail!("not a little-endian ELF64 binary");
    }
    let (Some(phoff), Some(phentsize), Some(phnum)) = (u64_at(0x20), u16_at(0x36), u16_at(0x38))
    else {
        bail!("truncated ELF header");
    };

    for i in 0..phnum {
        let ph = phoff + i * phentsize;
        if u32_at(ph) == Some(3) {
            // PT_INTERP
            let (Some(offset), Some(size)) = (u64_at(ph + 8), u64_at(ph + 32)) else {
                bail!("truncated ELF program header");
            };
            let raw = data
                .get(offset..offset + size)
                .context("PT_INTERP out of range")?;
            let end = raw.iter().position(|b| *b == 0).unwrap_or(raw.len());
            return Ok(Some(String::from_utf8_lossy(&raw[..end]).to_string()));
        }
    }
    Ok(None)
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

    for (link, target) in MERGED_USR_LINKS {
        match fs::read_link(rootfs.join(link)) {
            Ok(t) if t == Path::new(target) => {}
            Ok(t) => bail!(
                "/{link} points to {} instead of {target}: the merged-/usr layout is broken",
                t.display()
            ),
            Err(_) => bail!(
                "/{link} is not a link to {target}: the merged-/usr layout is broken \
                 (systemd would report \"unmerged-usr\")"
            ),
        }
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
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();

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
    if let Some(interpreter) = elf_interpreter(&rootfs.join("sbin/senbit-login"))? {
        ui::warn(format!(
            "senbit-login is dynamically linked (interpreter {interpreter})"
        ));
    }

    ui::ok("systemd integration verified.");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_interpreter_of_a_real_binary() {
        // /bin/sh is a dynamic ELF on every glibc distribution.
        let sh = host::find_tool("sh").expect("sh");
        let real = fs::canonicalize(sh).unwrap();
        if is_elf(&real) {
            let interp = elf_interpreter(&real).unwrap();
            assert!(interp.map(|i| i.contains("ld")).unwrap_or(true));
        }
    }

    #[test]
    fn meson_builtin_options_are_never_filtered() {
        let dir = std::env::temp_dir().join("senbit-opts-test");
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            dir.join("meson_options.txt"),
            "option('mode', type : 'combo')\n",
        )
        .unwrap();
        let (used, skipped) = usable_options(&dir);
        assert!(used.contains(&"-Dmode=release"));
        assert!(used.contains(&"-Dlibdir=lib"));
        assert!(skipped.contains(&"-Dman=disabled"));
    }

    #[test]
    fn rejects_non_elf() {
        assert!(parse_elf_interpreter(b"#!/bin/sh\n").is_err());
    }
}
