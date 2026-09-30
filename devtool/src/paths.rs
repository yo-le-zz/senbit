//! Tous les chemins du projet, calculés depuis ROOT_DIR — reflet direct des
//! variables en tête de scripts/build.sh et scripts/run.sh.

use std::path::{Path, PathBuf};

pub struct Paths {
    pub root: PathBuf,
}

impl Paths {
    /// ROOT_DIR = dossier parent de celui où se trouve l'exécutable `devtool`,
    /// exactement comme `$(dirname "${BASH_SOURCE[0]}")/..` dans les scripts —
    /// en supposant que `devtool` vit dans `scripts/`, à la place des .sh.
    /// Peut être forcé avec `--root` ou `$SENBIT_ROOT`.
    pub fn resolve(explicit: Option<PathBuf>) -> anyhow::Result<Self> {
        if let Some(root) = explicit {
            return Ok(Self { root: root.canonicalize().unwrap_or(root) });
        }
        if let Ok(root) = std::env::var("SENBIT_ROOT") {
            let root = PathBuf::from(root);
            return Ok(Self { root: root.canonicalize().unwrap_or(root) });
        }
        if let Ok(exe) = std::env::current_exe() {
            if let Some(dir) = exe.parent() {
                if dir.file_name().and_then(|s| s.to_str()) == Some("scripts") {
                    if let Some(root) = dir.parent() {
                        return Ok(Self { root: root.to_path_buf() });
                    }
                }
            }
        }
        Ok(Self { root: std::env::current_dir()? })
    }

    pub fn r(&self, rel: &str) -> PathBuf {
        self.root.join(rel)
    }

    // ---- kernel ----
    pub fn kernel_dir(&self) -> PathBuf { self.r("kernel/linux") }
    pub fn kernel_build_dir(&self) -> PathBuf { self.r("build/kernel") }
    pub fn kernel_image(&self) -> PathBuf { self.kernel_build_dir().join("arch/x86/boot/bzImage") }
    pub fn kernel_version_file(&self) -> PathBuf { self.r("config/kernel/version") }
    pub const KERNEL_REPO: &'static str =
        "https://git.kernel.org/pub/scm/linux/kernel/git/torvalds/linux.git";

    // ---- busybox ----
    pub fn busybox_dir(&self) -> PathBuf { self.r("third_party/busybox") }
    pub fn busybox_build_dir(&self) -> PathBuf { self.r("build/busybox") }
    pub fn busybox_binary(&self) -> PathBuf { self.busybox_build_dir().join("_install/bin/busybox") }
    pub fn busybox_version_file(&self) -> PathBuf { self.r("config/busybox/version") }
    pub fn busybox_config(&self) -> PathBuf { self.r("config/busybox.config") }
    pub fn busybox_patch_dir(&self) -> PathBuf { self.r("third_party/patches/busybox") }
    pub const BUSYBOX_REPO: &'static str = "https://git.busybox.net/busybox";

    // ---- util-linux ----
    pub fn util_linux_dir(&self) -> PathBuf { self.r("third_party/util-linux") }
    pub fn util_linux_build_dir(&self) -> PathBuf { self.r("build/util-linux") }
    pub fn util_linux_version_file(&self) -> PathBuf { self.r("config/util-linux/version") }
    pub fn util_linux_fdisk(&self) -> PathBuf { self.util_linux_build_dir().join("fdisk.static") }
    pub fn util_linux_sfdisk(&self) -> PathBuf { self.util_linux_build_dir().join("sfdisk.static") }
    pub const UTIL_LINUX_REPO: &'static str = "https://github.com/util-linux/util-linux.git";

    // ---- parted ----
    pub fn parted_dir(&self) -> PathBuf { self.r("third_party/parted") }
    pub fn parted_build_dir(&self) -> PathBuf { self.r("build/parted") }
    pub fn parted_binary(&self) -> PathBuf { self.parted_build_dir().join("_install/usr/sbin/parted") }
    pub fn parted_version_file(&self) -> PathBuf { self.r("config/parted/version") }
    pub fn parted_patch_dir(&self) -> PathBuf { self.r("third_party/patches/parted") }

    // ---- rust userspace ----
    pub const RUST_TARGET: &'static str = "x86_64-unknown-linux-musl";
    /// Nom du binaire produit par le crate racine (renommé de senbit-init à
    /// senbit).
    pub fn senbit_binary(&self) -> PathBuf {
        self.r(&format!("target/{}/release/senbit", Self::RUST_TARGET))
    }

    // ---- tools ----
    pub fn tools_dir(&self) -> PathBuf { self.r("tools") }
    pub fn keymaps_generated_dir(&self) -> PathBuf { self.r("build/tools/generated/keymaps") }
    pub fn keymaps_rootfs_dir(&self, rootfs: &Path) -> PathBuf { rootfs.join("usr/share/keymaps") }

    // ---- rootfs / initramfs / iso ----
    pub fn rootfs_dir(&self) -> PathBuf { self.r("build/rootfs") }
    pub fn senbit_rootfs_overlay(&self) -> PathBuf { self.r("rootfs") }
    pub fn initramfs(&self) -> PathBuf { self.r("build/initramfs.cpio.gz") }
    pub fn iso_build_dir(&self) -> PathBuf { self.r("build/iso") }
    pub fn iso_image(&self) -> PathBuf { self.iso_build_dir().join("senbit.iso") }
    pub fn iso_dist(&self) -> PathBuf { self.r("iso/senbit.iso") }

    // ---- vm ----
    pub fn vm_config(&self) -> PathBuf { self.r("config/vm.config") }
    pub fn vm_dir(&self) -> PathBuf { self.r("build/vm") }

    // ---- cache de rebuild intelligent ----
    pub fn state_file(&self) -> PathBuf { self.r("build/.state/cache.json") }
}
