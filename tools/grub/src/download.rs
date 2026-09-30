use anyhow::{Context, Result};
use colored::Colorize;
use reqwest::blocking::Client;
use std::fs::File;
use std::io::copy;
use std::path::Path;

use crate::config::Config;

pub fn download(config: &Config, output: &Path) -> Result<()> {
    let archive = output.join(format!(
        "grub-{}.tar.xz",
        config.grub.version
    ));

    if archive.exists() {
        println!(
            "{} {}",
            "GRUB archive already exists:".cyan(),
            archive.display()
        );

        return Ok(());
    }

    println!(
        "{} {}",
        "Downloading GRUB from".cyan(),
        config.grub.source
    );

    let client = Client::builder()
        .user_agent("Senbit/0.1 (+https://github.com/yo-le-zz/senbit)")
        .build()
        .context("Failed to create HTTP client")?;

    let mut response = client
        .get(&config.grub.source)
        .send()
        .context("Failed to download GRUB")?;

    response
        .error_for_status_ref()
        .context("GRUB download returned an error")?;

    let mut file = File::create(&archive)
        .context("Failed to create GRUB archive")?;

    copy(&mut response, &mut file)
        .context("Failed to write GRUB archive")?;

    println!(
        "{} {}",
        "GRUB downloaded:".green(),
        archive.display()
    );

    Ok(())
}