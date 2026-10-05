use anyhow::{Context, Result};
use std::fs;
use std::path::Path;

pub fn init_hostname(root: &str) -> Result<()> {
    let etc_dir = Path::new(root).join("etc");
    let hostname_path = etc_dir.join("hostname");

    fs::create_dir_all(&etc_dir)
        .with_context(|| {
            format!(
                "Failed to create {}",
                etc_dir.display()
            )
        })?;

    if !hostname_path.exists() {
        fs::write(
            &hostname_path,
            "senbit\n",
        )
        .with_context(|| {
            format!(
                "Failed to create {}",
                hostname_path.display()
            )
        })?;
    }

    Ok(())
}