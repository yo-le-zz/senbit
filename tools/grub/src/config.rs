use anyhow::{Context, Result};
use serde::Deserialize;
use std::fs;

#[derive(Debug, Deserialize)]
pub struct Config {
    pub grub: GrubConfig,
    pub build: BuildConfig,
    pub targets: TargetsConfig,
}

#[derive(Debug, Deserialize)]
pub struct GrubConfig {
    pub version: String,
    pub source: String,
}

#[derive(Debug, Deserialize)]
pub struct BuildConfig {
    pub jobs: String,
}

#[derive(Debug, Deserialize)]
pub struct TargetsConfig {
    pub legacy: bool,
    pub efi: bool,
}

impl Config {
    pub fn load(path: &str) -> Result<Self> {
        let content = fs::read_to_string(path)
            .with_context(|| format!("Failed to read {}", path))?;

        let config = toml::from_str(&content)
            .with_context(|| format!("Failed to parse {}", path))?;

        Ok(config)
    }
}