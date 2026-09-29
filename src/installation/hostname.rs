use anyhow::Result;
use colored::Colorize;
use inquire::{
    Text,
    validator::Validation,
};
use std::path::Path;

pub fn get_hostname() -> Result<String> {
    let hostname = Text::new(&format!(
        "{} ",
        "Hostname:".bold().green()
    ))
    .with_default("senbit")
    .with_validator(|input: &str| {
        if input.is_empty() {
            return Ok(Validation::Invalid(
                "Hostname cannot be empty.".into()
            ));
        }

        if input.contains(' ') {
            return Ok(Validation::Invalid(
                "Hostname cannot contain spaces.".into()
            ));
        }

        Ok(Validation::Valid)
    })
    .prompt()?;

    Ok(hostname)
}

pub fn write_hostname(root: &Path, hostname: &str) -> Result<()> {
    std::fs::write(
        root.join("etc/hostname"),
        format!("{}\n", hostname),
    )?;

    Ok(())
}

pub fn setup_hostname(root: &Path) -> Result<String> {
    let hostname = get_hostname()?;

    write_hostname(root, &hostname)?;

    println!(
        "{} {}",
        "✓".green().bold(),
        format!("Hostname set to '{}'.", hostname)
            .green()
            .bold()
    );

    Ok(hostname)
}