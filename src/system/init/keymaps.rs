use anyhow::{Context, Result};
use std::fs;
use std::path::Path;
use std::process::{Command, Stdio};

use crate::logln;

fn get_keymap(root: &str) -> Result<String> {
    let config_path =
        Path::new(root)
            .join("etc/default/keyboard");

    let config =
        fs::read_to_string(&config_path)
            .with_context(|| {
                format!(
                    "Failed to read keyboard config: {}",
                    config_path.display()
                )
            })?;

    let keymap = config
        .lines()
        .find_map(|line| {
            let line = line.trim();

            line.strip_prefix("XKBLAYOUT=")
                .map(|value| {
                    value
                        .trim_matches('"')
                        .to_string()
                })
        })
        .unwrap_or_else(|| "us".to_string());

    Ok(
        if keymap == "en" {
            "us".to_string()
        } else {
            keymap
        }
    )
}

pub fn init_keymaps(root: &str) -> Result<()> {
    let keymap = get_keymap(root)?;

    let keymap_path = format!(
        "{}/usr/share/keymaps/{}.bmap",
        root,
        keymap
    );

    let file =
        fs::File::open(&keymap_path)
            .with_context(|| {
                format!(
                    "Failed to open keymap: {}",
                    keymap_path
                )
            })?;

    let status =
        Command::new("loadkmap")
            .stdin(Stdio::from(file))
            .status()
            .context("Failed to execute loadkmap")?;

    if !status.success() {
        anyhow::bail!(
            "Failed to load keyboard layout '{}'",
            keymap
        );
    }

    logln!(
        "Keyboard layout '{}' initialized.",
        keymap
    );

    Ok(())
}