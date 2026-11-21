# Architecture

> Part of the [Senbit documentation](../README.md#documentation).

# 🏗️ Architecture

Senbit is built **around the Linux kernel**.

```text
┌────────────────────────────────────────────────────┐
│                  Server Applications                │
│                                                    │
│ Docker · Podman · nginx · databases · ...         │
├────────────────────────────────────────────────────┤
│                   Senbit Userspace                 │
│                                                    │
│ Rust commands · login · shell · sudo · log         │
├────────────────────────────────────────────────────┤
│              Senbit System Infrastructure          │
│                                                    │
│ PID 1 · IPC · networking · logging · installation  │
│ shutdown · configuration · services               │
├────────────────────────────────────────────────────┤
│                    Linux Kernel                    │
│                                                    │
│ Processes · memory · networking · drivers         │
│ filesystems · security · virtualization           │
└────────────────────────────────────────────────────┘
```

---

# 🐧 Linux Kernel

The Linux kernel provides the low-level operating-system functionality required by Senbit.

This includes:

* CPU scheduling
* Memory management
* Networking
* Storage
* Filesystems
* Hardware drivers
* Process management
* System calls
* Security primitives
* Virtualization

Senbit does not attempt to rewrite functionality already provided by Linux.

Instead, the project focuses on building a lightweight and specialized server userspace around it.

---

# 🦀 Rust Userspace

Senbit's own system components are primarily written in Rust.

Current architecture includes:

```text
src/
├── filesystem/
├── installation/
├── kernel/
├── system/
│   ├── handler/
│   ├── init/
│   ├── login/
│   ├── log/
│   └── shell/
└── utils/
```

The userspace is being progressively separated into dedicated subsystems.

This allows components such as logging, networking, services, authentication, and shutdown handling to evolve independently.

---

# 📦 Native Command System

Senbit's own commands live in a dedicated top-level folder, `commands/`:
**one folder = one command = one independent Rust crate.**

```text
commands/
├── adduser/      adduser, addgroup, deluser, delgroup  (Debian-compatible)
├── bootreport/   boot report shown at login
├── findmnt/
├── getent/
├── halt/  poweroff/  reboot/  shutdown/
├── log/
├── sudo/
├── useradd/      useradd, groupadd, userdel, groupdel, usermod
├── _shared/      code shared by several commands (not a command)
└── INDEX.md      generated index of every command
```

Each command describes its installation in its own `Cargo.toml`:

```toml
[package.metadata.senbit]
install = "sbin"                       # "bin" (default) or "sbin"
aliases = ["addgroup", "deluser"]      # optional: extra names (links to the same binary)
setuid  = true                         # optional: setuid root (like sudo)
```

## Adding a command

```bash
./newcmds.sh mycmd                 # creates commands/mycmd/ (empty Rust crate), indexes it
./newcmds.sh mycmd --sbin --alias mc,mycommand -d "What it does"
./scripts/devtool build            # detects, compiles and installs it in the rootfs
```

Nothing else to register: `devtool` scans `commands/`, builds every crate
(shared `target/`, cached per command), installs the binaries in `/usr/bin` or
`/usr/sbin`, creates the aliases and regenerates `commands/INDEX.md`.
`./newcmds.sh --list` lists the commands, `./newcmds.sh --remove mycmd` deletes one.
See [commands/README.md](commands/README.md).
