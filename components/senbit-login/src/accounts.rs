//! User accounts (/etc/passwd + /etc/shadow).
//!
//! Only *interactive* accounts are offered at login: root and the normal
//! users (UID >= 1000) that have a real shell and a usable password. System
//! accounts created later by packages (www-data, _apt, ...) never show up.

use std::collections::HashSet;
use std::fs;

use anyhow::{Context, Result};
use crypt_sha512::Password;

const PASSWD: &str = "/etc/passwd";
const SHADOW: &str = "/etc/shadow";

/// First UID of a normal (human) user.
const FIRST_NORMAL_UID: u32 = 1000;
/// `nobody` / `nogroup`.
const NOBODY_UID: u32 = 65534;

#[derive(Debug, Clone)]
pub struct Account {
    pub name: String,
    pub uid: u32,
    pub gid: u32,
    pub home: String,
    pub shell: String,
}

fn parse_passwd_line(line: &str) -> Option<Account> {
    let line = line.trim();
    if line.is_empty() || line.starts_with('#') {
        return None;
    }

    let fields: Vec<&str> = line.split(':').collect();
    if fields.len() < 7 || fields[0].is_empty() {
        return None;
    }

    Some(Account {
        name: fields[0].to_string(),
        uid: fields[2].parse().ok()?,
        gid: fields[3].parse().ok()?,
        home: fields[5].to_string(),
        shell: fields[6].to_string(),
    })
}

fn has_login_shell(shell: &str) -> bool {
    let base = shell.rsplit('/').next().unwrap_or(shell);
    !(shell.is_empty() || base == "nologin" || base == "false")
}

/// A hash that can actually be matched: not empty, not locked ('!' / '*').
fn is_usable_hash(hash: &str) -> bool {
    !(hash.is_empty() || hash.starts_with('!') || hash.starts_with('*'))
}

fn shadow_hash(name: &str) -> Result<Option<String>> {
    let shadow = fs::read_to_string(SHADOW).with_context(|| format!("failed to read {SHADOW}"))?;

    Ok(shadow.lines().find_map(|line| {
        let mut fields = line.split(':');
        (fields.next()? == name).then(|| fields.next().unwrap_or("").to_string())
    }))
}

/// Accounts that may log in, without duplicates, root first.
pub fn login_accounts() -> Result<Vec<Account>> {
    let passwd = fs::read_to_string(PASSWD).with_context(|| format!("failed to read {PASSWD}"))?;

    let mut seen = HashSet::new();
    let mut accounts = Vec::new();

    for account in passwd.lines().filter_map(parse_passwd_line) {
        let normal = account.uid == 0
            || (account.uid >= FIRST_NORMAL_UID && account.uid != NOBODY_UID);

        if !normal || !has_login_shell(&account.shell) {
            continue;
        }

        // Same name twice in /etc/passwd: the first entry wins (that is the
        // one the C library uses too).
        if !seen.insert(account.name.clone()) {
            continue;
        }

        match shadow_hash(&account.name) {
            Ok(Some(hash)) if is_usable_hash(&hash) => accounts.push(account),
            Ok(_) => {}
            Err(error) => return Err(error),
        }
    }

    accounts.sort_by_key(|a| (a.uid != 0, a.uid));

    Ok(accounts)
}

/// Checks `password` against the account's shadow hash.
pub fn authenticate(account: &Account, password: &str) -> bool {
    let hash = match shadow_hash(&account.name) {
        Ok(Some(hash)) if is_usable_hash(&hash) => hash,
        Ok(_) => return false,
        Err(error) => {
            log_error!("{error:#}");
            return false;
        }
    };

    match crypt_sha512::verify(Password::from(password.to_string()), &hash) {
        Ok(valid) => valid,
        Err(_) => {
            log_error!("unsupported password hash for '{}'", account.name);
            false
        }
    }
}
