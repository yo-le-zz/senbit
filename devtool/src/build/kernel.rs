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
    // Docker: without overlayfs dockerd falls back to devicemapper, which fails
    // ("devicemapper: Error running deviceCreate (CreatePool) dm_task_run failed").
    "OVERLAY_FS",
    // Docker: iptables (nft backend of Debian) must be able to create the `nat` table.
    // NF_TABLES_IPV4 / NF_TABLES_IPV6 are bool options WITHOUT default: without them
    // dockerd stops with "iptables ... TABLE_ADD failed (Operation not supported):
    // table nat" (kernel 6.18: net/ipv4/netfilter/Kconfig).
    "NF_TABLES",
    "NF_TABLES_IPV4",
    "NF_TABLES_IPV6",
    "NFT_NAT",
    "NFT_COMPAT",
    "NF_NAT",
    "VETH",
    "BRIDGE",
];

/// Options utiles mais dont l'absence ne doit pas bloquer la compilation
/// (le nom change parfois d'une version du noyau à l'autre) : activées si
/// possible, signalées sinon.
///
/// Elles complètent le README de systemd (« Requirements ») : cgroups v2
/// complets, pseudo-terminaux (SSH, `script`), espaces de noms, seccomp,
/// API crypto utilisée par systemd, BPF pour les cgroups.
const RECOMMENDED_CONFIG: &[&str] = &[
    "PROC_FS",
    "SYSFS",
    "UNIX",
    "NET",
    "DEVPTS_FS",
    "UNIX98_PTYS",
    "CGROUP_SCHED",
    "CGROUP_CPUACCT",
    "CGROUP_DEVICE",
    "CGROUP_FREEZER",
    "CGROUP_BPF",
    "BPF_SYSCALL",
    "MEMCG",
    "BLK_CGROUP",
    "CPUSETS",
    "PID_NS",
    "UTS_NS",
    "IPC_NS",
    "USER_NS",
    "NET_NS",
    "SECCOMP",
    "SECCOMP_FILTER",
    "BINFMT_MISC",
    "CRYPTO_USER_API_HASH",
    "CRYPTO_HMAC",
    "CRYPTO_SHA256",
    "DMIID",
    "EXT4_FS",
    "EXT4_FS_POSIX_ACL",
    "EXT4_FS_SECURITY",
    "PSI",
    "MAGIC_SYSRQ",
    // Containers (Docker, Podman, LXC): see Docker's check-config.sh. Built in
    // (=y): the rootfs ships no kernel modules. A name that does not exist in
    // this kernel version is only reported, it does not stop the build.
    // /proc/config.gz: `zcat /proc/config.gz | grep OVERLAY` tells what the running kernel has.
    "IKCONFIG",
    "IKCONFIG_PROC",
    "POSIX_MQUEUE",
    "KEYS",
    "CGROUP_PERF",
    "CGROUP_HUGETLB",
    "CFS_BANDWIDTH",
    "FAIR_GROUP_SCHED",
    "BLK_DEV_THROTTLING",
    "BRIDGE_NETFILTER",
    "BRIDGE_VLAN_FILTERING",
    "MACVLAN",
    "IPVLAN",
    "VXLAN",
    "TUN",
    // NOT enabled on purpose: CONFIG_DUMMY=y creates a "dummy0" interface at every
    // boot, which looked like a network card to the installer and to the init.
    "INET",
    "IPV6",
    "NETFILTER",
    "NETFILTER_ADVANCED",
    "NETFILTER_XTABLES",
    "NF_CONNTRACK",
    "NF_TABLES_INET",
    "NF_TABLES_ARP",
    "NF_TABLES_NETDEV",
    "NFT_MASQ",
    "NFT_CT",
    "NFT_CHAIN_NAT",
    "NETFILTER_XT_NAT",
    "NETFILTER_XT_MARK",
    "NETFILTER_XT_MATCH_ADDRTYPE",
    "NETFILTER_XT_MATCH_CONNTRACK",
    "NETFILTER_XT_MATCH_COMMENT",
    "NETFILTER_XT_MATCH_MULTIPORT",
    "NFT_COUNTER",
    "NFT_REJECT",
    "NFT_REDIR",
    "NETFILTER_XT_MATCH_MARK",
    "NETFILTER_XT_TARGET_MASQUERADE",
    "NETFILTER_XT_TARGET_REDIRECT",
    "NETFILTER_XT_TARGET_CHECKSUM",
    // Legacy iptables (iptables-legacy), a fallback if iptables-nft is not wanted: since
    // Linux 6.18 IP_NF_FILTER / IP_NF_NAT depend on the *_LEGACY options.
    "NETFILTER_XTABLES_LEGACY",
    "IP_NF_IPTABLES_LEGACY",
    "IP6_NF_IPTABLES_LEGACY",
    "IP_NF_IPTABLES",
    "IP_NF_FILTER",
    "IP_NF_MANGLE",
    "IP_NF_NAT",
    "IP_NF_TARGET_MASQUERADE",
    "IP6_NF_IPTABLES",
    "IP6_NF_FILTER",
    "IP6_NF_MANGLE",
    "IP6_NF_NAT",
    "IP6_NF_TARGET_MASQUERADE",
    "INET_DIAG",
    "NET_CLS_CGROUP",
    "CGROUP_NET_PRIO",
];

