// installation/lang.rs

use inquire::Select;
use crate::{elogln, logln};

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
        elogln!("Aucun keymap disponible dans /usr/share/keymaps/");
        return "us".to_string();
    }

    Select::new("Select a keyboard layout", options)
        .prompt()
        .unwrap_or_else(|_| "us".to_string())
}

fn set_keyboard_layout(lang: &str) -> Result<()> {
    let keymap = if lang == "en" { "us" } else { lang };
    let path = format!("/usr/share/keymaps/{}.bmap", keymap);

    if !std::path::Path::new(&path).exists() {
        elogln!(
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

pub fn setup_lang() -> anyhow::Result<()> {
    let lang = keyboard_select();

    if lang.is_empty() {
        logln!(
            "{}",
            "Failed to set keyboard layout. ( qwerty fallback )".red()
        );

        set_keyboard_layout("en")
            .map_err(|e| anyhow::anyhow!("Failed to set fallback keyboard layout: {}", e))?;

        return Ok(());
    }

    if let Err(e) = set_keyboard_layout(&lang) {
        logln!(
            "{}",
            format!(
                "Failed to set keyboard layout '{}': {}. ( qwerty fallback )",
                lang, e
            )
            .red()
        );

        if let Err(e) = set_keyboard_layout("en") {
            logln!(
                "{}",
                format!("Failed to set fallback keyboard layout: {}", e).red()
            );
        }
    }

    Ok(())
}