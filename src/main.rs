// src/main.rs

use colored::Colorize;

// modules
mod utils;
mod kernel;
mod filesystem;

// imports
use utils::clear;
use kernel::panic;
use filesystem::{fs, disk};

// wrapper pour clear
fn clear() {
    if clear::clear_screen().is_err() {
        clear::clear_screen_ansi();
    }
}

fn main() {
    clear();

    // start different steps
    println!("{}", "Senbit init starting...".cyan());

    // mount filesystems
    println!("{}", "Mounting filesystems...".cyan());
    if let Err(e) = fs::mount_filesystems() {
        kpanic!(format!("Failed to mount filesystems: {}", e));
    }
    println!("{}", "Filesystems mounted successfully.".green());

    // disk detection
    println!("{}", "Detecting disks...".cyan());
    let installed = disk::detect_installation();
    match installed {
        Some(p) => println!("{}", format!("Senbit installed on {}", p).green()),
        None => println!("{}", "No Senbit installation found.".red()),
    }

    loop {}
}