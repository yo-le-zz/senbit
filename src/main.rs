// src/main.rs

// Cosmetics
use colored::Colorize;

// modules
mod utils;
mod kernel;
mod filesystem;
mod installation;

// imports
// use kernel::alim;
use utils::clear;
use filesystem::{fs, disk};
use installation::install;

// wrapper pour clear
fn clear() {
    if clear::clear_screen().is_err() {
        clear::clear_screen_ansi();
    }
}

fn yes_installation() {
    
}

fn no_installation() {
    logln!("{}", "Starting system installation...".cyan());

    if let Err(e) = install::install_system() {
        kpanic!("Failed to install: {}", e);
    }

    logln!("{}", "Installation completed successfully.".green());
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
    logln!("{}", "Senbit init starting...".cyan());

    // mount filesystems
    logln!("{}", "Mounting filesystems...".cyan());
    if let Err(e) = fs::mount_filesystems() {
        kpanic!(format!("Failed to mount filesystems: {}", e));
    }
    logln!("{}", "Filesystems mounted successfully.".green());

    // disk detection
    logln!("{}", "Detecting disks...".cyan());
    
    let installed = disk::detect_installation();
    match installed {
        Some(p) => {
            logln!("{}", format!("Senbit installed on {}", p).green());
            yes_installation();
        }
        None => {
            logln!("{}", "No Senbit installation found.".red());
            no_installation();
        }
    }

    loop {
        
    }
    
    // alim::do_power_off();
}