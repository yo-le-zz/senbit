// filesystem/check.rs
//
// Filesystem check of the root partition, run BEFORE it is mounted.
//
// After a power cut or a crash the ext4 journal is replayed by the kernel at mount time, but
// real inconsistencies (a block used twice, a damaged directory...) need `e2fsck`. The check
// is "preen" mode (`-p`): when the filesystem was shut down cleanly it ends in a fraction of
// a second, otherwise it repairs what is safe to repair automatically.

use std::io::ErrorKind;
use std::process::Command;

use crate::{log_info, log_warn};

/// What the check found.
pub enum Outcome {
    /// Nothing to repair.
    Clean,
    /// Errors were found and repaired.
    Repaired,
    /// e2fsck could not repair everything: a full check by hand is needed.
    NeedsAttention(String),
    /// The check did not run (e2fsck missing, not executable...).
    NotChecked(String),
}

fn device_path(sys_disk: &str) -> String {
    if sys_disk.starts_with("/dev/") {
        sys_disk.to_string()
    } else {
        format!("/dev/{sys_disk}")
    }
}

/// Checks `sys_disk` (`vda1` or `/dev/vda1`), which must NOT be mounted.
///
/// `full` runs a complete check that answers "yes" to every repair (`-f -y`); otherwise the
/// check is the automatic one done at every boot (`-p`).
pub fn check_root(sys_disk: &str, full: bool) -> Outcome {
    let device = device_path(sys_disk);

    let mut command = Command::new("e2fsck");
    if full {
        command.args(["-f", "-y"]);
    } else {
        command.arg("-p");
    }
    command.arg(&device);

    log_info!("Checking {device} (e2fsck{})...", if full { " -f -y" } else { " -p" });

    let output = match command.output() {
        Ok(output) => output,
        Err(error) if error.kind() == ErrorKind::NotFound => {
            return Outcome::NotChecked("e2fsck not found".to_string());
        }
        Err(error) => return Outcome::NotChecked(format!("cannot run e2fsck: {error}")),
    };

    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    for line in text.lines().filter(|line| !line.trim().is_empty()) {
        log_info!("  e2fsck: {line}");
    }

    // Exit status: 0 clean, 1 errors corrected, 2 corrected and a reboot is advised (only
    // for a mounted filesystem), 4 errors left, 8 operational error, 12 and more: worse.
    match output.status.code() {
        Some(0) => Outcome::Clean,
        Some(1) | Some(2) => {
            log_warn!("e2fsck repaired errors on {device}");
            Outcome::Repaired
        }
        Some(code) => Outcome::NeedsAttention(format!("e2fsck exited with status {code}")),
        None => Outcome::NeedsAttention("e2fsck was stopped by a signal".to_string()),
    }
}
