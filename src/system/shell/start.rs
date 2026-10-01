use crate::logln;
use crate::utils::clear::clear_screen_ansi;
use colored::Colorize;

use std::env::set_var;
use std::fs;
use std::path::Path;

use anyhow::{Context, Result};

use crate::system::shell::prompt::{build_prompt, prepare_console, term_for, ColorMode};

fn get_user_config(
    username: &str,
) -> Result<(u32, u32, String, String)> {
    let passwd_path = Path::new("/etc/passwd");

    let passwd =
        fs::read_to_string(passwd_path)
            .with_context(|| {
                format!(
                    "Failed to read {}",
                    passwd_path.display()
                )
            })?;

    for line in passwd.lines() {
        let fields: Vec<&str> =
            line.split(':')
                .collect();

        if fields.len() >= 7
            && fields[0] == username
        {
            let uid =
                fields[2]
                    .parse::<u32>()
                    .with_context(|| {
                        format!(
                            "Invalid UID for user '{}'",
                            username
                        )
                    })?;

            let gid =
                fields[3]
                    .parse::<u32>()
                    .with_context(|| {
                        format!(
                            "Invalid GID for user '{}'",
                            username
                        )
                    })?;

            return Ok((
                uid,
                gid,
                fields[5].to_string(),
                fields[6].to_string(),
            ));
        }
    }

    Err(anyhow::anyhow!(
        "User '{}' not found in /etc/passwd",
        username
    ))
}

pub fn set_env(
    username: &str,
    mode: ColorMode,
) -> Result<()> {
    let (
        _uid,
        _gid,
        home,
        shell,
    ) =
        get_user_config(username)?;

    unsafe {
        set_var("USER", username);
        set_var("LOGNAME", username);
        set_var("HOME", &home);
        set_var("SHELL", &shell);
        set_var(
            "PATH",
            "/usr/local/bin:/usr/bin:/bin:/usr/local/sbin:/usr/sbin:/sbin",
        );
        set_var("PWD", &home);
        set_var("OLDPWD", &home);
        set_var("TERM", term_for(mode));
    }

    let ps1 =
        build_prompt(mode);

    unsafe {
        set_var("PS1", ps1);
    }

    Ok(())
}

pub fn splash(version: &str, username: &str) {
    let mode = prepare_console();

    clear_screen_ansi();

    logln!(
        "{}",
        format!("Welcome to Senbit {}", version).cyan().bold()
    );

    logln!("{}", format!("Hello, {}!", username).green().bold());

    if let Err(e) = set_env(username, mode) {
        logln!(
            "{}",
            format!("Failed to set environment: {:#}", e).red()
        );
    }
}