use regex::Regex;
use serde::Serialize;

use crate::cli::Level;

#[derive(Debug, Clone, Serialize)]
pub struct LogEntry {
    pub timestamp: Option<String>,
    pub level: Option<String>,
    pub module: Option<String>,
    pub message: String,
    pub raw: String,
}

impl LogEntry {
    pub fn level_enum(&self) -> Option<Level> {
        match self.level.as_deref() {
            Some("ERROR") => Some(Level::Error),
            Some("WARN") | Some("WARNING") => Some(Level::Warn),
            Some("INFO") => Some(Level::Info),
            Some("DEBUG") => Some(Level::Debug),
            Some("TRACE") => Some(Level::Trace),
            _ => None,
        }
    }
}

pub fn parse_line(line: &str) -> LogEntry {
    let raw = line.to_string();

    let timestamp_regex =
        Regex::new(
            r"^(\d{4}-\d{2}-\d{2}[T ][0-9:.+-]+)"
        ).unwrap();

    let level_regex =
        Regex::new(
            r"\[(ERROR|WARN|WARNING|INFO|DEBUG|TRACE)\]"
        ).unwrap();

    let module_regex =
        Regex::new(
            r"\]\s+([A-Za-z0-9_:\-./]+):\s*(.*)$"
        ).unwrap();

    let timestamp =
        timestamp_regex
            .captures(line)
            .and_then(|c| c.get(1))
            .map(|m| m.as_str().to_string());

    let level =
        level_regex
            .captures(line)
            .and_then(|c| c.get(1))
            .map(|m| {
                match m.as_str() {
                    "WARNING" => "WARN".to_string(),
                    value => value.to_string(),
                }
            });

    let module =
        module_regex
            .captures(line)
            .and_then(|c| c.get(1))
            .map(|m| m.as_str().to_string());

    let message =
        module_regex
            .captures(line)
            .and_then(|c| c.get(2))
            .map(|m| m.as_str().to_string())
            .unwrap_or_else(|| {
                let mut value = line.to_string();

                if let Some(timestamp) = &timestamp {
                    value = value
                        [timestamp.len()..]
                        .trim()
                        .to_string();
                }

                if let Some(level) = &level {
                    let marker = format!(
                        "[{}]",
                        level
                    );

                    value = value
                        .replace(
                            &marker,
                            "",
                        )
                        .trim()
                        .to_string();
                }

                value
            });

    LogEntry {
        timestamp,
        level,
        module,
        message,
        raw,
    }
}