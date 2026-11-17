# Senbit `log`

`log` is the native Senbit userspace command used to inspect, filter, follow, manage, and rotate system logs.

It is designed as a lightweight alternative to tools such as `journalctl`, while remaining independent from systemd.

---

## Features

* Display Senbit system logs
* Follow logs in real time
* Inspect boot logs
* Inspect kernel logs
* Filter by log level
* Filter by source
* Search with plain text or regular expressions
* Exclude messages, modules, or levels
* Limit output by number of lines
* Limit the amount of data read
* Filter logs by time range
* Output logs as JSON
* Output raw log lines
* Control terminal colors
* Display log statistics
* List available log sources
* Clear logs
* Rotate logs
* Keep multiple rotated log generations

---

# Usage

```text
log [OPTIONS] [COMMAND]
```

Running `log` without a command is equivalent to:

```bash
log show
```

---

# Commands

## `log show`

Display system logs.

```bash
log show
```

Examples:

```bash
log show -n 50
```

```bash
log show --source runtime
```

```bash
log show --grep network
```

---

## `log follow`

Follow new log entries in real time.

```bash
log follow
```

The command remains active and displays new entries as they are written.

Examples:

```bash
log follow --source runtime
```

```bash
log follow --grep network
```

```bash
log follow --level error
```

---

## `log boot`

Display log entries related to system startup.

```bash
log boot
```

Example:

```bash
log boot -n 100
```

---

## `log system`

Display system logs.

```bash
log system
```

Example:

```bash
log system --grep network
```

---

## `log errors`

Display only error entries.

```bash
log errors
```

Example:

```bash
log errors -n 50
```

---

## `log warnings`

Display only warning entries.

```bash
log warnings
```

Example:

```bash
log warnings --source runtime
```

---

## `log kernel`

Display Linux kernel messages using `dmesg`.

```bash
log kernel
```

Limit the number of entries:

```bash
log kernel -n 50
```

---

## `log list`

List available log sources.

```bash
log list
```

JSON output:

```bash
log list --json
```

---

## `log stats`

Display statistics about the available logs.

```bash
log stats
```

Example output:

```text
Senbit Log Statistics

Entries: 1248
Size: 182.45 KiB

Levels:
  ERROR  4
  WARN   18
  INFO   1200
  DEBUG  26
  TRACE  0
```

JSON output:

```bash
log stats --json
```

---

# Filtering

## `--level`

Filter entries by log level.

```bash
log show --level error
```

Supported levels:

```text
error
warn
info
debug
trace
```

---

## `--source`

Select which log source to read.

```bash
log show --source runtime
```

```bash
log show --source persistent
```

```bash
log show --source all
```

Available sources:

```text
persistent
    /var/log/senbit/system.log

runtime
    /run/log/system.log
```

---

## `--grep`

Search for text inside log messages.

```bash
log show --grep network
```

Example:

```bash
log errors --grep dhcp
```

---

## `--regex`

Filter using a regular expression.

```bash
log show --regex "network|dhcp"
```

Example:

```bash
log show --regex "error|failed|failure"
```

---

## `--exclude`

Exclude messages containing specific text.

```bash
log show --exclude debug
```

Example:

```bash
log show --exclude network
```

---

## `--exclude-module`

Exclude a specific module.

```bash
log show \
    --exclude-module senbit::system::init::network
```

---

## `--exclude-level`

Exclude a specific log level.

```bash
log show --exclude-level debug
```

---

# Time filtering

## `--since`

Display entries after a specific timestamp.

```bash
log show \
    --since "2026-10-02T10:00:00"
```

---

## `--until`

Display entries before a specific timestamp.

```bash
log show \
    --until "2026-10-02T12:00:00"
```

Both options can be combined:

```bash
log show \
    --since "2026-10-02T10:00:00" \
    --until "2026-10-02T12:00:00"
```

---

# Output control

## `--lines`

Limit the number of displayed entries.

Short form:

```bash
log show -n 50
```

Long form:

```bash
log show --lines 50
```

---

## `--max-size`

Limit the amount of log data read.

```bash
log show --max-size 10K
```

Supported units:

```text
B
K
KB
M
MB
G
GB
T
TB
```

Examples:

```bash
log show --max-size 512B
```

```bash
log show --max-size 10MB
```

---

## `--json`

Output structured JSON.

```bash
log show --json
```

Useful for scripts and other programs.

Example:

```bash
log stats --json
```

---

## `--raw`

Display the original log lines without additional formatting.

```bash
log show --raw
```

---

## `--color`

Force colored output.

```bash
log show --color
```

---

## `--no-color`

Disable colored output.

```bash
log show --no-color
```

---

## `--quiet`

Reduce additional command output.

```bash
log rotate --quiet
```

Short form:

```bash
log rotate -q
```

---

## `--verbose`

Enable additional information.

```bash
log show --verbose
```

Short form:

```bash
log show -v
```

---

# Log management

## `log clear`

Clear selected log files.

### Runtime logs

```bash
sudo log clear --runtime
```

