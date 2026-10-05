// src/main.rs

// Cosmetics
use colored::Colorize;

// modules
mod utils;
mod kernel;
mod filesystem;
mod installation;
mod system;

// imports
use kernel::alim;
use utils::clear;
use filesystem::{fs, disk};
use installation::install;
use system::entry;

// clear() wrapper
fn clear() {
    if clear::clear_screen().is_err() {
        clear::clear_screen_ansi();
    }
}

fn yes_installation(sys_disk: &str) {
    log_info!("{}", "Starting system initialization...".cyan());

    // start_system() replaces this process with systemd (exec): it can only
    // come back with an error.
    match entry::start_system(sys_disk) {
        Err(e) => {
            kpanic!("Failed to start system: {:#}", e);
        }
        Ok(never) => match never {},
    }
}

fn no_installation() {
    log_info!("{}", "Starting system installation...".cyan());

    if let Err(e) = install::install_system() {
        kpanic!("Failed to install: {}", e);
    }

    log_info!("{}", "Installation completed successfully.".green());
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
    log_info!("{}", "Senbit init starting...".cyan());

    // mount filesystems
    log_info!("{}", "Mounting filesystems...".cyan());
    if let Err(e) = fs::mount_filesystems() {
        kpanic!(format!("Failed to mount filesystems: {}", e));
    }
    log_info!("{}", "Filesystems mounted successfully.".green());

    // Load the console font as early as possible so the installer uses it
    // too (the shell only loads it after login). /dev must be mounted first.
    system::init::console::load_font();

    // disk detection
    log_info!("{}", "Detecting disks...".cyan());
    
    let installed = disk::detect_installation();
    match installed {
        Some(p) => {
            log_info!("{}", format!("Senbit installed on {}", p).green());
            yes_installation(&p);
        }
        None => {
            log_error!("{}", "No Senbit installation found.".red());
            no_installation();
        }
    }
    
    alim::do_reboot();
}