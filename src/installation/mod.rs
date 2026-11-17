// folders imports
pub mod bootloader;
pub mod local;
pub mod network;
pub mod users;

// files imports
pub mod hostname;
pub mod install;
pub mod partition;
pub mod plan;
pub mod rootfs;

/// Directory where the target disk is mounted during an installation.
///
/// A dedicated directory (not /mnt, which disk detection uses as a scratch
/// mount point): everything written to the new system lives under it, and it
/// is excluded when the live system is copied.
pub const INSTALL_ROOT: &str = "/target";
