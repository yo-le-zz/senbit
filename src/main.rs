// src/main.rs

use colored::Colorize;

// modules
mod utils;
mod kernel;
mod filesystem;

// imports
use utils::clear;
use kernel::alim;
use filesystem::{fs, disk, install};

// wrapper pour clear
fn clear() {
    if clear::clear_screen().is_err() {
        clear::clear_screen_ansi();
    }
}

fn yes_installation() {
    
}

fn no_installation() {
    println!("{}", "Starting system installation...".cyan());

    if let Err(e) = install::install_system() {
        kpanic!("Failed to install: {}", e);
    }

    println!("{}", "Installation completed successfully.".green());
}

fn main() {
    // PID 1 is responsible for setting the PATH environment variable
    unsafe {
        std::env::set_var(
            "PATH",
            "/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin",
        );
    }
    
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
        Some(p) => {
            println!("{}", format!("Senbit installed on {}", p).green());
            yes_installation();
        }
        None => {
            println!("{}", "No Senbit installation found.".red());
            no_installation();
        }
    }

    loop {
        
    }
    
    alim::do_power_off();
}