// src/utils/clear.rs

use anyhow::{Context, Result}; // <- Result vient de anyhow
use std::io::{self, Write};
use std::process::Command;

pub fn clear_screen() -> Result<()> {
    let status = Command::new("tput")
        .arg("clear")
        .status()
        .context("failed to run `tput clear`")?;

    if !status.success() {
        anyhow::bail!("`tput clear` exited with status: {status:?}");
    }

    Ok(())
}

pub fn clear_screen_ansi() {
    print!("\x1b[2J\x1b[1;1H");
    let _ = io::stdout().flush();
}