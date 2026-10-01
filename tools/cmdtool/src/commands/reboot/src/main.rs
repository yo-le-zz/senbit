use std::env;
use std::io;

fn print_help() {
    println!("Usage: reboot [OPTIONS]");
    println!();
    println!("Reboot the system.");
    println!();
    println!("Options:");
    println!("  -f, --force       Force reboot");
    println!("  -w, --wtmp-only   Do not reboot, only write wtmp");
    println!("  -h, --help        Show this help");
    println!("  -V, --version     Show version");
}

fn print_version() {
    println!("reboot 0.1.0");
}

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();

    for arg in &args {
        match arg.as_str() {
            "-h" | "--help" => {
                print_help();
                return;
            }

            "-V" | "--version" => {
                print_version();
                return;
            }

            "-f" => {
                // The syscall itself is already the low-level operation.
            }

            "-w" | "--wtmp-only" => {
                eprintln!("reboot: --wtmp-only is not supported yet");
                std::process::exit(1);
            }

            _ => {
                eprintln!("reboot: unrecognized option '{}'", arg);
                eprintln!("Try 'reboot --help' for more information.");
                std::process::exit(1);
            }
        }
    }

    let result = unsafe {
        libc::reboot(libc::RB_AUTOBOOT)
    };

    if result != 0 {
        eprintln!(
            "reboot: {}",
            io::Error::last_os_error()
        );
        std::process::exit(1);
    }
}