### Persistent logs

```bash
sudo log clear --persistent
```

### All logs

```bash
sudo log clear --all
```

The command asks for confirmation before clearing logs.

Use `--force` to skip confirmation:

```bash
sudo log clear --runtime --force
```

or:

```bash
sudo log clear --runtime -f
```

> Clearing logs is destructive. Use `--force` carefully.

---

# Log rotation

## `log rotate`

Rotate log files.

```bash
sudo log rotate
```

### Runtime logs

```bash
sudo log rotate --runtime
```

### Persistent logs

```bash
sudo log rotate --persistent
```

### All logs

```bash
sudo log rotate --all
```

---

## `--keep`

Specify how many rotated generations should be kept.

```bash
sudo log rotate --keep 5
```

Example:

```text
system.log
system.log.1
system.log.2
system.log.3
system.log.4
system.log.5
```

---

## `--max-size`

Only rotate logs when they reach the specified size.

```bash
sudo log rotate --max-size 100MB
```

---

## `--force`

Force rotation even when the configured size threshold has not been reached.

```bash
sudo log rotate --force
```

---

# Common examples

## Show the latest 50 entries

```bash
log show -n 50
```

## Show only errors

```bash
log errors
```

## Show network-related errors

```bash
log errors --grep network
```

## Follow network logs

```bash
log follow --grep network
```

## Show runtime logs

```bash
log show --source runtime
```

## Show only persistent logs

```bash
log show --source persistent
```

## Search for DHCP activity

```bash
log show --grep dhcp
```

## Search using a regular expression

```bash
log show --regex "network|dhcp|eth0"
```

## Show the last 100 boot entries

```bash
log boot -n 100
```

## Inspect kernel messages

```bash
log kernel -n 100
```

## Export statistics as JSON

```bash
log stats --json
```

## Follow errors in real time

```bash
log follow --level error
```

## Exclude debug messages

```bash
log show --exclude-level debug
```

## Inspect logs from a specific period

```bash
log show \
    --since "2026-10-02T10:00:00" \
    --until "2026-10-02T12:00:00"
```

---

# Options reference

| Option                      | Description                           |
| --------------------------- | ------------------------------------- |
| `-n, --lines <N>`           | Limit the number of displayed entries |
| `--max-size <SIZE>`         | Limit the amount of data read         |
| `--since <DATE>`            | Show entries after a timestamp        |
| `--until <DATE>`            | Show entries before a timestamp       |
| `--level <LEVEL>`           | Filter by log level                   |
| `--source <SOURCE>`         | Select a log source                   |
| `--grep <TEXT>`             | Search for text                       |
| `--regex <REGEX>`           | Search using a regular expression     |
| `--exclude <TEXT>`          | Exclude matching text                 |
| `--exclude-module <MODULE>` | Exclude a module                      |
| `--exclude-level <LEVEL>`   | Exclude a log level                   |
| `--json`                    | Output JSON                           |
| `--raw`                     | Output raw log lines                  |
| `--color`                   | Force colors                          |
| `--no-color`                | Disable colors                        |
| `-q, --quiet`               | Reduce additional output              |
| `-v, --verbose`             | Enable verbose output                 |

---

# Clear options

| Option         | Description           |
| -------------- | --------------------- |
| `--runtime`    | Clear runtime logs    |
| `--persistent` | Clear persistent logs |
| `--all`        | Clear all logs        |
| `-f, --force`  | Skip confirmation     |

---

# Rotate options

| Option              | Description                           |
| ------------------- | ------------------------------------- |
| `--runtime`         | Rotate runtime logs                   |
| `--persistent`      | Rotate persistent logs                |
| `--all`             | Rotate all logs                       |
| `--keep <N>`        | Number of rotated generations to keep |
| `--max-size <SIZE>` | Minimum size required for rotation    |
| `--force`           | Force rotation                        |
| `-q, --quiet`       | Reduce output                         |
| `-v, --verbose`     | Show additional information           |

---

# Log sources

Senbit currently uses two main log files.

### Persistent

```text
/var/log/senbit/system.log
```

Persistent logs survive reboots.

### Runtime

```text
/run/log/system.log
```

Runtime logs are stored in `/run` and are intended for the current boot/session.

---

# Design

The `log` command is designed to work with the native Senbit logging system.

```text
Senbit processes
       │
       ▼
  Senbit logger
       │
       ├───────────────┐
       ▼               ▼
/var/log/senbit/    /run/log/
 system.log         system.log
       │               │
       └───────┬───────┘
               ▼
             log
               │
      ┌────────┼─────────┐
      ▼        ▼         ▼
    show     follow    stats
```

The command does not require `systemd` or `journald`.

---

# Security

Operations that modify persistent system logs generally require root privileges.

Examples:

```bash
sudo log clear --persistent
```

```bash
sudo log rotate --persistent
```

Reading logs does not normally require root unless filesystem permissions restrict access.

---

# Status

`log` is part of the native Senbit userspace command system.

It is built and installed automatically by `cmdtool` according to its Senbit command metadata.
