use anyhow::{Context, Result};
use std::fs;
use std::io;
use std::path::Path;

pub fn get_timezone(root: &str) -> Result<String> {
    let path = Path::new(root).join("etc/timezone");

    match fs::read_to_string(&path) {
        Ok(tz) => {
            let tz = tz.trim();
            Ok(if tz.is_empty() { "UTC".to_string() } else { tz.to_string() })
        }
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok("UTC".to_string()),
        Err(e) => Err(e).with_context(|| format!("Failed to read {}", path.display())),
    }
}