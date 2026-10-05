use anyhow::{Context, Result};
use std::fmt::Arguments;
use std::sync::{
    Mutex,
    OnceLock,
};
use std::time::{
    SystemTime,
    UNIX_EPOCH,
};

use super::level::LogLevel;
use super::record::LogRecord;
use super::writer::LogWriter;

static LOGGER: OnceLock<Logger> =
    OnceLock::new();

pub struct Logger {
    writer: Mutex<LogWriter>,
    minimum_level: LogLevel,
}

impl Logger {
    pub fn new() -> Result<Self> {
        Ok(Self {
            writer: Mutex::new(
                LogWriter::new()?,
            ),
            minimum_level: LogLevel::Info,
        })
    }

    pub fn log(
        &self,
        level: LogLevel,
        component: &str,
        message: Arguments<'_>,
    ) {
        if level > self.minimum_level {
            return;
        }

        let timestamp =
            current_timestamp();

        let record = LogRecord {
            timestamp,
            level,
            component,
            message: message.to_string(),
        };

        super::debug::mirror(&record.format());

        let writer =
            match self.writer.lock() {
                Ok(writer) => writer,
                Err(_) => {
                    eprintln!(
                        "[ERROR] logger: logger mutex poisoned"
                    );
                    return;
                }
            };

        if let Err(error) =
            writer.write(&record)
        {
            eprintln!(
                "[ERROR] logger: failed to write log: {}",
                error
            );
        }
    }
}

pub fn init_logs() -> Result<()> {
    if LOGGER.get().is_some() {
        return Ok(());
    }

    let logger =
        Logger::new()
            .context(
                "failed to initialize logger"
            )?;

    LOGGER
        .set(logger)
        .map_err(|_| {
            anyhow::anyhow!(
                "logger already initialized"
            )
        })?;

    Ok(())
}

pub fn log(
    level: LogLevel,
    component: &str,
    message: Arguments<'_>,
) {
    match LOGGER.get() {
        Some(logger) => {
            logger.log(
                level,
                component,
                message,
            );
        }

        None => {
            super::debug::mirror(&format!(
                "[{}] {}: {}",
                level.as_str(),
                component,
                message
            ));

            eprintln!(
                "[{}] {}: {}",
                level.as_str(),
                component,
                message
            );
        }
    }
}

fn current_timestamp() -> String {
    let duration =
        match SystemTime::now()
            .duration_since(UNIX_EPOCH)
        {
            Ok(duration) => duration,

            Err(_) => {
                return "1970-01-01T00:00:00Z"
                    .to_string();
            }
        };

    let seconds =
        duration.as_secs();

    let days =
        seconds / 86_400;

    let remaining =
        seconds % 86_400;

    let hours =
        remaining / 3_600;

    let minutes =
        (remaining % 3_600) / 60;

    let secs =
        remaining % 60;

    let (year, month, day) =
        unix_days_to_date(days);

    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        year,
        month,
        day,
        hours,
        minutes,
        secs,
    )
}

fn unix_days_to_date(
    days: u64,
) -> (i32, u32, u32) {
    let days =
        days as i64;

    let z =
        days + 719_468;

    let era =
        if z >= 0 {
            z / 146_097
        } else {
            (z - 146_096) / 146_097
        };

    let doe =
        z - era * 146_097;

    let yoe =
        (doe
            - doe / 1_460
            + doe / 36_524
            - doe / 146_096)
            / 365;

    let y =
        yoe
            + era * 400;

    let doy =
        doe
            - (365 * yoe
                + yoe / 4
                - yoe / 100);

    let mp =
        (5 * doy + 2) / 153;

    let d =
        doy
            - (153 * mp + 2) / 5
            + 1;

    let m =
        mp
            + if mp < 10 {
                3
            } else {
                -9
            };

    let y =
        y
            + if m <= 2 {
                1
            } else {
                0
            };

    (
        y as i32,
        m as u32,
        d as u32,
    )
}