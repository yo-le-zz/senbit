//! System API socket (/run/senbit.sock). The protocol is described in docs/API.md.
//!
//! A client connects, sends one request word and reads the answer: "ok" (possibly followed
//! by a value) or "error: ...". The socket belongs to root (mode 0600).
//!
//! Requests: `reboot`, `shutdown`, `halt`, `poweroff` (power transitions, handled by
//! systemd), `panic` (a real kernel panic, for tests), `ping` and `version`.

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

/// What a client can ask for.
enum Request {
    Power(PowerAction),
    Panic,
    Ping,
    Version,
}

fn parse_request(word: &str) -> Result<Request> {
    match word.trim() {
        "shutdown" => Ok(Request::Power(PowerAction::Shutdown)),
        "reboot" => Ok(Request::Power(PowerAction::Reboot)),
        "halt" => Ok(Request::Power(PowerAction::Halt)),
        "poweroff" => Ok(Request::Power(PowerAction::Poweroff)),
        "panic" => Ok(Request::Panic),
        "ping" => Ok(Request::Ping),
        "version" => Ok(Request::Version),
        other => bail!("unknown system request '{other}'"),
    }
}

/// The Senbit version written by the senbit-userspace package in /etc/senbit-release
/// (`VERSION=0.4.0`), or "unknown".
fn system_version() -> String {
    fs::read_to_string("/etc/senbit-release")
        .ok()
        .and_then(|text| {
            text.lines()
                .find_map(|line| line.strip_prefix("VERSION=").map(|v| v.trim().to_string()))
        })
        .unwrap_or_else(|| "unknown".to_string())
}

fn handle_client(mut stream: UnixStream) -> Result<()> {
    // A client that connects and never writes must not block the listener.
    stream.set_read_timeout(Some(Duration::from_secs(5)))?;

    let mut buffer = [0u8; 128];
    let size = stream.read(&mut buffer).context("failed to read the request")?;

    if size == 0 {
        return Ok(());
    }

    let text = std::str::from_utf8(&buffer[..size]).context("request is not valid UTF-8")?;

    let request = match parse_request(text) {
        Ok(request) => request,
        Err(error) => {
            let _ = stream.write_all(format!("error: {error:#}\n").as_bytes());
            return Err(error);
        }
    };

    // The system is about to crash: answer first and close the connection, then crash.
    if let Request::Panic = request {
        stream.write_all(b"ok\n")?;
        let _ = stream.flush();
        drop(stream);
        return power::panic();
    }

    let outcome: Result<String> = match request {
        Request::Power(action) => power::request(action).map(|()| "ok".to_string()),
        Request::Ping => Ok("ok pong".to_string()),
        Request::Version => Ok(format!("ok {}", system_version())),
        Request::Panic => unreachable!("handled above"),
    };

    match outcome {
        Ok(answer) => {
            stream.write_all(format!("{answer}\n").as_bytes())?;
            Ok(())
        }
        Err(error) => {
            let _ = stream.write_all(format!("error: {error:#}\n").as_bytes());
            Err(error)
        }
    }
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
