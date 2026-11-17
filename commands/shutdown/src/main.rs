use std::env;
use std::io::{
    Read,
    Write,
};
use std::os::unix::net::UnixStream;
use std::thread;
use std::time::Duration;

#[derive(Clone, Copy)]
enum Action {
    Halt,
    Poweroff,
    Reboot,
}

const SOCKET_PATH: &str =
    "/run/senbit.sock";

fn print_help() {
    println!("Usage: shutdown [OPTIONS] TIME [MESSAGE]");
    println!();
    println!("Schedule a system shutdown.");
    println!();
    println!("TIME:");
    println!("  now               Shutdown immediately");
    println!("  +N                Shutdown after N minutes");
    println!();
    println!("Options:");
    println!("  -H                Halt the system");
    println!("  -P                Power off the system");
    println!("  -h                Power off the system");
    println!("  -r                Reboot the system");
    println!("  -c                Cancel a scheduled shutdown");
    println!("  -f, --force       Force the operation");
    println!("  -V, --version     Show version");
    println!("      --help        Show this help");
}

fn print_version() {
    println!(
        "{} {}",
        env!("CARGO_PKG_NAME"),
        env!("CARGO_PKG_VERSION")
    );
}

fn fail(message: &str) -> ! {
    eprintln!(
        "shutdown: {}",
        message
    );

    std::process::exit(1);
}

fn send_event(
    action: Action,
) -> Result<(), String> {
    let event =
        match action {
            Action::Halt => "halt",
            Action::Poweroff => "poweroff",
            Action::Reboot => "reboot",
        };

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
        .read_to_string(
            &mut response,
        )
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

fn parse_delay(
    time: &str,
) -> Result<u64, String> {
    if time == "now" {
        return Ok(0);
    }

    if let Some(minutes) =
        time.strip_prefix('+')
    {
        if minutes.is_empty() {
            return Err(
                "invalid time specification"
                    .to_string()
            );
        }

        let minutes: u64 =
            minutes
                .parse()
                .map_err(|_| {
                    "invalid time specification"
                        .to_string()
                })?;

        return Ok(minutes);
    }

    Err(
        "only 'now' and '+N' time specifications are supported"
            .to_string()
    )
}

fn main() {
    let args: Vec<String> =
        env::args()
            .skip(1)
            .collect();

    if args.is_empty() {
        print_help();
        return;
    }

    let mut action =
        Action::Poweroff;

    let mut time:
        Option<String> = None;

    let mut force =
        false;

    let mut index =
        0;

    while index < args.len() {
        let arg =
            &args[index];

        match arg.as_str() {
            "-H" => {
                action =
                    Action::Halt;
            }

            "-P" | "-h" => {
                action =
                    Action::Poweroff;
            }

            "-r" => {
                action =
                    Action::Reboot;
            }

            "-f" | "--force" => {
                force =
                    true;
            }

            "-V" | "--version" => {
                print_version();
                return;
            }

            "--help" => {
                print_help();
                return;
            }

            "-c" => {
                println!(
                    "shutdown: no scheduled shutdown is active."
                );
                return;
            }

            value
                if value.starts_with('-')
                    && value != "-" =>
            {
                fail(&format!(
                    "unrecognized option '{}'",
                    value
                ));
            }

            value => {
                if time.is_none() {
                    time = Some(
                        value.to_string()
                    );
                } else {
                    // Remaining arguments form
                    // the optional message.
                    break;
                }
            }
        }

        index += 1;
    }

    let time =
        match time {
            Some(time) => time,

            None => {
                fail(
                    "time is required. Try 'shutdown --help'."
                );
            }
        };

    let delay_minutes =
        match parse_delay(
            &time,
        ) {
            Ok(delay) => delay,

            Err(error) => {
                fail(&error);
            }
        };

    if force {
        let result = unsafe {
            match action {
                Action::Halt => {
                    libc::reboot(
                        libc::RB_HALT_SYSTEM,
                    )
                }
    
                Action::Poweroff => {
                    libc::reboot(
                        libc::RB_POWER_OFF,
                    )
                }
    
                Action::Reboot => {
                    libc::reboot(
                        libc::RB_AUTOBOOT,
                    )
                }
            }
        };
    
        if result != 0 {
            eprintln!(
                "shutdown: {}",
                std::io::Error::last_os_error()
            );
            std::process::exit(1);
        }
    
        return;
    }

    if delay_minutes > 0 {
        println!(
            "Shutdown scheduled in {} minute(s).",
            delay_minutes
        );

        thread::sleep(
            Duration::from_secs(
                delay_minutes * 60
            )
        );
    } else {
        println!(
            "System shutdown initiated."
        );
    }

    if let Err(error) =
        send_event(action)
    {
        eprintln!(
            "shutdown: {}",
            error
        );

        std::process::exit(1);
    }
}