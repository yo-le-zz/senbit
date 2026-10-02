// installation/lang.rs

use inquire::Select;
use crate::{log_error, log_info};

use std::path::Path;

use std::process::Stdio;
use std::process::Command;
use colored::Colorize;
use anyhow::{Context, Result};

fn get_keymap() -> Vec<String> {
    std::fs::read_dir("/usr/share/keymaps/")
        .expect("Répertoire inexistant")
        .filter_map(|entry| entry.ok())
        .filter_map(|entry| {
            entry
                .file_name()
                .to_str()
                .map(|name| name.to_string())
        })
        .filter(|name| name.ends_with(".bmap"))
        .map(|name| {
            name.trim_end_matches(".bmap").to_string()
        })
        .filter(|name| !name.is_empty())
        .collect()
}

pub fn keyboard_select() -> String {
    let options = get_keymap();

    if options.is_empty() {
        log_error!("Aucun keymap disponible dans /usr/share/keymaps/");
        return "us".to_string();
    }

    Select::new("Select a keyboard layout", options)
        .prompt()
        .unwrap_or_else(|_| "us".to_string())
}

pub fn set_keyboard_layout(lang: &str) -> Result<()> {
    let keymap = if lang == "en" { "us" } else { lang };
    let path = format!("/usr/share/keymaps/{}.bmap", keymap);

    if !std::path::Path::new(&path).exists() {
        log_error!(
            "Failed to set keyboard layout '{}': Keyboard layout '{}' not found. (us fallback)",
            lang, keymap
        );

        let fallback = "/usr/share/keymaps/us.bmap";

        if !std::path::Path::new(fallback).exists() {
            anyhow::bail!("Failed to set fallback keyboard layout: 'us' not found");
        }

        let file = std::fs::File::open(fallback)
            .context("Failed to open fallback keyboard layout")?;

        let status = Command::new("loadkmap")
            .stdin(Stdio::from(file))
            .status()
            .context("Failed to execute loadkmap")?;

        if !status.success() {
            anyhow::bail!("Failed to load fallback keyboard layout");
        }

        return Ok(());
    }

    let file = std::fs::File::open(&path)
        .with_context(|| format!("Failed to open keyboard layout '{}'", path))?;

    let status = Command::new("loadkmap")
        .stdin(Stdio::from(file))
        .status()
        .with_context(|| format!("Failed to execute loadkmap for '{}'", keymap))?;

    if !status.success() {
        anyhow::bail!("Failed to load keyboard layout '{}'", keymap);
    }

    Ok(())
}

fn write_config(lang: &str) -> anyhow::Result<()> {
    let config = format!(
        "XKBMODEL=\"pc105\"\n\
         XKBLAYOUT=\"{}\"\n\
         XKBVARIANT=\"\"\n\
         XKBOPTIONS=\"\"\n",
        lang
    );

    let path = Path::new("/etc/default/keyboard");

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| {
                format!(
                    "Failed to create keyboard config directory: {}",
                    parent.display()
                )
            })?;
    }

    std::fs::write(&path, config)
        .with_context(|| {
            format!(
                "Failed to write keyboard layout config: {}",
                path.display()
            )
        })?;

    Ok(())
}

pub fn setup_lang() -> anyhow::Result<()> {
    let lang = keyboard_select();

    if lang.is_empty() {
        log_error!(
            "{}",
            "Failed to set keyboard layout. ( qwerty fallback )".red()
        );

        set_keyboard_layout("en")
            .map_err(|e| anyhow::anyhow!("Failed to set fallback keyboard layout: {}", e))?;

        return Ok(());
    }

    if let Err(e) = set_keyboard_layout(&lang) {
        log_error!(
            "{}",
            format!(
                "Failed to set keyboard layout '{}': {}. ( qwerty fallback )",
                lang, e
            )
            .red()
        );

        if let Err(e) = set_keyboard_layout("en") {
            log_error!(
                "{}",
                format!("Failed to set fallback keyboard layout: {}", e).red()
            );
        }
    }
    
    write_config(&lang)?;
    
    Ok(())
}