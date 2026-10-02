use anyhow::{
    Context,
    Result,
};

use std::fs;
use std::path::{
    Path,
    PathBuf,
};

use crate::cli::Source;

pub const PERSISTENT_LOG:
    &str = "/var/log/senbit/system.log";

pub const RUNTIME_LOG:
    &str = "/run/log/system.log";

#[derive(Debug, Clone)]
pub struct LogSource {
    pub name: &'static str,
    pub path: PathBuf,
    pub persistent: bool,
}

impl LogSource {
    pub fn size(&self) -> u64 {
        fs::metadata(&self.path)
            .map(|metadata| metadata.len())
            .unwrap_or(0)
    }

    pub fn exists(&self) -> bool {
        self.path.is_file()
    }
}

pub fn sources(
    source: Option<Source>,
) -> Vec<LogSource> {
    match source.unwrap_or(
        Source::All
    ) {
        Source::All => vec![
            persistent(),
            runtime(),
        ],

        Source::Persistent => vec![
            persistent()
        ],

        Source::Runtime => vec![
            runtime()
        ],

        Source::System => vec![
            persistent(),
            runtime(),
        ],
    }
}

pub fn persistent() -> LogSource {
    LogSource {
        name: "persistent",
        path: PathBuf::from(
            PERSISTENT_LOG
        ),
        persistent: true,
    }
}

pub fn runtime() -> LogSource {
    LogSource {
        name: "runtime",
        path: PathBuf::from(
            RUNTIME_LOG
        ),
        persistent: false,
    }
}

pub fn ensure_parent(
    path: &Path,
) -> Result<()> {
    if let Some(parent) =
        path.parent()
    {
        fs::create_dir_all(parent)
            .with_context(|| {
                format!(
                    "failed to create {}",
                    parent.display()
                )
            })?;
    }

    Ok(())
}