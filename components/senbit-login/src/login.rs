//! Interactive login loop on the console.

use std::io::{self, Write};
use std::os::fd::AsRawFd;
use std::thread;
use std::time::Duration;

use anyhow::{bail, Result};
use colored::Colorize;
use inquire::{InquireError, Select};

use crate::accounts::{self, Account};
use crate::console;
use crate::session::Session;
use crate::shell;

/// Pause after a failed attempt (slows down guessing).
const FAILURE_DELAY: Duration = Duration::from_secs(2);

// ============================================================
// Password input
// ============================================================

fn read_password() -> Result<String> {
    let fd = io::stdin().as_raw_fd();

    let mut original = unsafe { std::mem::zeroed::<libc::termios>() };
    if unsafe { libc::tcgetattr(fd, &mut original) } != 0 {
        bail!("failed to read terminal settings (is stdin the console?)");
    }

    let mut hidden = original;
    hidden.c_lflag &= !libc::ECHO;
    if unsafe { libc::tcsetattr(fd, libc::TCSANOW, &hidden) } != 0 {
        bail!("failed to disable password echo");
    }

    print!("Password: ");
    io::stdout().flush()?;

    let mut password = String::new();
    let read = io::stdin().read_line(&mut password);

    // Always restore the terminal, even if the read failed.
    unsafe {
        libc::tcsetattr(fd, libc::TCSANOW, &original);
    }
    println!();

    if read? == 0 {
        bail!("console closed while reading the password");
    }

    while password.ends_with(['\n', '\r']) {
        password.pop();
    }

    Ok(password)
}

// ============================================================
// Login loop
// ============================================================

fn pick_account(mut accounts: Vec<Account>) -> Result<Option<Account>> {
    let names: Vec<String> = accounts.iter().map(|a| a.name.clone()).collect();

    match Select::new("Select user:", names).prompt() {
        Ok(name) => Ok(accounts
            .iter()
            .position(|a| a.name == name)
            .map(|index| accounts.swap_remove(index))),

        // Ctrl+C / Esc at the prompt: just show the menu again.
        Err(InquireError::OperationCanceled | InquireError::OperationInterrupted) => Ok(None),

        Err(error) => Err(error.into()),
    }
}

fn failed(message: &str) {
    println!();
    println!("{}", message.red().bold());
    thread::sleep(FAILURE_DELAY);
}

/// Runs forever. Only returns on an unrecoverable error (the service is then
/// restarted by systemd).
pub fn run() -> Result<()> {
    loop {
        console::acquire_controlling_tty();
        console::clear();

        let accounts = accounts::login_accounts()?;
        if accounts.is_empty() {
            bail!("no account is allowed to log in (check /etc/passwd and /etc/shadow)");
        }

        let Some(account) = pick_account(accounts)? else {
            continue;
        };

        let password = read_password()?;

        if !accounts::authenticate(&account, &password) {
            log_warn!("failed login for '{}'", account.name);
            failed("Invalid password.");
            continue;
        }

        log_info!("'{}' logged in", account.name);

        // Configuration may have changed since the last login.
        let session = Session::load();
        let mode = console::prepare();

        console::clear();
        println!("{}", format!("Welcome to Senbit {}", session.version).cyan().bold());
        println!("{}", format!("Hello, {}!", account.name).green().bold());

        let environment = session.environment(&account, mode);

        if let Err(error) = shell::run(&account, &environment) {
            log_error!("{error:#}");
            failed(&format!("Failed to start the shell: {error:#}"));
        }
    }
}
