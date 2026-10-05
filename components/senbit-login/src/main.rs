//! senbit-login — Senbit console login service.
//!
//! Started by systemd (`senbit-login.service`) once the system is up. It is
//! fully independent from `senbit-init`, which only prepares the system and
//! then replaces itself with systemd (PID 1).
//!
//! What this program does:
//!   1. creates the system event socket (/run/senbit.sock) and serves the
//!      power requests (reboot / shutdown / halt / poweroff) sent by the
//!      Senbit commands, forwarding them to systemd;
//!   2. runs the login loop on the console (stdin/stdout are the console,
//!      set up by systemd: StandardInput=tty-force, TTYPath=/dev/console).
//!
//! Any fatal error makes the process exit with a non-zero status; systemd
//! restarts the service (Restart=always), so the console never stays dead.

#[macro_use]
mod log;

mod accounts;
mod console;
mod events;
mod login;
mod power;
mod session;
mod shell;

use anyhow::{bail, Context, Result};

fn run() -> Result<()> {
    if unsafe { libc::geteuid() } != 0 {
        bail!("senbit-login must run as root");
    }

    // systemd gives us /dev/console as controlling terminal; make sure of it
    // (best effort) so that Ctrl+C / job control work in the login prompt.
    console::acquire_controlling_tty();

    let listener = events::bind().context("failed to create the system event socket")?;

    std::thread::Builder::new()
        .name("senbit-events".into())
        .spawn(move || events::serve(listener))
        .context("failed to start the system event listener")?;

    login::run()
}

fn main() {
    log_info!("starting (pid {})", std::process::id());

    if let Err(error) = run() {
        log_error!("fatal: {error:#}");
        std::process::exit(1);
    }
}
