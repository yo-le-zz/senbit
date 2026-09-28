// installation/users/questions.rs

use anyhow::Result;
use colored::Colorize;
use inquire::{Password, Text};

use super::user::User;

use std::path::Path;

pub fn ask_user(root: &Path) -> Result<User> {
    println!();
    println!("{}", "=== User configuration ===".bold().cyan());
    println!(
        "{}",
        "Create the main user that will be used to access Senbit."
            .dimmed()
    );
    println!();

    let name = Text::new(&format!("{} ", "Username:".bold().green()))
        .with_help_message("Name of the user to create")
        .prompt()?;

    let password = Password::new(&format!(
        "{} ",
        "Password:".bold().green()
    ))
    .with_display_mode(inquire::PasswordDisplayMode::Masked)
    .with_custom_confirmation_message(&format!(
        "{} ",
        "Confirmation:".bold().green()
    ))
    .prompt()?;

    let home = Text::new(&format!("{} ", "Home directory:".bold().green()))
        .with_default(&format!("/home/{}", name))
        .prompt()?;

    let shell = Text::new(&format!("{} ", "Shell:".bold().green()))
        .with_default("/bin/sh")
        .prompt()?;

    println!();
    println!("{}", "✓ User configuration completed.".green().bold());
    println!();

    User::new_auto(
        root,
        name,
        home,
        shell,
        &password,
    )
}