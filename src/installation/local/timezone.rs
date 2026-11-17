use crate::log_info;
use anyhow::Result;
use colored::Colorize;
use inquire::Select;
use std::fs::File;
use std::io::Write;
use std::path::Path;

const TIMEZONES_LIST: &[&str] = &[
    "UTC",
    // Europe
    "Europe/Paris",
    "Europe/London",
    "Europe/Berlin",
    "Europe/Madrid",
    "Europe/Rome",
    "Europe/Amsterdam",
    "Europe/Brussels",
    "Europe/Zurich",
    "Europe/Vienna",
    "Europe/Prague",
    "Europe/Warsaw",
    "Europe/Budapest",
    "Europe/Bucharest",
    "Europe/Athens",
    "Europe/Helsinki",
    "Europe/Stockholm",
    "Europe/Copenhagen",
    "Europe/Oslo",
    "Europe/Lisbon",
    "Europe/Dublin",
    "Europe/Moscow",
    // America
    "America/New_York",
    "America/Chicago",
    "America/Denver",
    "America/Los_Angeles",
    "America/Toronto",
    "America/Vancouver",
    "America/Mexico_City",
    "America/Sao_Paulo",
    "America/Argentina/Buenos_Aires",
    // Asia
    "Asia/Tokyo",
    "Asia/Seoul",
    "Asia/Shanghai",
    "Asia/Hong_Kong",
    "Asia/Singapore",
    "Asia/Bangkok",
    "Asia/Kolkata",
    "Asia/Dubai",
    "Asia/Jerusalem",
    "Asia/Riyadh",
    // Africa
    "Africa/Cairo",
    "Africa/Johannesburg",
    "Africa/Lagos",
    "Africa/Nairobi",
    // Oceania
    "Australia/Sydney",
    "Australia/Melbourne",
    "Australia/Perth",
    "Pacific/Auckland",
    "Pacific/Honolulu",
];

/// Question : nothing is written yet (see installation::plan).
pub fn ask_timezone() -> Result<String> {
    let timezone = Select::new("Select a timezone:", TIMEZONES_LIST.to_vec())
        .prompt()
        .map_err(|e| anyhow::anyhow!("Failed to select timezone: {}", e))?;

    Ok(timezone.to_string())
}

/// Application of the answer of `ask_timezone`.
pub fn apply_timezone(mount_point: &Path, timezone: &str) -> Result<()> {
    log_info!("{}", format!("Setting up timezone {}...", timezone).green());

    write_timezone(timezone, mount_point)
        .map_err(|e| anyhow::anyhow!("Failed to write timezone: {}", e))
}

fn write_timezone(timezone: &str, mount_point: &Path) -> Result<()> {
    let path = mount_point.join("etc/timezone");

    let mut file = File::create(&path)
        .map_err(|e| anyhow::anyhow!("Cannot create {}: {}", path.display(), e))?;

    writeln!(file, "{}", timezone)
        .map_err(|e| anyhow::anyhow!("Cannot write {}: {}", path.display(), e))?;

    Ok(())
}
