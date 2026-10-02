use anyhow::Result;

use crate::parser::LogEntry;

pub struct OutputOptions {
    pub json: bool,
    pub raw: bool,
    pub color: bool,
}

pub fn print_entry(
    entry: &LogEntry,
    options: &OutputOptions,
) -> Result<()> {
    if options.json {
        println!(
            "{}",
            serde_json::to_string(entry)?
        );

        return Ok(());
    }

    if options.raw {
        println!(
            "{}",
            entry.raw
        );

        return Ok(());
    }

    let mut output =
        String::new();

    if let Some(timestamp) =
        &entry.timestamp
    {
        output.push_str(
            timestamp
        );
        output.push(' ');
    }

    if let Some(level) =
        &entry.level
    {
        if options.color {
            let color =
                match level.as_str() {
                    "ERROR" =>
                        "\x1b[31m",

                    "WARN" =>
                        "\x1b[33m",

                    "INFO" =>
                        "\x1b[32m",

                    "DEBUG" =>
                        "\x1b[36m",

                    "TRACE" =>
                        "\x1b[90m",

                    _ =>
                        "\x1b[0m",
                };

            output.push_str(color);
            output.push('[');
            output.push_str(level);
            output.push(']');
            output.push_str(
                "\x1b[0m "
            );
        } else {
            output.push('[');
            output.push_str(level);
            output.push_str("] ");
        }
    }

    if let Some(module) =
        &entry.module
    {
        output.push_str(module);
        output.push_str(": ");
    }

    output.push_str(
        &entry.message
    );

    println!(
        "{}",
        output
    );

    Ok(())
}

pub fn print_error(
    entry: &LogEntry,
    options: &OutputOptions,
) -> Result<()> {
    if options.json {
        return print_entry(
            entry,
            options
        );
    }

    if options.raw {
        println!(
            "{}",
            entry.raw
        );

        return Ok(());
    }

    let mut output =
        String::new();

    if let Some(timestamp) =
        &entry.timestamp
    {
        output.push_str(
            timestamp
        );
        output.push(' ');
    }

    if options.color {
        output.push_str(
            "\x1b[31m"
        );
    }

    output.push_str(
        &entry.message
    );

    if options.color {
        output.push_str(
            "\x1b[0m"
        );
    }

    println!(
        "{}",
        output
    );

    Ok(())
}