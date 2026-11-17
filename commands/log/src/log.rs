use anyhow::{
    bail,
    Context,
    Result,
};

use std::fs::{
    self,
    File,
    OpenOptions,
};

use std::io::{
    BufRead,
    BufReader,
    Read,
    Seek,
    SeekFrom,
    Write,
};

use std::path::{
    Path,
    PathBuf,
};

use std::process::Command;

use std::thread;

use std::time::Duration;

use crate::cli::{
    ClearArgs,
    Cli,
    Command as LogCommand,
    KernelArgs,
    Level,
    RotateArgs,
};

use crate::filter::Filter;

use crate::output::{
    print_entry,
    OutputOptions,
};

use crate::parser::{
    parse_line,
    LogEntry,
};

use crate::source::{
    sources,
    LogSource,
};

pub fn run(
    cli: Cli,
) -> Result<()> {
    let command =
        cli.command
            .as_ref()
            .unwrap_or(
                &LogCommand::Show
            );

    match command {
        LogCommand::Show =>
            show(&cli),

        LogCommand::Follow =>
            follow(&cli),

        LogCommand::Boot =>
            show_boot(&cli),

        LogCommand::System =>
            show_system(&cli),

        LogCommand::Errors =>
            show_level(
                &cli,
                Level::Error,
            ),

        LogCommand::Warnings =>
            show_level(
                &cli,
                Level::Warn,
            ),

        LogCommand::Kernel(args) =>
            show_kernel(
                &cli,
                args,
            ),

        LogCommand::List =>
            list_sources(&cli),

        LogCommand::Stats =>
            stats(&cli),

        LogCommand::Clear(args) =>
            clear(
                &cli,
                args,
            ),

        LogCommand::Rotate(args) =>
            rotate(
                &cli,
                args,
            ),
    }
}

fn output_options(
    cli: &Cli,
) -> OutputOptions {
    let color =
        if cli.no_color {
            false
        } else if cli.color {
            true
        } else {
            std::env::var(
                "NO_COLOR"
            ).is_err()
                && std::env::var(
                    "TERM",
                )
                .map(|term| {
                    term != "dumb"
                })
                .unwrap_or(true)
        };

    OutputOptions {
        json: cli.json,
        raw: cli.raw,
        color,
    }
}

fn show(
    cli: &Cli,
) -> Result<()> {
    let filter =
        Filter::from_cli(cli)?;

    let options =
        output_options(cli);

    let limit =
        cli.lines
            .unwrap_or(usize::MAX);

    let max_size =
        match &cli.max_size {
            Some(value) =>
                Some(parse_size(value)?),

            None =>
                None,
        };

    let mut displayed =
        0usize;

    for source in
        sources(cli.source)
    {
        if !source.exists() {
            continue;
        }

        if let Some(max) =
            max_size
        {
            if source.size() > max {
                if cli.verbose {
                    eprintln!(
                        "log: reading last {} bytes from {}",
                        max,
                        source.path.display(),
                    );
                }

                let lines =
                    read_last_bytes(
                        &source.path,
                        max,
                    )?;

                for line in
                    lines
                {
                    if displayed >= limit {
                        return Ok(());
                    }

                    let entry =
                        parse_line(
                            &line,
                        );

                    if !filter.matches(
                        &entry,
                    ) {
                        continue;
                    }

                    print_entry(
                        &entry,
                        &options,
                    )?;

                    displayed += 1;
                }

                continue;
            }
        }

        let file =
            File::open(
                &source.path,
            )
            .with_context(|| {
                format!(
                    "failed to open {}",
                    source.path.display(),
                )
            })?;

        let reader =
            BufReader::new(file);

        for line in
            reader.lines()
        {
            if displayed >= limit {
                return Ok(());
            }

            let line =
                line.with_context(
                    || {
                        format!(
                            "failed to read {}",
                            source.path.display(),
                        )
                    },
                )?;

            let entry =
                parse_line(
                    &line,
                );

            if !filter.matches(
                &entry,
            ) {
                continue;
            }

            print_entry(
                &entry,
                &options,
            )?;

            displayed += 1;
        }
    }

    Ok(())
}

