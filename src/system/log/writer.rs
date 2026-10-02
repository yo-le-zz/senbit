use anyhow::{Context, Result};
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use super::record::LogRecord;
use super::rotation::rotate_if_needed;

pub struct LogWriter {
    runtime_dir: PathBuf,
    persistent_dir: PathBuf,
}

impl LogWriter {
    pub fn new() -> Result<Self> {
        let runtime_dir =
            PathBuf::from("/run/log");

        let persistent_dir =
            PathBuf::from("/var/log/senbit");

        fs::create_dir_all(&runtime_dir)
            .context("failed to create /run/log")?;

        fs::create_dir_all(&persistent_dir)
            .context(
                "failed to create /var/log/senbit"
            )?;

        Ok(Self {
            runtime_dir,
            persistent_dir,
        })
    }

    pub fn write(
        &self,
        record: &LogRecord<'_>,
    ) -> Result<()> {
        let line = record.format();

        self.write_runtime(&line)?;
        self.write_persistent(&line)?;

        Ok(())
    }

    fn write_runtime(
        &self,
        line: &str,
    ) -> Result<()> {
        let path =
            self.runtime_dir.join("system.log");

        append_line(&path, line)
    }

    fn write_persistent(
        &self,
        line: &str,
    ) -> Result<()> {
        let path =
            self.persistent_dir.join("system.log");

        rotate_if_needed(&path)?;

        append_line(&path, line)
    }
}

fn append_line(
    path: &Path,
    line: &str,
) -> Result<()> {
    let mut file: File =
        OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .with_context(|| {
                format!(
                    "failed to open log {}",
                    path.display()
                )
            })?;

    file.write_all(line.as_bytes())
        .with_context(|| {
            format!(
                "failed to write log {}",
                path.display()
            )
        })?;

    file.flush()
        .with_context(|| {
            format!(
                "failed to flush log {}",
                path.display()
            )
        })?;

    Ok(())
}