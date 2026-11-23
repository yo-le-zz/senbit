//! Panic hook shared by the native commands.
//!
//! Without it, a bug in a command prints Rust's raw message (and, with
//! `panic = "abort"`, ends with SIGABRT). With it, the user gets one clear line,
//! where to report the bug, and a stable exit status (70, "internal software error").
//!
//! Include it from a command with:
//!
//! ```ignore
//! #[path = "../../_shared/hook.rs"]
//! mod hook;
//!
//! fn main() {
//!     hook::install();
//!     // ...
//! }
//! ```
#![allow(dead_code)]

use std::io::Write;
use std::panic;
use std::process;

/// Exit status of an internal error (EX_SOFTWARE in sysexits.h).
const EXIT_INTERNAL_ERROR: i32 = 70;

/// Where to report a bug.
const ISSUES_URL: &str = "https://github.com/yo-le-zz/senbit/issues";

/// Name the command was started with (`addgroup` for an alias of `adduser`).
fn command_name() -> String {
    std::env::args()
        .next()
        .and_then(|path| path.rsplit('/').next().map(str::to_string))
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| "senbit".to_string())
}

/// Replaces the default panic message. Call it first thing in `main`.
pub fn install() {
    let name = command_name();

    panic::set_hook(Box::new(move |info| {
        let place = info
            .location()
            .map(|location| format!("{}:{}", location.file(), location.line()))
            .unwrap_or_else(|| "unknown place".to_string());

        let payload = info.payload();
        let message = if let Some(text) = payload.downcast_ref::<&str>() {
            (*text).to_string()
        } else if let Some(text) = payload.downcast_ref::<String>() {
            text.clone()
        } else {
            "no message".to_string()
        };

        // `writeln!` and not `eprintln!`: a closed stderr must not cause a second panic.
        let mut stderr = std::io::stderr();
        let _ = writeln!(stderr, "{name}: internal error: {message} ({place})");
        let _ = writeln!(stderr, "{name}: this is a bug, please report it: {ISSUES_URL}");

        process::exit(EXIT_INTERNAL_ERROR);
    }));
}
