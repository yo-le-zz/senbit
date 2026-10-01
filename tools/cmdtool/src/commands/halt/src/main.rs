use std::env;
use std::io;

fn print_help() {
    println!("Usage: halt [OPTIONS]");
    println!();
    println!("Halt the system.");
    println!();
    println!("Options:");
    println!("  -f, --force       Force halt");
    println!("  -w, --wtmp-only   Do not halt, only write wtmp");
    println!("  -h, --help        Show this help");
    println!("  -V, --version     Show version");
}

fn print_version() {
    println!("halt 0.1.0");
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
                eprintln!("halt: --wtmp-only is not supported yet");
                std::process::exit(1);
            }

            _ => {
                eprintln!(
                    "halt: unrecognized option '{}'",
                    arg
                );
                eprintln!(
                    "Try 'halt --help' for more information."
                );
                std::process::exit(1);
            }
        }
    }

    let result = unsafe {
        libc::reboot(libc::RB_HALT_SYSTEM)
    };

    if result != 0 {
        eprintln!(
            "halt: {}",
            io::Error::last_os_error()
        );
        std::process::exit(1);
    }
}