//! Per-session environment.
//!
//! systemd starts services with an almost empty environment, so everything a
//! user session needs is rebuilt here from the system configuration files
//! written by the Senbit installer:
//!
//!   /etc/hostname         -> HOSTNAME
//!   /etc/default/locale   -> LANG / LC_ALL
//!   /etc/timezone         -> TZ
//!   /etc/senbit-release   -> SENBIT_VERSION

use std::fs;

use crate::accounts::Account;
use crate::console::{self, ColorMode};

const PATH: &str = "/usr/local/bin:/usr/bin:/bin:/usr/local/sbin:/usr/sbin:/sbin";

pub struct Session {
    pub hostname: String,
    pub lang: String,
    pub timezone: String,
    pub version: String,
}

fn read_trimmed(path: &str) -> Option<String> {
    let text = fs::read_to_string(path).ok()?;
    let text = text.trim();
    (!text.is_empty()).then(|| text.to_string())
}

/// Reads `KEY=value` (value optionally quoted) from a config file.
fn read_assignment(path: &str, key: &str) -> Option<String> {
    let prefix = format!("{key}=");

    fs::read_to_string(path).ok()?.lines().find_map(|line| {
        let value = line.trim().strip_prefix(&prefix)?;
        let value = value.trim().trim_matches('"').trim_matches('\'');
        (!value.is_empty()).then(|| value.to_string())
    })
}

impl Session {
    /// Re-read at every login so configuration changes are picked up.
    pub fn load() -> Self {
        Self {
            hostname: read_trimmed("/etc/hostname")
                .or_else(|| read_trimmed("/proc/sys/kernel/hostname"))
                .unwrap_or_else(|| "senbit".to_string()),
            lang: read_assignment("/etc/default/locale", "LANG").unwrap_or_else(|| "C".to_string()),
            timezone: read_trimmed("/etc/timezone").unwrap_or_else(|| "UTC".to_string()),
            version: read_assignment("/etc/senbit-release", "VERSION")
                .unwrap_or_else(|| "unknown".to_string()),
        }
    }

    /// Complete, explicit environment of the user's shell.
    pub fn environment(&self, account: &Account, mode: ColorMode) -> Vec<(String, String)> {
        let shell = if account.shell.is_empty() {
            "/bin/sh".to_string()
        } else {
            account.shell.clone()
        };

        let env = |k: &str, v: &str| (k.to_string(), v.to_string());

        vec![
            env("USER", &account.name),
            env("LOGNAME", &account.name),
            env("HOME", &account.home),
            env("SHELL", &shell),
            env("PATH", PATH),
            env("PWD", &account.home),
            env("OLDPWD", &account.home),
            env("TERM", console::term_for(mode)),
            env("TERMINFO", "/usr/share/terminfo"),
            env("HOSTNAME", &self.hostname),
            env("LANG", &self.lang),
            env("LC_ALL", &self.lang),
            env("TZ", &self.timezone),
            env("SENBIT_VERSION", &self.version),
            env("SENBIT_ROOT", "/"),
            env("PS1", &console::build_prompt(mode, &account.name, &self.hostname)),
        ]
    }
}
