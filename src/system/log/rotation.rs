use anyhow::{Context, Result};
use std::fs;
use std::path::Path;

const MAX_LOG_SIZE: u64 = 10 * 1024 * 1024;
const ROTATED_LOGS: usize = 5;

pub fn rotate_if_needed(path: &Path) -> Result<()> {
    if !path.exists() {
        return Ok(());
    }

    let metadata = fs::metadata(path)
        .with_context(|| {
            format!(
                "failed to read log metadata: {}",
                path.display()
            )
        })?;

    if metadata.len() < MAX_LOG_SIZE {
        return Ok(());
    }

    for index in (1..=ROTATED_LOGS).rev() {
        let current = if index == 1 {
            path.to_path_buf()
        } else {
            path.with_extension(
                format!("log.{}", index - 1)
            )
        };

        let next = path.with_extension(
            format!("log.{}", index)
        );

        if next.exists() {
            fs::remove_file(&next)
                .with_context(|| {
                    format!(
                        "failed to remove old log: {}",
                        next.display()
                    )
                })?;
        }

        if current.exists() {
            fs::rename(&current, &next)
                .with_context(|| {
                    format!(
                        "failed to rotate log {} -> {}",
                        current.display(),
                        next.display()
                    )
                })?;
        }
    }

    Ok(())
}