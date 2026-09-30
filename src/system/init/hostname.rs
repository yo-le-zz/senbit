use anyhow::{Context, Result};
use std::fs;

pub fn get_hostname(root: &str) -> Result<String> {
    let path = format!("{}/etc/hostname", root);

    fs::read_to_string(&path)
        .with_context(|| format!("failed to read {}", path))
        .map(|hostname| hostname.trim().to_string())
}

pub fn init_hostname(root: &str) -> Result<()> {
    let path = format!("{}/etc/hostname", root);

    if !std::path::Path::new(&path).exists() {
        fs::write(&path, "senbit\n")
            .with_context(|| format!("failed to create {}", path))?;
    }

    Ok(())
}