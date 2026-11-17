# Senbit native commands

Programs that run **inside** Senbit live here: one folder = one command = one
independent Rust crate. They are detected, compiled and installed in the root
filesystem automatically by `devtool` (see [INDEX.md](INDEX.md) for the list).

## Add a command

```bash
./newcmds.sh mycmd                       # commands/mycmd/ (empty Rust crate) + index
./newcmds.sh mycmd --sbin                # installed in /usr/sbin instead of /usr/bin
./newcmds.sh mycmd --alias mc,mycommand  # extra names (links to the same binary)
./newcmds.sh mycmd --setuid              # setuid root (like sudo)
./newcmds.sh mycmd -d "What it does"     # description shown in INDEX.md
./newcmds.sh --list                      # detected commands
./newcmds.sh --remove mycmd              # delete a command
```

Then edit `commands/mycmd/src/main.rs` and run `./scripts/devtool build`
(or `./scripts/devtool build commands` for the commands only). There is nothing
else to register.

## How it works

`devtool` (module `devtool/src/build/commands.rs`):

1. scans `commands/*/Cargo.toml` (folders starting with `_` or `.` are ignored);
2. reads `[package.metadata.senbit]` of each one;
3. compiles every crate for `x86_64-unknown-linux-musl` (static binaries) with a
   shared `target/` (`build/commands/target`), skipping the commands whose files
   did not change;
4. stages them in `build/commands/stage/{bin,sbin}` (aliases = relative links,
   `setuid = true` -> mode 4755) and lists them in `/usr/share/senbit/commands.list`;
5. regenerates `commands/INDEX.md`;
6. `build::rootfs` merges the stage into the rootfs AFTER BusyBox, systemd and the
   Debian packages: a Senbit command always wins over a link of the same name.

A command name (or alias) can only be provided once: the build fails with a clear
message otherwise.

## `Cargo.toml`

```toml
[package]
name = "mycmd"            # the binary must be named like this (or like the first [[bin]])
version = "0.1.0"
edition = "2024"
description = "Shown in INDEX.md"

[package.metadata.senbit]
install = "bin"                  # "bin" (default) or "sbin"
aliases = ["mc", "mycommand"]    # optional
setuid  = false                  # optional

[dependencies]
```

A command with aliases is ONE binary that looks at `argv[0]` to know which name
it was called by (see `commands/adduser`).

## Shared code

`commands/_shared/` is not a command (no `Cargo.toml`). A command includes a shared
file with `#[path = "../../_shared/accounts.rs"] mod accounts;`
(used by `adduser` and `useradd` to edit `/etc/passwd`, `group`, `shadow`, `gshadow`).

## Tests

Each command carries unit tests: `cargo test --manifest-path commands/<name>/Cargo.toml`
(CI runs them all).
