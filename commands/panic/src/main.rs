//! panic - Crash the system on purpose (real kernel panic)
//!
//! Folder: commands/panic/  (detected and compiled automatically by devtool)
//! Installed as: /usr/sbin/panic
//!
//! Asks `senbit-login` (the system API, see docs/API.md) to crash the kernel for real.
//! It is a test tool: it proves that a crash is survived (the kernel command line has
//! `panic=10`, so the machine reboots by itself) and exercises the boot counter and the
//! automatic rollback. Unsaved data is lost.

use std::io::{self, BufRead, Write};
use std::process::ExitCode;

#[path = "../../_shared/api.rs"]
mod api;
#[path = "../../_shared/hook.rs"]
mod hook;

/// The word to type to confirm.
const CONFIRMATION: &str = "panic";

fn print_help() {
    println!("Usage: panic [OPTIONS]");
    println!();
    println!("Crash the system on purpose with a real kernel panic.");
    println!("The kernel reboots by itself a few seconds later (panic=10).");
    println!("Unsaved data is lost. Root only.");
    println!();
    println!("Options:");
    println!("  -y, --yes       Do not ask for a confirmation");
    println!("  -h, --help      Show this help");
    println!("  -V, --version   Show version");
}

/// True when the user types the confirmation word.
fn confirmed() -> bool {
    println!("This crashes the kernel on purpose. Unsaved data is lost.");
    print!("Type '{CONFIRMATION}' to continue: ");
    let _ = io::stdout().flush();

    let mut answer = String::new();
    if io::stdin().lock().read_line(&mut answer).is_err() {
        return false;
    }
    answer.trim() == CONFIRMATION
}

fn main() -> ExitCode {
    hook::install();

    let mut assume_yes = false;

    for argument in std::env::args().skip(1) {
        match argument.as_str() {
            "-y" | "--yes" => assume_yes = true,
            "-h" | "--help" => {
                print_help();
                return ExitCode::SUCCESS;
            }
            "-V" | "--version" => {
                println!("panic {}", env!("CARGO_PKG_VERSION"));
                return ExitCode::SUCCESS;
            }
            other => {
                eprintln!("panic: unknown option '{other}'");
                eprintln!("Try 'panic --help'.");
                return ExitCode::from(2);
            }
        }
    }

    if unsafe { libc::geteuid() } != 0 {
        eprintln!("panic: root is required, run: sudo panic");
        return ExitCode::FAILURE;
    }

    if !assume_yes && !confirmed() {
        println!("Nothing changed.");
        return ExitCode::SUCCESS;
    }

    match api::request("panic") {
        Ok(_) => {
            println!("panic: request accepted, the kernel is crashing now.");
            ExitCode::SUCCESS
        }
        Err(message) => {
            eprintln!("panic: {message}");
            ExitCode::FAILURE
        }
    }
}
