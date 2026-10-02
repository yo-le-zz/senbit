use anyhow::{
    Context,
    Result,
};

use std::fs;
use std::io::{
    Read,
    Write,
};
use std::os::unix::fs::{
    FileTypeExt,
    PermissionsExt,
};
use std::os::unix::net::{
    UnixListener,
    UnixStream,
};
use std::path::Path;

use colored::Colorize;

use crate::logln;

use crate::system::handler::main::{
    handle,
    SystemEvent,
};

pub const SOCKET_PATH: &str =
    "/run/senbit.sock";

/* ============================================================
   Socket setup
============================================================ */

pub fn setup_socket() -> Result<UnixListener> {
    let socket_path =
        Path::new(SOCKET_PATH);

    let run_dir =
        Path::new("/run");

    /*
     * ========================================================
     * /run
     * ========================================================
     */

    if !run_dir.exists() {
        fs::create_dir_all(run_dir)
            .context(
                "failed to create /run"
            )?;
    }

    fs::set_permissions(
        run_dir,
        fs::Permissions::from_mode(0o755),
    )
    .context(
        "failed to set permissions on /run"
    )?;

    /*
     * ========================================================
     * Remove stale socket
     * ========================================================
     */

    match fs::symlink_metadata(socket_path) {
        Ok(metadata) => {
            /*
             * Never follow symlinks.
             *
             * A symlink at the socket path is rejected
             * instead of being followed or deleted.
             */

            if metadata.file_type().is_symlink() {
                anyhow::bail!(
                    "{} is a symbolic link",
                    SOCKET_PATH
                );
            }

            if !metadata.file_type().is_socket() {
                anyhow::bail!(
                    "{} already exists and is not a socket",
                    SOCKET_PATH
                );
            }

            fs::remove_file(socket_path)
                .context(
                    "failed to remove stale socket"
                )?;
        }

        Err(error)
            if error.kind()
                == std::io::ErrorKind::NotFound =>
        {
            /*
             * Socket does not exist.
             * Nothing to remove.
             */
        }

        Err(error) => {
            return Err(
                anyhow::anyhow!(
                    "failed to inspect {}: {}",
                    SOCKET_PATH,
                    error
                )
            );
        }
    }

    /*
     * ========================================================
     * Create Unix socket
     * ========================================================
     */

    let listener =
        UnixListener::bind(socket_path)
            .context(
                "failed to create Senbit Unix socket"
            )?;

    /*
     * ========================================================
     * Permissions
     * ========================================================
     *
     * The socket is created by PID1/root.
     *
     * 0600:
     *
     *   root  -> read/write
     *   group -> no access
     *   other -> no access
     */

    fs::set_permissions(
        socket_path,
        fs::Permissions::from_mode(0o600),
    )
    .context(
        "failed to set permissions on Senbit socket"
    )?;

    logln!(
        "{}",
        "System event socket ready."
            .green()
            .bold()
    );

    Ok(listener)
}

/* ============================================================
   Event parsing
============================================================ */

fn parse_event(
    command: &str,
) -> Result<SystemEvent> {
    match command.trim() {
        "shutdown" => {
            Ok(SystemEvent::Shutdown)
        }

        "reboot" => {
            Ok(SystemEvent::Reboot)
        }

        "halt" => {
            Ok(SystemEvent::Halt)
        }

        "poweroff" => {
            Ok(SystemEvent::Poweroff)
        }

        other => {
            Err(
                anyhow::anyhow!(
                    "unknown system event '{}'",
                    other
                )
            )
        }
    }
}

/* ============================================================
   Socket client
============================================================ */

fn handle_socket_client(
    mut stream: UnixStream,
) -> Result<()> {
    let mut buffer =
        [0u8; 128];

    let size =
        stream
            .read(&mut buffer)
            .context(
                "failed to read socket request"
            )?;

    if size == 0 {
        return Ok(());
    }

    let command =
        std::str::from_utf8(
            &buffer[..size],
        )
        .context(
            "invalid UTF-8 in socket request"
        )?;

    let event =
        parse_event(command)?;

    /*
     * The handler is synchronous.
     *
     * The listener waits until the system event
     * has finished before continuing.
     */

    handle(event)?;

    stream
        .write_all(b"ok\n")
        .context(
            "failed to send socket response"
        )?;

    Ok(())
}

/* ============================================================
   Event listener
============================================================ */

pub fn start_event_listener(
    listener: UnixListener,
) -> Result<()> {
    logln!(
        "{}",
        "System event listener started."
            .green()
            .bold()
    );

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                if let Err(error) =
                    handle_socket_client(stream)
                {
                    logln!(
                        "{}",
                        format!(
                            "System event error: {}",
                            error
                        )
                        .red()
                    );
                }
            }

            Err(error) => {
                logln!(
                    "{}",
                    format!(
                        "Socket accept error: {}",
                        error
                    )
                    .red()
                );
            }
        }
    }

    Ok(())
}