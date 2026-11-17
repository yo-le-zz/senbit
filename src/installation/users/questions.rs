// installation/users/questions.rs

use anyhow::Result;
use colored::Colorize;
use inquire::validator::Validation;
use inquire::{Password, Text};

use std::path::Path;

/// Un nom d'utilisateur valide : commence par une minuscule ou `_`,
/// puis minuscules, chiffres, `_` ou `-`, 32 caractères maximum.
fn is_valid_username(name: &str) -> bool {
    let mut chars = name.chars();

    match chars.next() {
        Some(c) if c.is_ascii_lowercase() || c == '_' => {}
        _ => return false,
    }

    name.len() <= 32
        && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-')
}

fn ask_username() -> Result<String> {
    loop {
        let name = Text::new(&format!("{} ", "Username:".bold().green()))
            .with_help_message("Lowercase letters, digits, '_' or '-'")
            .prompt()?;

        let name = name.trim().to_string();

        if !is_valid_username(&name) {
            println!(
                "{}",
                "Invalid username (lowercase, digits, '_' or '-', max 32).".red()
            );
            continue;
        }

        if name == "root" || name == "nobody" {
            println!("{}", format!("'{}' is reserved.", name).red());
            continue;
        }

        return Ok(name);
    }
}

pub fn ask_password(question: &str) -> Result<String> {
    loop {
        let password = Password::new(&format!(
            "{} ",
            if question.is_empty() {
                "Password:".bold().green()
            } else {
                format!("{}:", question).bold().green()
            }
        ))
        .with_display_mode(inquire::PasswordDisplayMode::Masked)
        .prompt()?;

        if password.is_empty() {
            println!("{}", "Password must not be empty.".red());
            continue;
        }

        return Ok(password);
    }
}

/// Everything the installer asks about accounts. Nothing is written until the
/// final confirmation: the passwords only live in memory until then (this type
/// deliberately does not implement Debug).
pub struct AccountsPlan {
    pub root_password: String,
    pub username: String,
    pub user_password: String,
    pub home: String,
    pub shell: String,
}

fn starts_with_slash(input: &str) -> Result<Validation, Box<dyn std::error::Error + Send + Sync>> {
    if input.trim().starts_with('/') {
        Ok(Validation::Valid)
    } else {
        Ok(Validation::Invalid("Must be an absolute path.".into()))
    }
}

pub fn ask_accounts() -> Result<AccountsPlan> {
    let root_password = ask_password("Please enter the root/sudo password")?;

    let username = ask_username()?;

    let user_password = ask_password(&format!("Password for {}", username))?;

    let home = Text::new(&format!("{} ", "Home directory:".bold().green()))
        .with_default(&format!("/home/{}", username))
        .with_validator(starts_with_slash)
        .prompt()?;

    // bash comes with the Debian packages of the image; /bin/sh (BusyBox) otherwise.
    let default_shell = if Path::new("/bin/bash").exists() {
        "/bin/bash"
    } else {
        "/bin/sh"
    };

    let shell = Text::new(&format!("{} ", "Shell:".bold().green()))
        .with_default(default_shell)
        .with_validator(starts_with_slash)
        .prompt()?;

    Ok(AccountsPlan {
        root_password,
        username,
        user_password,
        home: home.trim().to_string(),
        shell: shell.trim().to_string(),
    })
}