fn show_system(
    cli: &Cli,
) -> Result<()> {
    show(cli)
}

fn show_boot(
    cli: &Cli,
) -> Result<()> {
    let filter =
        Filter::from_cli(cli)?;

    let options =
        output_options(cli);

    let limit =
        cli.lines
            .unwrap_or(usize::MAX);

    let mut displayed =
        0usize;

    for source in
        sources(cli.source)
    {
        if !source.exists() {
            continue;
        }

        let file =
            File::open(
                &source.path,
            )?;

        let reader =
            BufReader::new(file);

        for line in
            reader.lines()
        {
            if displayed >= limit {
                return Ok(());
            }

            let line =
                line?;

            if !is_boot_entry(
                &line,
            ) {
                continue;
            }

            let entry =
                parse_line(
                    &line,
                );

            if !filter.matches(
                &entry,
            ) {
                continue;
            }

            print_entry(
                &entry,
                &options,
            )?;

            displayed += 1;
        }
    }

    Ok(())
}

fn show_level(
    cli: &Cli,
    level: Level,
) -> Result<()> {
    let filter =
        Filter::from_cli(cli)?;

    let options =
        output_options(cli);

    let limit =
        cli.lines
            .unwrap_or(usize::MAX);

    let mut displayed =
        0usize;

    for source in
        sources(cli.source)
    {
        if !source.exists() {
            continue;
        }

        let file =
            File::open(
                &source.path,
            )
            .with_context(|| {
                format!(
                    "failed to open {}",
                    source.path.display(),
                )
            })?;

        let reader =
            BufReader::new(file);

        for line in
            reader.lines()
        {
            if displayed >= limit {
                return Ok(());
            }

            let line =
                line.with_context(
                    || {
                        format!(
                            "failed to read {}",
                            source.path.display(),
                        )
                    },
                )?;

            let entry =
                parse_line(
                    &line,
                );

            if entry.level_enum()
                != Some(level)
            {
                continue;
            }

            if !filter.matches(
                &entry,
            ) {
                continue;
            }

            print_entry(
                &entry,
                &options,
            )?;

            displayed += 1;
        }
    }

    Ok(())
}

fn follow(
    cli: &Cli,
) -> Result<()> {
    let filter =
        Filter::from_cli(cli)?;

    let options =
        output_options(cli);

    let limit =
        cli.lines
            .unwrap_or(usize::MAX);

    let max_size =
        match &cli.max_size {
            Some(value) =>
                Some(parse_size(value)?),

            None =>
                None,
        };

    let selected =
        sources(cli.source);

    let mut states =
        Vec::new();

    for source in
        selected
    {
        if !source.exists() {
            continue;
        }

        let mut file =
            OpenOptions::new()
                .read(true)
                .open(
                    &source.path,
                )?;

        if let Some(max) =
            max_size
        {
            let metadata =
                file.metadata()?;

            let position =
                metadata
                    .len()
                    .saturating_sub(
                        max,
                    );

            file.seek(
                SeekFrom::Start(
                    position,
                ),
            )?;
        } else {
            file.seek(
                SeekFrom::End(0),
            )?;
        }

        states.push(
            FollowState {
                path: source.path,
                file,
                buffer: String::new(),
            },
        );
    }

    let mut displayed =
        0usize;

    loop {
        let mut activity =
            false;

        for state in
            &mut states
        {
            let mut buffer =
                [0u8; 4096];

            let size =
                state.file.read(
                    &mut buffer,
                )?;

            if size == 0 {
                continue;
            }

            activity = true;

            state.buffer
                .push_str(
                    &String::from_utf8_lossy(
                        &buffer[..size],
                    ),
                );

            while let Some(
                newline,
            ) =
                state.buffer.find('\n')
            {
                let line =
                    state.buffer
                        [..newline]
                        .trim_end_matches(
                            '\r',
                        )
                        .to_string();

                state.buffer =
                    state.buffer
                        [newline + 1..]
                        .to_string();

                let entry =
                    parse_line(
                        &line,
                    );

                if !filter.matches(
                    &entry,
                ) {
                    continue;
                }

                print_entry(
                    &entry,
                    &options,
                )?;

                displayed += 1;

                if displayed >= limit {
                    return Ok(());
                }
            }
        }

        if !activity {
            thread::sleep(
                Duration::from_millis(
                    100,
                ),
            );
        }
    }
}

