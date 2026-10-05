//! All project paths, calculated from ROOT_DIR — a direct reflection of the
//! variables defined at the top of scripts/build.sh and scripts/run.sh.

use std::path::{Path, PathBuf};

pub struct Paths {
    pub root: PathBuf,
}

impl Paths {
    /// ROOT_DIR = parent directory of the directory containing the `devtool`
    /// executable, exactly like `$(dirname "${BASH_SOURCE[0]}")/..` in the
    /// scripts, assuming `devtool` lives in `scripts/` instead of the .sh files.
    /// Can be overridden with `--root` or `$SENBIT_ROOT`.
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
    pub fn busybox_binary(&self) -> PathBuf {
        self.busybox_build_dir().join("_install/bin/busybox")
    }
    pub fn busybox_version_file(&self) -> PathBuf {
        self.r("config/busybox/version")
    }
    pub fn busybox_config(&self) -> PathBuf {
        self.r("config/busybox.config")
    }
    pub fn busybox_patch_dir(&self) -> PathBuf {
        self.r("third_party/patches/busybox")
    }
    pub const BUSYBOX_REPO: &'static str = "https://git.busybox.net/busybox";

    // ---- util-linux ----
    pub fn util_linux_dir(&self) -> PathBuf {
        self.r("third_party/util-linux")
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

    pub const UTIL_LINUX_REPO: &'static str =
        "https://github.com/util-linux/util-linux.git";

    // ---- parted ----
    pub fn parted_dir(&self) -> PathBuf {
        self.r("third_party/parted")
    }

    pub fn parted_build_dir(&self) -> PathBuf {
        self.r("build/parted")
    }

    pub fn parted_binary(&self) -> PathBuf {
        self.parted_build_dir()
            .join("_install/usr/sbin/parted")
    }

    pub fn parted_version_file(&self) -> PathBuf {
        self.r("config/parted/version")
    }

    pub fn parted_patch_dir(&self) -> PathBuf {
        self.r("third_party/patches/parted")
    }

    // ---- systemd ----
    pub fn systemd_dir(&self) -> PathBuf { self.r("external/systemd") }
    pub fn systemd_build_dir(&self) -> PathBuf { self.r("build/systemd") }
    pub fn systemd_install_dir(&self) -> PathBuf { self.systemd_build_dir().join("_install") }
    pub fn systemd_binary(&self) -> PathBuf {
        self.systemd_install_dir().join("usr/lib/systemd/systemd")
    }
    pub fn systemd_version_file(&self) -> PathBuf { self.r("config/systemd/version") }
    pub fn systemd_sha_file(&self) -> PathBuf { self.r("config/systemd/sha") }
    pub const SYSTEMD_REPO: &'static str = "https://github.com/systemd/systemd.git";

    // ---- rust userspace ----
    pub const RUST_TARGET: &'static str =
        "x86_64-unknown-linux-musl";

    /// Name of the binary produced by the root crate
    /// (renamed from senbit-init to senbit).
    pub fn senbit_binary(&self) -> PathBuf {
        self.r(&format!(
            "target/{}/release/senbit",
            Self::RUST_TARGET
        ))
    }

    // ---- senbit-login (independent crate, run by systemd) ----
    pub fn login_dir(&self) -> PathBuf {
        self.r("components/senbit-login")
    }

    pub fn login_target_dir(&self) -> PathBuf {
        self.r("build/login/target")
    }

    pub fn login_binary(&self) -> PathBuf {
        self.login_target_dir().join(format!(
            "{}/release/senbit-login",
            Self::RUST_TARGET
        ))
    }

    // ---- tools ----
    pub fn tools_dir(&self) -> PathBuf {
        self.r("tools")
    }

    pub fn keymaps_generated_dir(&self) -> PathBuf {
        self.r("build/tools/generated/keymaps")
    }

    pub fn keymaps_rootfs_dir(
        &self,
        rootfs: &Path,
    ) -> PathBuf {
        rootfs.join("usr/share/keymaps")
    }

    // ---- fonts ----

    pub fn console_fonts_dir(&self) -> PathBuf {
        self.r("build/tools/generated/fonts")
    }
    
    pub fn fonts_cache_dir(&self) -> PathBuf {
        self.r("build/tools/cache/fonts")
    }

    // ---- grub ----
    /// GRUB release built for the live system (grub-install + platform files).
    pub const GRUB_VERSION: &'static str = "2.12";

    pub fn grub_build_root(&self) -> PathBuf {
        self.r("build/grub")
    }

    pub fn grub_cache_dir(&self) -> PathBuf {
        self.grub_build_root().join("cache")
    }

    pub fn grub_src_dir(&self) -> PathBuf {
        self.grub_build_root().join("src")
    }

    pub fn grub_efi_build_dir(&self) -> PathBuf {
        self.grub_build_root().join("build-efi")
    }

    pub fn grub_pc_build_dir(&self) -> PathBuf {
        self.grub_build_root().join("build-pc")
    }

    pub fn grub_efi_install_dir(&self) -> PathBuf {
        self.grub_build_root().join("install-efi")
    }

    pub fn grub_pc_install_dir(&self) -> PathBuf {
        self.grub_build_root().join("install-pc")
    }

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