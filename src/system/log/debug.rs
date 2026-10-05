//! Debug mode: mirrors every Senbit log line into the kernel log (/dev/kmsg).
//!
//! A background process copies the kernel log to the serial port, so with
//! `devtool run --debug` the whole boot (kernel, senbit-init, systemd, journal,
//! senbit-login) ends up in `build/vm/qemu.log` and nothing more is printed on
//! the screen.
//!
//! Debug mode is on when either:
//!   - the kernel command line contains `senbit.debug`, or
//!   - QEMU was started with `-fw_cfg name=opt/senbit/debug,string=1`
//!     (`devtool run --debug` does it; needs CONFIG_FW_CFG_SYSFS).

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::Path;
use std::os::unix::process::CommandExt;
use std::process::{Command, Stdio};
use std::sync::Once;

const FW_CFG_FLAG: &str = "/sys/firmware/qemu_fw_cfg/by_name/opt/senbit/debug/raw";

pub fn enabled() -> bool {
    fs::read_to_string("/proc/cmdline")
        .map(|c| c.split_whitespace().any(|a| a == "senbit.debug"))
        .unwrap_or(false)
        || Path::new(FW_CFG_FLAG).exists()
}

/// Starts (once) a background `cat /dev/kmsg > /dev/ttyS0`: the whole kernel
/// log (kernel, senbit-init, systemd, journal, senbit-login) is copied to the
/// serial port, i.e. into `build/vm/qemu.log`, WITHOUT raising the console
/// loglevel: nothing extra is printed on the screen, the interactive
/// installer and login menus stay clean.
///
/// The helper is a separate process, so it survives the exec() of systemd; it
/// keeps its open files across the switch_root.
fn start_serial_forwarder() {
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        if !Path::new("/dev/kmsg").exists() || !Path::new("/dev/ttyS0").exists() {
            return;
        }
        let _ = Command::new("sh")
            .args(["-c", "exec cat /dev/kmsg > /dev/ttyS0 2>/dev/null"])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .process_group(0)
            .spawn();
    });
}

fn strip_ansi(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\x1b' {
            if chars.peek() == Some(&'[') {
                chars.next();
                for n in chars.by_ref() {
                    if n.is_ascii_alphabetic() {
                        break;
                    }
                }
            }
            continue;
        }
        out.push(c);
    }
    out
}

/// Writes `text` (one or more lines) to /dev/kmsg. Never fails loudly.
pub fn mirror(text: &str) {
    if !enabled() {
        return;
    }
    start_serial_forwarder();

    let Ok(mut kmsg) = OpenOptions::new().write(true).open("/dev/kmsg") else {
        return;
    };

    for line in strip_ansi(text).lines() {
        let line = line.trim_end();
        if line.is_empty() {
            continue;
        }
        // One write() per record; the kernel truncates at ~1 KiB.
        let mut record = format!("<6>senbit: {line}\n");
        if record.len() > 900 {
            record.truncate(899);
            record.push('\n');
        }
        let _ = kmsg.write_all(record.as_bytes());
    }
}