struct FollowState {
    #[allow(dead_code)]
    path: PathBuf,
    file: File,
    buffer: String,
}

fn show_kernel(
    cli: &Cli,
    args: &KernelArgs,
) -> Result<()> {
    let mut command =
        Command::new("dmesg");

    command.arg(
        "--color=never",
    );

    command.arg(
        "--nopager",
    );

    let output =
        command.output()
            .context(
                "failed to execute dmesg",
            )?;

    if !output.status.success() {
        bail!(
            "dmesg exited with status {}",
            output.status,
        );
    }

    let text =
        String::from_utf8_lossy(
            &output.stdout,
        );

    let requested_lines =
        args.lines
            .or(cli.lines);

    let lines =
        match requested_lines {
            Some(limit) =>
                text.lines()
                    .rev()
                    .take(limit)
                    .collect::<Vec<_>>()
                    .into_iter()
                    .rev()
                    .collect::<Vec<_>>(),

            None =>
                text.lines()
                    .collect::<Vec<_>>(),
        };

    let options =
        output_options(cli);

    for line in
        lines
    {
        let entry =
            LogEntry {
                timestamp: None,
                level: None,
                module: None,
                message:
                    line.to_string(),
                raw:
                    line.to_string(),
            };

        print_entry(
            &entry,
            &options,
        )?;
    }

    Ok(())
}

fn list_sources(
    cli: &Cli,
) -> Result<()> {
    if cli.json {
        let values =
            sources(None)
                .into_iter()
                .map(|source| {
                    serde_json::json!({
                        "name": source.name,
                        "path": source.path,
                        "persistent": source.persistent,
                        "exists": source.exists(),
                        "size": source.size(),
                    })
                })
                .collect::<Vec<_>>();

        println!(
            "{}",
            serde_json::to_string_pretty(
                &values,
            )?,
        );

        return Ok(());
    }

    println!(
        "Senbit log sources:",
    );

    for source in
        sources(None)
    {
        let status =
            if source.exists() {
                "available"
            } else {
                "missing"
            };

        println!(
            "  {:<12} {:<10} {} ({} bytes)",
            source.name,
            status,
            source.path.display(),
            source.size(),
        );
    }

    Ok(())
}

fn stats(
    cli: &Cli,
) -> Result<()> {
    let mut total_entries =
        0u64;

    let mut total_bytes =
        0u64;

    let mut errors =
        0u64;

    let mut warnings =
        0u64;

    let mut infos =
        0u64;

    let mut debug =
        0u64;

    let mut trace =
        0u64;

    for source in
        sources(cli.source)
    {
        if !source.exists() {
            continue;
        }

        total_bytes +=
            source.size();

        let file =
            File::open(
                &source.path,
            )?;

        for line in
            BufReader::new(
                file,
            ).lines()
        {
            let entry =
                parse_line(
                    &line?,
                );

            total_entries += 1;

            match entry.level_enum()
            {
                Some(Level::Error) =>
                    errors += 1,

                Some(Level::Warn) =>
                    warnings += 1,

                Some(Level::Info) =>
                    infos += 1,

                Some(Level::Debug) =>
                    debug += 1,

                Some(Level::Trace) =>
                    trace += 1,

                None => {}
            }
        }
    }

    if cli.json {
        let value =
            serde_json::json!({
                "entries": total_entries,
                "bytes": total_bytes,
                "error": errors,
                "warn": warnings,
                "info": infos,
                "debug": debug,
                "trace": trace,
            });

        println!(
            "{}",
            serde_json::to_string_pretty(
                &value,
            )?,
        );

        return Ok(());
    }

    println!(
        "Senbit Log Statistics",
    );

    println!();

    println!(
        "Entries: {}",
        total_entries,
    );

    println!(
        "Size: {}",
        format_size(
            total_bytes,
        ),
    );

    println!();

    println!(
        "Levels:",
    );

    println!(
        "  ERROR  {}",
        errors,
    );

    println!(
        "  WARN   {}",
        warnings,
    );

    println!(
        "  INFO   {}",
        infos,
    );

    println!(
        "  DEBUG  {}",
        debug,
    );

    println!(
        "  TRACE  {}",
        trace,
    );

    Ok(())
}

