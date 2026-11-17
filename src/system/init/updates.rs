use anyhow::{Context, Result};

pub fn get_version(root: &str) -> Result<String> {
    let path = std::path::Path::new(root).join("etc/senbit-release");

    let contents = std::fs::read_to_string(&path)
        .with_context(|| format!("failed to read {}", path.display()))?;

    contents
        .lines()
        .find_map(|line| line.strip_prefix("VERSION="))
        .map(str::trim)
        .filter(|version| !version.is_empty())
        .map(String::from)
        .ok_or_else(|| anyhow::anyhow!("VERSION not found in {}", path.display()))
}

/// Not implemented yet: no update channel exists, so there is never an update.
pub fn check_for_updates(_version: &str, _repo: &str) -> bool {
    false
}

/// Not implemented yet (see `check_for_updates`).
pub fn update_kernel(_repo: &str) -> Result<()> {
    Ok(())
}
