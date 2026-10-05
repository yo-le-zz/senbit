//! Minimal logging.
//!
//! Log lines go to the kernel log (/dev/kmsg): journald collects them (the
//! journal shows them as `senbit-login`), `devtool run --debug` captures them
//! on the serial console, and they never end up in the middle of the
//! interactive login screen, which is drawn on stderr (= the console).

use std::fmt::Arguments;
use std::fs::OpenOptions;
use std::io::Write;

pub fn write(level: &str, args: Arguments<'_>) {
    let priority = match level {
        "ERROR" => 3,
        "WARN" => 4,
        _ => 6,
    };

    let mut line = format!("<{priority}>senbit-login: [{level}] {args}");
    line.truncate(900);
    line.push('\n');

    let written = OpenOptions::new()
        .write(true)
        .open("/dev/kmsg")
        .and_then(|mut kmsg| kmsg.write_all(line.as_bytes()))
        .is_ok();

    // Last resort (no /dev/kmsg): stderr.
    if !written {
        eprint!("{}", &line[line.find('>').map_or(0, |i| i + 1)..]);
    }
}

macro_rules! log_info {
    ($($arg:tt)*) => { $crate::log::write("INFO", format_args!($($arg)*)) };
}

macro_rules! log_warn {
    ($($arg:tt)*) => { $crate::log::write("WARN", format_args!($($arg)*)) };
}

macro_rules! log_error {
    ($($arg:tt)*) => { $crate::log::write("ERROR", format_args!($($arg)*)) };
}