fn clear(
    cli: &Cli,
    args: &ClearArgs,
) -> Result<()> {
    let clear_all =
        args.all
            || (!args.runtime
                && !args.persistent);

    let mut targets =
        Vec::new();

    for source in
        sources(None)
    {
        if !source.exists() {
            continue;
        }

        if clear_all
            || (args.runtime
                && !source.persistent)
            || (args.persistent
                && source.persistent)
        {
            targets.push(
                source,
            );
        }
    }

    if targets.is_empty() {
        if !cli.quiet {
            println!(
                "No log files selected.",
            );
        }

        return Ok(());
    }

    if !args.force {
        println!(
            "The following log files will be cleared:",
        );

        for target in
            &targets
        {
            println!(
                "  {}",
                target.path.display(),
            );
        }

        print!(
            "Continue? [y/N] ",
        );

        std::io::stdout()
            .flush()?;

        let mut answer =
            String::new();

        std::io::stdin()
            .read_line(
                &mut answer,
            )?;

        if !matches!(
            answer.trim(),
            "y" | "Y" | "yes" | "YES"
        ) {
            println!(
                "Cancelled.",
            );

            return Ok(());
        }
    }

    for target in
        targets
    {
        clear_file(
            &target.path,
        )?;

        if !cli.quiet {
            println!(
                "Cleared {}",
                target.path.display(),
            );
        }
    }

    Ok(())
}

fn clear_file(
    path: &Path,
) -> Result<()> {
    OpenOptions::new()
        .write(true)
        .truncate(true)
        .open(path)
        .with_context(|| {
            format!(
                "failed to clear {}",
                path.display(),
            )
        })?;

    Ok(())
}

fn rotate(
    cli: &Cli,
    args: &RotateArgs,
) -> Result<()> {
    if args.keep == 0 {
        bail!(
            "--keep must be greater than zero",
        );
    }

    let max_size =
        match &args.max_size {
            Some(value) =>
                Some(parse_size(
                    value,
                )?),
    
            None =>
                None,
        };

    let all =
        args.all
            || (!args.runtime
                && !args.persistent);

    for source in
        sources(None)
    {
        if !source.exists() {
            continue;
        }

        if !all
            && !(
                (args.runtime
                    && !source.persistent)
                    ||
                (args.persistent
                    && source.persistent)
            )
        {
            continue;
        }

        if let Some(max) =
            max_size
        {
            if source.size() < max
                && !args.force
            {
                if cli.verbose {
                    println!(
                        "Skipping {}: {} < {}",
                        source.path.display(),
                        format_size(
                            source.size(),
                        ),
                        format_size(
                            max,
                        ),
                    );
                }

                continue;
            }
        }

        rotate_file(
            &source,
            args.keep,
        )?;

        if !cli.quiet {
            println!(
                "Rotated {}",
                source.path.display(),
            );
        }
    }

    Ok(())
}

fn rotate_file(
    source: &LogSource,
    keep: usize,
) -> Result<()> {
    for index in
        (1..=keep).rev()
    {
        let old =
            rotated_path(
                &source.path,
                index,
            );

        let new =
            rotated_path(
                &source.path,
                index + 1,
            );

        if old.exists() {
            if index == keep {
                fs::remove_file(
                    &old,
                )
                .with_context(
                    || {
                        format!(
                            "failed to remove {}",
                            old.display(),
                        )
                    },
                )?;
            } else {
                fs::rename(
                    &old,
                    &new,
                )
                .with_context(
                    || {
                        format!(
                            "failed to rotate {} to {}",
                            old.display(),
                            new.display(),
                        )
                    },
                )?;
            }
        }
    }

    let first =
        rotated_path(
            &source.path,
            1,
        );

    fs::rename(
        &source.path,
        &first,
    )
    .with_context(
        || {
            format!(
                "failed to rotate {}",
                source.path.display(),
            )
        },
    )?;

    File::create(
        &source.path,
    )
    .with_context(
        || {
            format!(
                "failed to recreate {}",
                source.path.display(),
            )
        },
    )?;

    Ok(())
}

