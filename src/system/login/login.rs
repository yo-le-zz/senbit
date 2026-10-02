use anyhow::{
    Context,
    Result,
};

use colored::Colorize;

use inquire::{
    Password,
    Select,
};

use std::fs;
use std::os::unix::net::UnixListener;
use std::path::Path;
use std::thread;

use crate::logln;

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
     *
     * The listener runs independently from the
     * login and shell session.
     */

    thread::spawn(
        move || {
            if let Err(error) =
                start_event_listener(listener)
            {
                logln!(
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
        logln!(
            "{}",
            "Login screen"
                .cyan()
                .bold()
        );

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
            Password::new(
                "Password:",
            )
            .with_display_mode(
                inquire::PasswordDisplayMode::Masked,
            )
            .prompt()?;

        let password_hash =
            get_password(
                &username,
                root,
            )?;

        if !verify_shadow_password(
            &password,
            &password_hash,
        )? {
            logln!(
                "{}",
                "Invalid password."
                    .red()
            );

            continue;
        }

        /*
         * ====================================================
         * User authenticated
         * ====================================================
         */

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
         * shell_start() blocks until the shell exits.
         */

        shell_start(
            uid,
            gid,
        )?;

        logln!(
            "{}",
            "Shell exited. Returning to login..."
                .cyan()
        );
    }
}