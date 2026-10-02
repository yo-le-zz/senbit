// installation/users/questions.rs

use anyhow::Result;
use colored::Colorize;
use inquire::{Password, Text};

use super::user::User;

use std::fs;
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
        && chars.all(|c| {
            c.is_ascii_lowercase()
                || c.is_ascii_digit()
                || c == '_'
                || c == '-'
        })
}

/// Vrai si `name` existe déjà dans `<root>/etc/passwd`.
fn user_exists(root: &Path, name: &str) -> bool {
    fs::read_to_string(root.join("etc/passwd"))
        .map(|content| {
            content
                .lines()
                .any(|line| line.split(':').next() == Some(name))
        })
        .unwrap_or(false)
}

fn ask_username(root: &Path) -> Result<String> {
    loop {
        let name = Text::new(&format!("{} ", "Username:".bold().green()))
            .with_help_message("Lowercase letters, digits, '_' or '-'")
            .prompt()?;

        let name = name.trim().to_string();

        if !is_valid_username(&name) {
            println!(
                "{}",
                "Invalid username (lowercase, digits, '_' or '-', max 32)."
                    .red()
            );
            continue;
        }

        if user_exists(root, &name) {
            println!(
                "{}",
                format!("User '{}' already exists.", name).red()
            );
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

pub fn ask_user(root: &Path) -> Result<User> {
    let name = ask_username(root)?;

    let password = ask_password("")?;

    let home = Text::new(&format!("{} ", "Home directory:".bold().green()))
        .with_default(&format!("/home/{}", name))
        .prompt()?;

    let shell = Text::new(&format!("{} ", "Shell:".bold().green()))
        .with_default("/bin/sh")
        .prompt()?;

    User::new_auto(
        root,
        name,
        home.trim().to_string(),
        shell.trim().to_string(),
        &password,
    )
}