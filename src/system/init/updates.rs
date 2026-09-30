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

pub fn check_for_updates(version: &str, repo: &str) -> bool {
    false
}

pub fn update_kernel(repo: &str) -> Result<()> {
    Ok(())
}