# System API

Native commands do not touch the system directly: they ask `senbit-login`, which runs as root and does
the work. This is the **system API**. It is a Unix socket and a one-line protocol, deliberately small.

## The socket

| | |
|---|---|
| Path | `/run/senbit.sock` |
| Server | `senbit-login` (`components/senbit-login/src/events.rs`) |
| Access | root only (mode 0600); symbolic links and other file types at that path are refused |
| Created | at boot by `senbit-login.service`, removed at shutdown |

## Protocol

The client sends **one request word**, then reads until the connection closes. The answer is one line:

```text
ok                  the request was accepted
ok <value>          accepted, with a value (ping, version)
error: <message>    refused or failed
```

A client that connects and sends nothing is dropped after 5 seconds. Requests are never queued.

| Request | What it does | Answer |
|---|---|---|
| `reboot` | Reboots through systemd (`systemctl --no-block reboot`): every service is stopped in order, then the machine restarts | `ok` |
| `shutdown`, `poweroff` | Same, then powers off | `ok` |
| `halt` | Same, then halts | `ok` |
| `panic` | **Real kernel panic** (magic SysRq `c`), after a `sync`. Answers first, then crashes. The kernel command line has `panic=10`, so the machine reboots by itself 10 seconds later | `ok` |
| `ping` | Checks that the API answers | `ok pong` |
| `version` | The Senbit version (`VERSION=` of `/etc/senbit-release`) | `ok 0.4.0` |

Anything else is answered with `error: unknown system request '<word>'`.

Power requests do not stop services themselves: systemd does, so services added by packages are handled
with no change. `--no-block` queues the transition and returns, so the client always gets its answer
before `senbit-login` is stopped.

## Using it from a command

Commands share two small files from `commands/_shared/`, included with `#[path]`:

```rust
#[path = "../../_shared/api.rs"]
mod api;
#[path = "../../_shared/hook.rs"]
mod hook;

fn main() {
    hook::install();                       // clean message on a bug, exit status 70

    match api::request("ping") {
        Ok(answer) => println!("{answer}"), // "ok pong"
        Err(message) => eprintln!("error: {message}"),
    }
}
```

- `api::request(line)` returns the answer or a message ready to show (permission denied, `senbit-login`
  not running, no answer...).
- `hook::install()` replaces Rust's raw panic message: one clear line with the place, where to report the
  bug, and exit status **70**. Every native command calls it first.

`reboot`, `shutdown`, `halt` and `poweroff` still carry their own small client; `panic` is the first command
built on `api.rs`.

## The `panic` command

```bash
sudo panic          # asks you to type "panic", then crashes the kernel
sudo panic --yes    # without the question
```

It exists to test recovery: after the crash the machine reboots by itself, the boot counter and the
[automatic rollback](UPDATES.md) can be observed, and the recovery tools of [SYSTEM.md](SYSTEM.md) can be
exercised. Unsaved data is lost.

## Panics inside Senbit itself

A Rust panic in `senbit-init` (PID 1) used to end with `panic = "abort"`: the process died and the kernel
stopped with "Attempted to kill init", with no explanation. A panic hook now sends it through the same
**SENBIT KERNEL PANIC** screen as every other fatal error of init: the message and the place are shown,
then a countdown to power off, or a key press to reboot.

## Adding a request

1. Add a variant to `Request` and a word in `parse_request` (`events.rs`).
2. Handle it in `handle_client`; a request that does real work goes in its own function (`power.rs`).
3. Document it in the table above.
4. Call it from a command with `api::request("<word>")`.

Keep requests one word and without arguments as long as possible: the server reads one 128-byte message.
