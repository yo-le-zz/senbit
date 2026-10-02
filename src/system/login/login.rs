use anyhow::{
    Context,
    Result,
};

use colored::Colorize;

use inquire::Select;

use std::fs;
use std::io::{
    self,
    Write,
};
use std::os::fd::AsRawFd;
use std::os::unix::net::UnixListener;
use std::path::Path;
use std::thread;
use std::time::Duration;

use crate::log_error;

use crate::system::handler::socket::{
    start_event_listener,
};

use crate::system::shell::shell::shell_start;
use crate::system::shell::start::splash;

use crate::utils::clear::clear_screen_ansi;
use crate::utils::crypto::hash::verify_shadow_password;

/* ============================================================
   User information
============================================================ */

fn get_user_ids(
    username: &str,
    root: &str,
) -> Result<(u32, u32)> {
    let path =
        Path::new(root)
            .join("etc/passwd");

    let content =
        fs::read_to_string(&path)
            .with_context(|| {
                format!(
                    "failed to read {}",
                    path.display()
                )
            })?;

    for line in content.lines() {
        let fields: Vec<&str> =
            line.split(':')
                .collect();

        if fields.len() >= 4
            && fields[0] == username
        {
            let uid =
                fields[2]
                    .parse::<u32>()
                    .with_context(|| {
                        format!(
                            "invalid UID for '{}'",
                            username
                        )
                    })?;

            let gid =
                fields[3]
                    .parse::<u32>()
                    .with_context(|| {
                        format!(
                            "invalid GID for '{}'",
                            username
                        )
                    })?;

            return Ok((
                uid,
                gid,
            ));
        }
    }

    Err(
        anyhow::anyhow!(
            "user '{}' not found in passwd",
            username
        )
    )
}

fn list_users(
    root: &str,
) -> Result<Vec<String>> {
    let path =
        Path::new(root)
            .join("etc/passwd");

    let content =
        fs::read_to_string(&path)
            .with_context(|| {
                format!(
                    "failed to read {}",
                    path.display()
                )
            })?;

    let users =
        content
            .lines()
            .filter_map(|line| {
                let username =
                    line.split(':')
                        .next()?;

                if username.is_empty() {
                    None
                } else {
                    Some(
                        username.to_string()
                    )
                }
            })
            .collect();

    Ok(users)
}

fn get_password(
    user: &str,
    root: &str,
) -> Result<String> {
    let path =
        Path::new(root)
            .join("etc/shadow");

    let shadow =
        fs::read_to_string(&path)
            .with_context(|| {
                format!(
                    "failed to read {}",
                    path.display()
                )
            })?;

    for line in shadow.lines() {
        let mut fields =
            line.split(':');

        let username =
            fields
                .next()
                .unwrap_or_default();

        let password =
            fields
                .next()
                .unwrap_or_default();

        if username == user {
            return Ok(
                password.to_string()
            );
        }
    }

    Err(
        anyhow::anyhow!(
            "user '{}' not found",
            user
        )
    )
}

/* ============================================================
   Password input
============================================================ */

fn read_password() -> Result<String> {
    let stdin =
        io::stdin();

    let fd =
        stdin.as_raw_fd();

    let mut original =
        unsafe {
            std::mem::zeroed::<libc::termios>()
        };

    if unsafe {
        libc::tcgetattr(
            fd,
            &mut original,
        )
    } != 0 {
        return Err(
            anyhow::anyhow!(
                "failed to read terminal settings"
            )
        );
    }

    let mut hidden =
        original;

    hidden.c_lflag &=
        !libc::ECHO;

    if unsafe {
        libc::tcsetattr(
            fd,
            libc::TCSANOW,
            &hidden,
        )
    } != 0 {
        return Err(
            anyhow::anyhow!(
                "failed to disable password echo"
            )
        );
    }

    print!("Password: ");
    io::stdout().flush()?;

    let mut password =
        String::new();

    let result =
        io::stdin()
            .read_line(
                &mut password
            );

    unsafe {
        libc::tcsetattr(
            fd,
            libc::TCSANOW,
            &original
        );
    }

    println!();

    result?;

    while password.ends_with(
        &['\n', '\r'],
    ) {
        password.pop();
    }

    Ok(password)
}

/* ============================================================
   Login
============================================================ */

pub fn login(
    root: &str,
    version: &str,
    listener: UnixListener,
) -> Result<()> {
    clear_screen_ansi();

    /*
     * ========================================================
     * System event listener
     * ========================================================
     */

    thread::spawn(
        move || {
            if let Err(error) =
                start_event_listener(listener)
            {
                log_error!(
                    "{}",
                    format!(
                        "Event listener stopped: {}",
                        error
                    )
                    .red()
                );
            }
        }
    );

    /*
     * ========================================================
     * Login loop
     * ========================================================
     */

    loop {
        clear_screen_ansi();

        let users =
            list_users(root)?;

        if users.is_empty() {
            return Err(
                anyhow::anyhow!(
                    "no users found"
                )
            );
        }

        let username =
            Select::new(
                "Select user:",
                users,
            )
            .prompt()?;

        let password =
            read_password()?;

        let password_hash =
            get_password(
                &username,
                root,
            )?;

        let authenticated =
            verify_shadow_password(
                &password,
                &password_hash,
            )?;

        if !authenticated {
            println!();
            println!(
                "{}",
                "Invalid password."
                    .red()
                    .bold()
            );

            thread::sleep(
                Duration::from_secs(2)
            );

            clear_screen_ansi();

            continue;
        }

        /*
         * ====================================================
         * User authenticated
         * ====================================================
         */

        clear_screen_ansi();

        splash(
            version,
            &username,
        );

        let (uid, gid) =
            get_user_ids(
                &username,
                root,
            )?;

        /*
         * shell_start() blocks until
         * the shell exits.
         */

        shell_start(
            uid,
            gid,
        )?;
    }
}