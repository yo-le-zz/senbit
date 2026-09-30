
use anyhow::{Context, Result};
use std::process::Command;

pub fn start_shell() -> Result<()> {
    Command::new("/bin/sh")
        .status()
        .context("failed to start shell")?;

    Ok(())
}