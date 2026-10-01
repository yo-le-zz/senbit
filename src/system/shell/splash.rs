use crate::logln;
use colored::Colorize;
use crate::utils::clear::clear_screen_ansi;

use std::fs;
use std::path::Path;

use std::env::set_var;

use anyhow::{Result, Context};

const HOME_DIR: &str = "/home/";

fn get_user_config(
    username: &str,
) -> Result<(String, String)> {
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
            line.split(':').collect();

        if fields.len() >= 7
            && fields[0] == username
        {
            let home = fields[5].to_string();
            let shell = fields[6].to_string();

            return Ok((
                home,
                shell,
            ));
        }
    }

    Err(anyhow::anyhow!(
        "User '{}' not found in /etc/passwd",
        username
    ))
}

fn set_env(username: &str) -> Result<()> {
    let (home, shell) =
        get_user_config(username)?;

    unsafe {
        set_var("USER", username);
        set_var("LOGNAME", username);

        set_var("HOME", &home);
        set_var("SHELL", &shell);

        set_var(
            "PATH",
            "/usr/local/bin:/usr/bin:/bin",
        );

        set_var("PWD", &home);
        set_var("OLDPWD", &home);

        set_var(
            "PS1",
            format!(
                "[{}@Senbit \\w]$ ",
                username
            ),
        );

        set_var(
            "TERM",
            "xterm-256color",
        );
    }

    Ok(())
}

pub fn splash(version: &str, username: &str) {
    clear_screen_ansi();
    logln!(
        "{}",
        format!("Welcome to Senbit {}", version)
            .cyan()
            .bold()
    );

    logln!(
        "{}",
        format!("Hello, {}!", username)
            .green()
            .bold()
    );
    // set environment variables
    set_env(&username);
}