/// Options forcées à « non ». `dummy0` : avec CONFIG_DUMMY=y le noyau crée une
/// interface réseau factice à chaque démarrage, que l'installateur et l'init
/// prenaient pour une carte réseau. (Il faut aussi la désactiver explicitement :
/// un .config créé par un build précédent la contient déjà.)
const DISABLED_CONFIG: &[&str] = &["DUMMY"];

fn apply_required_config(p: &Paths) -> Result<()> {
    let build_dir = p.kernel_build_dir();
    let config = build_dir.join(".config");
    let script = p.kernel_dir().join("scripts/config");

    let mut cmd = Command::new(&script);
    cmd.arg("--file").arg(&config);
    for opt in REQUIRED_CONFIG.iter().chain(RECOMMENDED_CONFIG) {
        cmd.arg("-e").arg(opt);
    }
    for opt in DISABLED_CONFIG {
        cmd.arg("-d").arg(opt);
    }
    run_quiet(&mut cmd)?;

    let o = format!("O={}", build_dir.display());
    run_quiet(
        Command::new("make")
            .arg("-C")
            .arg(p.kernel_dir())
            .arg(&o)
            .arg("olddefconfig"),
    )?;

    // olddefconfig supprime silencieusement une option dont une dépendance
    // manque : on vérifie qu'elles sont toutes réellement à "=y".
    let text = std::fs::read_to_string(&config)?;
    let missing: Vec<&str> = REQUIRED_CONFIG
        .iter()
        .copied()
        .filter(|opt| !text.lines().any(|l| l == format!("CONFIG_{opt}=y")))
        .collect();

    let skipped: Vec<&str> = RECOMMENDED_CONFIG
        .iter()
        .copied()
        .filter(|opt| !text.lines().any(|l| l == format!("CONFIG_{opt}=y")))
        .collect();
    if !skipped.is_empty() {
        ui::warn(format!(
            "optional kernel options not enabled (not in this kernel?): {}",
            skipped.join(" ")
        ));
    }

    if !missing.is_empty() {
        bail!(
            "kernel options could not be enabled (missing dependency?):\n  {}",
            missing.join("\n  ")
        );
    }

    let still_on: Vec<&str> = DISABLED_CONFIG
        .iter()
        .copied()
        .filter(|opt| text.lines().any(|l| l == format!("CONFIG_{opt}=y")))
        .collect();
    if !still_on.is_empty() {
        bail!(
            "kernel options that must be off are still enabled (another option selects them?):\n  {}",
            still_on.join("\n  ")
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

    let pinned = crate::get::read_pinned(&p.kernel_version_file(), "kernel version")?;
    if gitutil::describe_exact(&dir) != pinned {
        bail!("Linux is not on the pinned version {pinned}.\n\nRun:\n  devtool get linux");
    }
    crate::patches::apply(p, "linux", "Linux", &dir)?;
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
    let fp = format!(
        "{}|cfg:{}|patches:{}",
        git_fingerprint(&dir),
        REQUIRED_CONFIG.join(",") + &RECOMMENDED_CONFIG.join(",") + "|off:" + &DISABLED_CONFIG.join(","),
        crate::patches::fingerprint(p, "linux")
    );

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
        run_quiet(
            Command::new("make")
                .arg("-C")
                .arg(&dir)
                .arg(&o)
                .arg("x86_64_defconfig"),
        )?;
    }

    println!();
    ui::info("Updating kernel configuration (framebuffer console for UEFI)...");
    apply_required_config(p)?;

    println!();
    ui::info("Building Linux kernel...");
    ui::detail(format!("Jobs: {jobs}"));
    ui::detail("Incremental build enabled.");

    let spin = ui::spinner("Compiling Linux kernel (this can take a while)...");
    let result = run_quiet(
        Command::new("make")
            .arg("-C")
            .arg(&dir)
            .arg(&o)
            .arg(format!("-j{jobs}")),
    );
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
