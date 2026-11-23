//! Repair mode of the ISO.
//!
//! The ISO has an entry "Senbit (repair an installed system)" that adds `senbit.repair=1` to
//! the kernel command line. Instead of starting the installed system (what the ISO does
//! otherwise when it finds one), senbit-init then:
//!
//!   1. finds the installed Senbit,
//!   2. checks its filesystem (automatically, and fully when asked),
//!   3. mounts it on /newroot with /proc, /sys, /dev and /run,
//!   4. gives a root shell INSIDE it (chroot): `snapshot restore`, `rollback`, apt...,
//!   5. unmounts and reboots when the shell ends.
//!
//! Nothing here changes the installed system by itself.

use std::io::{self, BufRead, Write};
use std::process::Command;

use colored::Colorize;

use crate::filesystem::check::{self, Outcome};
use crate::filesystem::{disk, fs as sysfs};
use crate::kernel::alim;
use crate::system::init::init::mount_system;
use crate::{log_error, log_warn};

const REPAIR_ROOT: &str = "/newroot";

/// True when the ISO was started in repair mode.
pub fn requested() -> bool {
    std::fs::read_to_string("/proc/cmdline")
        .map(|cmdline| {
            cmdline
                .split_whitespace()
                .any(|word| word == "senbit.repair=1" || word == "senbit.repair")
        })
        .unwrap_or(false)
}

fn read_line() -> String {
    let mut line = String::new();
    let _ = io::stdin().lock().read_line(&mut line);
    line.trim().to_string()
}

fn ask_yes_no(question: &str) -> bool {
    print!("{question} [y/N] ");
    let _ = io::stdout().flush();
    matches!(read_line().to_lowercase().as_str(), "y" | "yes" | "o" | "oui")
}

fn wait_for_enter(message: &str) {
    print!("{message}");
    let _ = io::stdout().flush();
    let _ = read_line();
}

fn show(outcome: &Outcome) {
    match outcome {
        Outcome::Clean => println!("{}", "Filesystem: clean.".green()),
        Outcome::Repaired => println!("{}", "Filesystem: errors were found and repaired.".yellow()),
        Outcome::NeedsAttention(why) => println!(
            "{}",
            format!("Filesystem: {why}. Run a full check, then try again.").red()
        ),
        Outcome::NotChecked(why) => println!("{}", format!("Filesystem: not checked ({why}).").yellow()),
    }
}

/// Runs the repair mode. Never returns: it ends with a reboot.
pub fn run() -> ! {
    println!();
    println!("{}", "Senbit repair mode".bold());
    println!("Nothing is changed until you do it yourself.");
    println!();

    let Some(partition) = disk::detect_installation() else {
        log_error!("{}", "No Senbit installation found on this machine.".red());
        wait_for_enter("Press Enter to reboot. ");
        alim::do_reboot();
    };

    println!("Senbit found on /dev/{partition}.");

    // Automatic check first (quick when the filesystem is clean), full check on request.
    show(&check::check_root(&partition, false));
    if ask_yes_no("Run a full filesystem check (e2fsck -f -y)? It can take a while.") {
        show(&check::check_root(&partition, true));
    }

    if let Err(error) = mount_system(&partition, REPAIR_ROOT) {
        log_error!("{}", format!("Cannot mount /dev/{partition}: {error:#}").red());
        wait_for_enter("Press Enter to reboot. ");
        alim::do_reboot();
    }

    println!();
    println!("The installed system is mounted on {REPAIR_ROOT} and you are inside it.");
    println!("Useful: snapshot list | snapshot restore NAME | rollback | apt");
    println!("Type 'exit' to leave: the system is unmounted and the machine reboots.");
    println!();

    let status = Command::new("chroot")
        .arg(REPAIR_ROOT)
        .arg("/bin/sh")
        .env("HOME", "/root")
        .env("USER", "root")
        .env("PS1", "(repair) # ")
        .status();

    if let Err(error) = status {
        log_warn!("{}", format!("Could not start the repair shell: {error}").yellow());
    }

    // Flush everything, then detach the mounts (the API filesystems below /newroot go with it).
    unsafe {
        libc::sync();
    }
    if let Err(error) = sysfs::lazy_unmount(REPAIR_ROOT) {
        log_warn!("{}", format!("Could not unmount {REPAIR_ROOT}: {error}").yellow());
    }
    unsafe {
        libc::sync();
    }

    println!("Rebooting...");
    alim::do_reboot();
}
