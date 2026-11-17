use anyhow::{
    bail,
    Context,
    Result,
};

use regex::Regex;

use crate::cli::{
    Cli,
    Level,
};

use crate::parser::LogEntry;

pub struct Filter {
    grep: Option<String>,
    regex: Option<Regex>,
    exclude: Vec<String>,
    exclude_modules: Vec<String>,
    exclude_levels: Vec<Level>,
    minimum_level: Option<Level>,
    since: Option<String>,
    until: Option<String>,
}

impl Filter {
    pub fn from_cli(cli: &Cli) -> Result<Self> {
        let regex =
            match &cli.regex {
                Some(pattern) => Some(
                    Regex::new(pattern)
                        .with_context(|| {
                            format!(
                                "invalid regular expression '{}'",
                                pattern
                            )
                        })?
                ),
                None => None,
            };

        let since =
            normalize_time(
                cli.since.as_deref()
            )?;

        let until =
            normalize_time(
                cli.until.as_deref()
            )?;

        Ok(Self {
            grep: cli.grep.clone(),
            regex,
            exclude: cli.exclude.clone(),
            exclude_modules: cli.exclude_module.clone(),
            exclude_levels: cli.exclude_level.clone(),
            minimum_level: cli.level,
            since,
            until,
        })
    }

    pub fn matches(
        &self,
        entry: &LogEntry,
    ) -> bool {
        if let Some(minimum) =
            self.minimum_level
        {
            if let Some(level) =
                entry.level_enum()
            {
                if level.priority()
                    > minimum.priority()
                {
                    return false;
                }
            }
        }

        if self.exclude_levels
            .iter()
            .any(|level| {
                entry
                    .level_enum()
                    .map(|entry_level| {
                        entry_level.as_str()
                            == level.as_str()
                    })
                    .unwrap_or(false)
            })
        {
            return false;
        }

        if let Some(grep) =
            &self.grep
        {
            if !entry.raw
                .to_lowercase()
                .contains(
                    &grep.to_lowercase()
                )
            {
                return false;
            }
        }

        if let Some(regex) =
            &self.regex
        {
            if !regex.is_match(
                &entry.raw
            ) {
                return false;
            }
        }

        for excluded in
            &self.exclude
        {
            if entry.raw
                .to_lowercase()
                .contains(
                    &excluded.to_lowercase()
                )
            {
                return false;
            }
        }

        for module in
            &self.exclude_modules
        {
            if entry
                .module
                .as_deref()
                .map(|value| {
                    value
                        .to_lowercase()
                        .contains(
                            &module.to_lowercase()
                        )
                })
                .unwrap_or(false)
            {
                return false;
            }
        }

        if let Some(since) =
            &self.since
        {
            if let Some(timestamp) =
                &entry.timestamp
            {
                if timestamp < since {
                    return false;
                }
            }
        }

        if let Some(until) =
            &self.until
        {
            if let Some(timestamp) =
                &entry.timestamp
            {
                if timestamp > until {
                    return false;
                }
            }
        }

        true
    }
}

fn normalize_time(
    value: Option<&str>,
) -> Result<Option<String>> {
    let Some(value) = value else {
        return Ok(None);
    };

    if value.is_empty() {
        bail!("time value cannot be empty");
    }

    if let Some(duration) =
        parse_duration_seconds(value)
    {
        let now =
            std::time::SystemTime::now();

        let timestamp =
            now.checked_sub(
                std::time::Duration::from_secs(
                    duration
                )
            )
            .context(
                "failed to calculate relative timestamp"
            )?;

        let seconds =
            timestamp
                .duration_since(
                    std::time::UNIX_EPOCH
                )?
                .as_secs();

        return Ok(
            Some(
                format_unix_timestamp(
                    seconds
                )
            )
        );
    }

    Ok(Some(
        value.to_string()
    ))
}

fn parse_duration_seconds(
    value: &str,
) -> Option<u64> {
    let unit =
        value.chars().last()?;

    let number =
        &value[..value.len() - 1];

    let number =
        number.parse::<u64>().ok()?;

    match unit {
        's' => Some(number),
        'm' => Some(number * 60),
        'h' => Some(number * 60 * 60),
        'd' => Some(number * 60 * 60 * 24),
        _ => None,
    }
}

fn format_unix_timestamp(
    seconds: u64,
) -> String {
    let days =
        seconds / 86_400;

    let remaining =
        seconds % 86_400;

    let hour =
        remaining / 3_600;

    let minute =
        (remaining % 3_600) / 60;

    let second =
        remaining % 60;

    let (
        year,
        month,
        day,
    ) = civil_from_days(
        days as i64
    );

    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}",
        year,
        month,
        day,
        hour,
        minute,
        second
    )
}

fn civil_from_days(
    days: i64,
) -> (i32, u32, u32) {
    let z =
        days + 719468;

    let era =
        if z >= 0 {
            z / 146097
        } else {
            (z - 146096) / 146097
        };

    let doe =
        z - era * 146097;

    let yoe =
        (doe
            - doe / 1460
            + doe / 36524
            - doe / 146096)
            / 365;

    let y =
        yoe + era * 400;

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