//! All project paths, calculated from ROOT_DIR — a direct reflection of the
//! variables defined at the top of scripts/build.sh and scripts/run.sh.

use std::path::{Path, PathBuf};

use crate::config::Config;

pub struct Paths {
    pub root: PathBuf,
    pub cfg: Config,
}

/// Un dossier est la racine du projet s'il contient `config.toml` et `devtool/`.
fn is_project_root(dir: &Path) -> bool {
    dir.join("config.toml").is_file() && dir.join("devtool").join("Cargo.toml").is_file()
}

/// Remonte depuis `start` jusqu'à trouver la racine du projet.
fn find_root_upwards(start: &Path) -> Option<PathBuf> {
    start
        .ancestors()
        .find(|d| is_project_root(d))
        .map(Path::to_path_buf)
}

impl Paths {
    /// Racine du projet, par ordre de priorité :
    /// 1. `--root` ; 2. `$SENBIT_ROOT` ;
    /// 3. en remontant depuis l'emplacement de l'exécutable `devtool`
    ///    (liens symboliques résolus : `scripts/devtool` ou `devtool/target/...`) ;
    /// 4. en remontant depuis le dossier courant.
    ///
    /// Aucun chemin absolu : le dépôt marche cloné n'importe où.
    pub fn resolve(explicit: Option<PathBuf>) -> anyhow::Result<Self> {
        let from_dir = |root: PathBuf| -> anyhow::Result<Self> {
            let root = root.canonicalize().unwrap_or(root);
            let cfg = Config::load(&root)?;
            Ok(Self { root, cfg })
        };

        if let Some(root) = explicit {
            return from_dir(root);
        }
        if let Ok(root) = std::env::var("SENBIT_ROOT") {
            return from_dir(PathBuf::from(root));
        }
        if let Ok(exe) = std::env::current_exe() {
            let exe = exe.canonicalize().unwrap_or(exe);
            if let Some(root) = exe.parent().and_then(find_root_upwards) {
                return from_dir(root);
            }
        }
        if let Some(root) = find_root_upwards(&std::env::current_dir()?) {
            return from_dir(root);
        }
        anyhow::bail!(
            "Senbit project root not found (no config.toml next to a devtool/ directory).\n\
             Run devtool from inside the project, or pass --root <dir>, or set SENBIT_ROOT."
        )
    }

    pub fn r(&self, rel: &str) -> PathBuf {
        self.root.join(rel)
    }

    // ---- kernel ----
    pub fn kernel_dir(&self) -> PathBuf {
        self.r(&self.cfg.kernel.directory)
    }
    pub fn kernel_build_dir(&self) -> PathBuf {
        self.r("build/kernel")
    }
    pub fn kernel_image(&self) -> PathBuf {
        self.kernel_build_dir().join("arch/x86/boot/bzImage")
    }
    pub fn kernel_version_file(&self) -> PathBuf {
        self.r("config/kernel/version")
    }
    pub fn kernel_repo(&self) -> &str {
        &self.cfg.kernel.repository
    }
    pub fn kernel_mirror(&self) -> &str {
        &self.cfg.kernel.mirror
    }
    pub fn kernel_sha_file(&self) -> PathBuf {
        self.r("config/kernel/sha")
    }

    // ---- busybox ----
    pub fn busybox_dir(&self) -> PathBuf {
        self.r(&self.cfg.busybox.directory)
    }
    pub fn busybox_build_dir(&self) -> PathBuf {
        self.r("build/busybox")
    }
    pub fn busybox_binary(&self) -> PathBuf {
        self.busybox_build_dir().join("_install/bin/busybox")
    }
    pub fn busybox_version_file(&self) -> PathBuf {
        self.r("config/busybox/version")
    }
    pub fn busybox_config(&self) -> PathBuf {
        self.r("config/busybox.config")
    }
    pub fn busybox_repo(&self) -> &str {
        &self.cfg.busybox.repository
    }

    // ---- util-linux ----
    pub fn util_linux_dir(&self) -> PathBuf {
        self.r(&self.cfg.util_linux.directory)
    }

    pub fn util_linux_build_dir(&self) -> PathBuf {
        self.r("build/util-linux")
    }

    pub fn util_linux_version_file(&self) -> PathBuf {
        self.r("config/util-linux/version")
    }

    pub fn util_linux_fdisk(&self) -> PathBuf {
        self.util_linux_build_dir().join("fdisk.static")
    }

    pub fn util_linux_sfdisk(&self) -> PathBuf {
        self.util_linux_build_dir().join("sfdisk.static")
    }

    pub fn util_linux_repo(&self) -> &str {
        &self.cfg.util_linux.repository
    }
    pub fn util_linux_sha_file(&self) -> PathBuf {
        self.r("config/util-linux/sha")
    }

    // ---- parted ----
    pub fn parted_dir(&self) -> PathBuf {
        self.r(&self.cfg.parted.directory)
    }

