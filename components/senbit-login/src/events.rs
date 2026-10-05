//! System event socket (/run/senbit.sock).
//!
//! The Senbit commands (reboot, shutdown, halt, poweroff) connect here and
//! send one word. The protocol is unchanged: the request is a single word,
//! the answer is "ok\n" (or "error: ...\n").

use std::fs;
use std::io::{Read, Write};
use std::os::unix::fs::{FileTypeExt, PermissionsExt};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::Path;
use std::thread;
use std::time::Duration;

use anyhow::{bail, Context, Result};

use crate::power::{self, PowerAction};

pub const SOCKET_PATH: &str = "/run/senbit.sock";

// ============================================================
// Setup
// ============================================================

pub fn bind() -> Result<UnixListener> {
    let path = Path::new(SOCKET_PATH);

    fs::create_dir_all("/run").context("failed to create /run")?;

    // Remove a stale socket left by a previous run of this service. Never
    // follow symlinks and never delete anything that is not a socket.
    match fs::symlink_metadata(path) {
        Ok(metadata) => {
            if metadata.file_type().is_symlink() {
                bail!("{SOCKET_PATH} is a symbolic link");
            }
            if !metadata.file_type().is_socket() {
                bail!("{SOCKET_PATH} exists and is not a socket");
            }
            fs::remove_file(path).context("failed to remove the stale socket")?;
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error).with_context(|| format!("failed to inspect {SOCKET_PATH}")),
    }

    let listener = UnixListener::bind(path).context("failed to bind the Unix socket")?;

    // root only.
    fs::set_permissions(path, fs::Permissions::from_mode(0o600))
        .context("failed to set the socket permissions")?;

    log_info!("system event socket ready: {SOCKET_PATH}");

    Ok(listener)
}

// ============================================================
// Requests
// ============================================================

fn parse_action(word: &str) -> Result<PowerAction> {
    match word.trim() {
        "shutdown" => Ok(PowerAction::Shutdown),
        "reboot" => Ok(PowerAction::Reboot),
        "halt" => Ok(PowerAction::Halt),
        "poweroff" => Ok(PowerAction::Poweroff),
        other => bail!("unknown system event '{other}'"),
    }
}

fn handle_client(mut stream: UnixStream) -> Result<()> {
    // A client that connects and never writes must not block the listener.
    stream.set_read_timeout(Some(Duration::from_secs(5)))?;

    let mut buffer = [0u8; 128];
    let size = stream.read(&mut buffer).context("failed to read the request")?;

    if size == 0 {
        return Ok(());
    }

    let request = std::str::from_utf8(&buffer[..size]).context("request is not valid UTF-8")?;

    let outcome = parse_action(request).and_then(power::request);

    match &outcome {
        Ok(()) => stream.write_all(b"ok\n")?,
        Err(error) => {
            let _ = stream.write_all(format!("error: {error:#}\n").as_bytes());
        }
    }

    outcome
}

/// Serves requests until the process exits.
pub fn serve(listener: UnixListener) {
    log_info!("system event listener started");

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                if let Err(error) = handle_client(stream) {
                    log_error!("system event failed: {error:#}");
                }
            }
            Err(error) => {
                log_error!("socket accept failed: {error}");
                // Do not spin if accept keeps failing (e.g. EMFILE).
                thread::sleep(Duration::from_millis(200));
            }
        }
    }
}
