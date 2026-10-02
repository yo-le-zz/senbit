use std::env;
use std::io::{
    Read,
    Write,
};
use std::os::unix::net::UnixStream;

const SOCKET_PATH: &str =
    "/run/senbit.sock";

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
    println!(
        "{} {}", 
        env!("CARGO_PKG_NAME"), 
        env!("CARGO_PKG_VERSION")
    );
}

fn send_event(
    event: &str,
) -> Result<(), String> {
    let mut stream =
        UnixStream::connect(
            SOCKET_PATH,
        )
        .map_err(|e| {
            format!(
                "failed to connect to {}: {}",
                SOCKET_PATH,
                e
            )
        })?;

    stream
        .write_all(event.as_bytes())
        .map_err(|e| {
            format!(
                "failed to send event: {}",
                e
            )
        })?;

    let mut response =
        String::new();

    stream
        .read_to_string(&mut response)
        .map_err(|e| {
            format!(
                "failed to read response: {}",
                e
            )
        })?;

    if response.trim() != "ok" {
        return Err(
            format!(
                "system handler returned: {}",
                response.trim()
            )
        );
    }

    Ok(())
}

fn main() {
    let args: Vec<String> =
        env::args()
            .skip(1)
            .collect();

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

            "-f" | "--force" => {
                let result = unsafe {
                    libc::reboot(
                        libc::RB_AUTOBOOT,
                    )
                };
            
                if result != 0 {
                    eprintln!(
                        "reboot: {}",
                        std::io::Error::last_os_error()
                    );
                    std::process::exit(1);
                }
            
                return;
            }

            "-w" | "--wtmp-only" => {
                eprintln!(
                    "reboot: --wtmp-only is not supported yet"
                );
                std::process::exit(1);
            }

            _ => {
                eprintln!(
                    "reboot: unrecognized option '{}'",
                    arg
                );
                eprintln!(
                    "Try 'reboot --help' for more information."
                );
                std::process::exit(1);
            }
        }
    }

    if let Err(error) =
        send_event("reboot")
    {
        eprintln!(
            "reboot: {}",
            error
        );

        std::process::exit(1);
    }
}