    pub fn parted_build_dir(&self) -> PathBuf {
        self.r("build/parted")
    }

    pub fn parted_binary(&self) -> PathBuf {
        self.parted_build_dir().join("_install/usr/sbin/parted")
    }

    pub fn parted_version_file(&self) -> PathBuf {
        self.r("config/parted/version")
    }

    pub fn parted_sha256_file(&self) -> PathBuf {
        self.r("config/parted/sha256")
    }

    /// Patches Senbit d'un composant : `patches/<composant>/*.patch`
    /// (suivis par git, contrairement à `third_party/` qui est téléchargé).
    pub fn patch_dir(&self, component: &str) -> PathBuf {
        self.r(&format!("patches/{component}"))
    }

    /// Ancien emplacement (`third_party/patches/<composant>`), encore lu.
    pub fn legacy_patch_dir(&self, component: &str) -> PathBuf {
        self.r(&format!("third_party/patches/{component}"))
    }

    // ---- systemd ----
    pub fn systemd_dir(&self) -> PathBuf {
        self.r(&self.cfg.systemd.directory)
    }
    pub fn systemd_build_dir(&self) -> PathBuf {
        self.r("build/systemd")
    }
    pub fn systemd_install_dir(&self) -> PathBuf {
        self.systemd_build_dir().join("_install")
    }
    pub fn systemd_binary(&self) -> PathBuf {
        self.systemd_install_dir().join("usr/lib/systemd/systemd")
    }
    pub fn systemd_version_file(&self) -> PathBuf {
        self.r("config/systemd/version")
    }
    pub fn systemd_sha_file(&self) -> PathBuf {
        self.r("config/systemd/sha")
    }
    pub fn systemd_repo(&self) -> &str {
        &self.cfg.systemd.repository
    }

    // ---- apt / dpkg (paquets Debian amorcés dans le rootfs) ----
    pub fn apt_cache_dir(&self) -> PathBuf {
        self.r("build/apt/cache")
    }

    // ---- rust userspace ----
    pub fn rust_target(&self) -> &str {
        &self.cfg.rust.target
    }

    /// Name of the binary produced by the root crate
    /// (renamed from senbit-init to senbit).
    pub fn senbit_binary(&self) -> PathBuf {
        self.r(&format!("target/{}/release/senbit", self.rust_target()))
    }

    // ---- senbit-login (independent crate, run by systemd) ----
    pub fn login_dir(&self) -> PathBuf {
        self.r("components/senbit-login")
    }

    pub fn login_target_dir(&self) -> PathBuf {
        self.r("build/login/target")
    }

    pub fn login_binary(&self) -> PathBuf {
        self.login_target_dir()
            .join(format!("{}/release/senbit-login", self.rust_target()))
    }

    // ---- commands natives (commands/<nom>/, une crate par commande) ----
    pub fn commands_dir(&self) -> PathBuf {
        self.r(&self.cfg.commands.directory)
    }

    pub fn commands_build_dir(&self) -> PathBuf {
        self.r("build/commands")
    }

    /// Dossier `target/` partagé par toutes les commandes (dépendances
    /// compilées une seule fois).
    pub fn commands_target_dir(&self) -> PathBuf {
        self.commands_build_dir().join("target")
    }

    /// Arborescence prête à être fusionnée dans le rootfs : bin/, sbin/,
    /// usr/share/senbit/commands.list.
    pub fn commands_stage_dir(&self) -> PathBuf {
        self.commands_build_dir().join("stage")
    }

    // ---- tools ----
    pub fn tools_dir(&self) -> PathBuf {
        self.r("tools")
    }

    // ---- fonts ----

    pub fn console_fonts_dir(&self) -> PathBuf {
        self.r("build/tools/generated/fonts")
    }

    // ---- grub ----
    /// GRUB files copied into the rootfs (used by the installer).
    pub fn grub_rootfs_dir(&self) -> PathBuf {
        self.r("build/tools/generated/grub/stage")
    }

    // ---- rootfs / initramfs / iso ----
    pub fn rootfs_dir(&self) -> PathBuf {
        self.r("build/rootfs")
    }

    pub fn senbit_rootfs_overlay(&self) -> PathBuf {
        self.root.join("rootfs")
    }

    pub fn initramfs(&self) -> PathBuf {
        self.r("build/initramfs.cpio.gz")
    }

    pub fn iso_build_dir(&self) -> PathBuf {
        self.r("build/iso")
    }

    pub fn iso_image(&self) -> PathBuf {
        self.iso_build_dir().join("senbit.iso")
    }

    pub fn iso_dist(&self) -> PathBuf {
        self.r("iso/senbit.iso")
    }

    // ---- vm ----
    pub fn vm_config(&self) -> PathBuf {
        self.r("config/vm.config")
    }

    pub fn vm_dir(&self) -> PathBuf {
        self.r("build/vm")
    }

    // ---- smart rebuild cache ----
    pub fn state_file(&self) -> PathBuf {
        self.r("build/.state/cache.json")
    }
}