fn rotated_path(
    path: &Path,
    index: usize,
) -> PathBuf {
    PathBuf::from(
        format!(
            "{}.{}",
            path.display(),
            index,
        ),
    )
}

fn read_last_bytes(
    path: &Path,
    max_size: u64,
) -> Result<Vec<String>> {
    let mut file =
        File::open(path)?;

    let length =
        file.metadata()?.len();

    let start =
        length.saturating_sub(
            max_size,
        );

    file.seek(
        SeekFrom::Start(
            start,
        ),
    )?;

    let mut buffer =
        String::new();

    file.read_to_string(
        &mut buffer,
    )?;

    if start > 0 {
        if let Some(index) =
            buffer.find('\n')
        {
            buffer =
                buffer[index + 1..]
                    .to_string();
        }
    }

    Ok(
        buffer
            .lines()
            .map(
                |line| line.to_string()
            )
            .collect(),
    )
}

fn parse_size(
    value: &str,
) -> Result<u64> {
    let value =
        value.trim();

    if value.is_empty() {
        bail!(
            "size cannot be empty",
        );
    }

    let mut number =
        String::new();

    let mut unit =
        String::new();

    for character in
        value.chars()
    {
        if character.is_ascii_digit()
            || character == '.'
        {
            if !unit.is_empty() {
                bail!(
                    "invalid size '{}'",
                    value,
                );
            }

            number.push(
                character,
            );
        } else if character
            .is_ascii_alphabetic()
        {
            unit.push(
                character,
            );
        } else {
            bail!(
                "invalid size '{}'",
                value,
            );
        }
    }

    let number =
        number
            .parse::<f64>()
            .with_context(
                || {
                    format!(
                        "invalid size '{}'",
                        value,
                    )
                },
            )?;

    let multiplier =
        match unit
            .to_ascii_lowercase()
            .as_str()
        {
            "" | "b" =>
                1f64,

            "k" | "kb" =>
                1024f64,

            "m" | "mb" =>
                1024f64
                    * 1024f64,

            "g" | "gb" =>
                1024f64
                    * 1024f64
                    * 1024f64,

            "t" | "tb" =>
                1024f64
                    * 1024f64
                    * 1024f64
                    * 1024f64,

            _ => {
                bail!(
                    "unknown size unit '{}'",
                    unit,
                );
            }
        };

    let result =
        number * multiplier;

    if !result.is_finite()
        || result < 0.0
    {
        bail!(
            "invalid size '{}'",
            value,
        );
    }

    Ok(result as u64)
}

fn format_size(
    size: u64,
) -> String {
    if size >=
        1024 * 1024 * 1024
    {
        format!(
            "{:.2} GiB",
            size as f64
                / (
                    1024f64
                        * 1024f64
                        * 1024f64
                ),
        )
    } else if size >=
        1024 * 1024
    {
        format!(
            "{:.2} MiB",
            size as f64
                / (
                    1024f64
                        * 1024f64
                ),
        )
    } else if size >= 1024 {
        format!(
            "{:.2} KiB",
            size as f64
                / 1024f64,
        )
    } else {
        format!(
            "{} B",
            size,
        )
    }
}

fn is_boot_entry(
    line: &str,
) -> bool {
    let lower =
        line.to_lowercase();

    lower.contains("senbit")
        || lower.contains("init")
        || lower.contains("mount")
        || lower.contains("boot")
        || lower.contains("network")
        || lower.contains("hostname")
        || lower.contains("rootfs")
}