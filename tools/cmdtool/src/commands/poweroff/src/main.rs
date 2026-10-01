use std::env;
use std::io;

fn print_help() {
    println!("Usage: poweroff [OPTIONS]");
    println!();
    println!("Power off the system.");
    println!();
    println!("Options:");
    println!("  -f, --force       Force poweroff");
    println!("  -w, --wtmp-only   Do not power off, only write wtmp");
    println!("  -h, --help        Show this help");
    println!("  -V, --version     Show version");
}

fn print_version() {
    println!("poweroff 0.1.0");
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
                // Direct syscall below.
            }

            "-w" | "--wtmp-only" => {
                eprintln!("poweroff: --wtmp-only is not supported yet");
                std::process::exit(1);
            }

            _ => {
                eprintln!(
                    "poweroff: unrecognized option '{}'",
                    arg
                );
                eprintln!(
                    "Try 'poweroff --help' for more information."
                );
                std::process::exit(1);
            }
        }
    }

    let result = unsafe {
        libc::reboot(libc::RB_POWER_OFF)
    };

    if result != 0 {
        eprintln!(
            "poweroff: {}",
            io::Error::last_os_error()
        );
        std::process::exit(1);
    }
}