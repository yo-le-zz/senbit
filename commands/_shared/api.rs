//! Client of the Senbit system API.
//!
//! The API is the event socket `/run/senbit.sock`, served by `senbit-login` (see
//! `docs/API.md`). Requests are one word, answers are `ok ...` or `error: ...`.
//! The socket belongs to root (mode 0600): only root can connect.
//!
//! Include it from a command with:
//!
//! ```ignore
//! #[path = "../../_shared/api.rs"]
//! mod api;
//!
//! match api::request("ping") {
//!     Ok(answer) => println!("{answer}"),
//!     Err(message) => eprintln!("error: {message}"),
//! }
//! ```
#![allow(dead_code)]

use std::io::{ErrorKind, Read, Write};
use std::os::unix::net::UnixStream;
use std::time::Duration;

/// The socket of the system API.
pub const SOCKET_PATH: &str = "/run/senbit.sock";

/// How long to wait for the answer.
const ANSWER_TIMEOUT: Duration = Duration::from_secs(10);

/// Sends one request and returns the answer without the final newline.
///
/// `Ok` carries the answer (`ok`, `ok pong`, `ok 0.4.0`...); `Err` carries the text of
/// the error, ready to be shown to the user.
pub fn request(line: &str) -> Result<String, String> {
    let mut stream = UnixStream::connect(SOCKET_PATH).map_err(|error| match error.kind() {
        ErrorKind::PermissionDenied => {
            format!("permission denied on {SOCKET_PATH}: run it as root (sudo)")
        }
        ErrorKind::NotFound => format!("{SOCKET_PATH} not found: is senbit-login running?"),
        _ => format!("cannot connect to {SOCKET_PATH}: {error}"),
    })?;

    let _ = stream.set_read_timeout(Some(ANSWER_TIMEOUT));

    stream
        .write_all(line.as_bytes())
        .map_err(|error| format!("cannot send the request: {error}"))?;

    // The server closes the connection after its answer. For a request that crashes the
    // system the connection may also break right after "ok": that is not an error.
    let mut answer = String::new();
    let _ = stream.read_to_string(&mut answer);
    let answer = answer.trim().to_string();

    if let Some(message) = answer.strip_prefix("error:") {
        Err(message.trim().to_string())
    } else if answer.is_empty() {
        Err("no answer from the system (the connection was closed)".to_string())
    } else {
        Ok(answer)
    }
}
