use anyhow::{Context, Result};
use colored::Colorize;
use inquire::{Password, Select};

use std::fs;
use std::path::Path;
use crate::system::shell::shell::start_shell;

use crate::system::shell::splash::splash;

use crate::logln;
use crate::utils::crypto::hash::verify_password;
use crate::utils::clear::clear_screen_ansi;

fn list_users(root: &str) -> Result<Vec<String>> {
    let path = Path::new(root).join("etc/passwd");

    let content = fs::read_to_string(&path)
        .with_context(|| format!("failed to read {}", path.display()))?;

    let users = content
        .lines()
        .filter_map(|line| {
            let username = line.split(':').next()?;

            if username.is_empty() {
                None
            } else {
                Some(username.to_string())
            }
        })
        .collect();

    Ok(users)
}

fn get_password(user: &str, root: &str) -> Result<String> {
    let path = Path::new(root).join("etc/shadow");

    let shadow = fs::read_to_string(&path)
        .with_context(|| format!("failed to read {}", path.display()))?;

    for line in shadow.lines() {
        let mut fields = line.split(':');

        let username = fields.next().unwrap_or_default();
        let password = fields.next().unwrap_or_default();

        if username == user {
            return Ok(password.to_string());
        }
    }

    Err(anyhow::anyhow!("user '{}' not found", user))
}

pub fn login(root: &str, version: &str) -> Result<()> {
    clear_screen_ansi();
    
    loop {
        logln!("{}", "Login screen".cyan().bold());

        let users = list_users(root)?;

        if users.is_empty() {
            return Err(anyhow::anyhow!("no users found"));
        }

        let username = Select::new("Select user:", users)
            .prompt()?;

        let password = Password::new("Password:")
            .with_display_mode(inquire::PasswordDisplayMode::Masked)
            .prompt()?;

        let password_hash = get_password(&username, root)?;

        if !verify_password(&password, &password_hash)? {
            logln!("{}", "Invalid password.".red());
            continue;
        }

        splash(&version, &username);

        start_shell()?;

        logln!("{}", "Shell exited. Returning to login...".cyan());
    }
}