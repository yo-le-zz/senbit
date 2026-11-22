# Architecture

Senbit is built **around the Linux kernel**. It does not rewrite what Linux already does well; it adds a
small Rust userspace on top and reuses the standard server tools (systemd, apt, dpkg).

```text
┌────────────────────────────────────────────────────────┐
│ Server applications   Docker · nginx · databases ...   │  installed with apt
├────────────────────────────────────────────────────────┤
│ Senbit userspace      init · login · installer         │  Rust
│                       native commands · devtool        │
├────────────────────────────────────────────────────────┤
│ System infrastructure systemd · apt/dpkg · networking  │  standard tools
│                       updates · recovery               │
├────────────────────────────────────────────────────────┤
│ Linux kernel          processes · memory · drivers ·   │  pinned version,
│                       filesystems · networking         │  Senbit configuration
└────────────────────────────────────────────────────────┘
```

## Components

| Component | Role | Source |
|---|---|---|
| `senbit-init` | Runs from the initramfs, only for the transition: finds the disk, mounts the system, prepares it, then `exec`s systemd. Also contains the installer. | `src/` |
| `senbit-login` | Console login, and the event socket used by the power commands. Independent crate. | `components/senbit-login/` |
| Native commands | One crate per command: `sudo`, `log`, `bootreport`, `findmnt`, `getent`, `adduser`, `useradd`, `halt`, `poweroff`, `reboot`, `shutdown`. | `commands/` |
| systemd | PID 1 of the running system: services, ordering, shutdown. | built from the pinned sources |
| apt and dpkg | Packages, with the Debian `.deb` format. | see [PACKAGES.md](PACKAGES.md) |
| `devtool` | Builds everything and runs it in QEMU. | `devtool/` |

The kernel, BusyBox, util-linux, Parted and systemd are downloaded at **pinned versions** (see
[BUILD.md](BUILD.md)); nothing follows "latest".

## Source layout

```text
src/                  senbit-init
├── filesystem/       disks, mounts
├── installation/     the installer (bootloader, local settings, network, users)
├── kernel/           kernel-facing helpers
├── system/           init sequence and logging
└── utils/            shared helpers (hashing...)
components/           separate programs (senbit-login)
commands/             native commands, one crate each (see below)
rootfs/               overlay merged into the root filesystem (units, scripts, /etc)
tools/                generators run at build time (keymaps, fonts, GRUB files)
devtool/              the build tool
config/               pinned versions, kernel and BusyBox configuration, VM settings
patches/              patches applied to the downloaded sources
```

## From power-on to login

```text
GRUB → kernel → initramfs → senbit-init
         │
         ├─ no installation found → installer
         │
         └─ installed system
              mount the root filesystem, then /proc /sys /dev /run /dev/shm /dev/pts and cgroup v2
              check /etc/fstab, apply hostname, bring the network up (first DHCP lease)
              switch_root, load the keyboard layout
              exec /usr/lib/systemd/systemd
                   │
                   systemd (PID 1)
                   ├─ systemd-sysusers, systemd-tmpfiles, journald, udevd, ldconfig
                   ├─ senbit-dhcp@<interface>  (keeps the DHCP lease)
                   └─ senbit-login             (console login)
```

Only an impossible mount or switch root stops the boot. Every other step (fstab, hostname, network,
keyboard) records its result and the system starts anyway: see *Boot report* in [SYSTEM.md](SYSTEM.md).

## Power commands

`reboot`, `shutdown`, `halt` and `poweroff` write one word to `/run/senbit.sock` (root only).
`senbit-login` answers `ok`, then runs `systemctl --no-block <verb>`. systemd stops **every** service in
reverse dependency order (Senbit's and those added by packages), then unmounts and powers off. No service
name is written in the code.

## Native commands

Senbit's own commands live in `commands/`: **one folder, one command, one independent Rust crate.**

```text
commands/
├── adduser/      adduser, addgroup, deluser, delgroup  (Debian-compatible)
├── bootreport/
├── findmnt/
├── getent/
├── halt/  poweroff/  reboot/  shutdown/
├── log/
├── sudo/
├── useradd/      useradd, groupadd, userdel, groupdel, usermod
├── _shared/      code shared by several commands (not a command)
└── INDEX.md      generated index of every command
```

Each command describes how it is installed in its own `Cargo.toml`:

```toml
[package.metadata.senbit]
install = "sbin"                       # "bin" (default) or "sbin"
aliases = ["addgroup", "deluser"]      # optional: other names for the same binary
setuid  = true                         # optional: setuid root (as `sudo`)
```

To add one:

```bash
./newcmds.sh mycmd                                   # creates commands/mycmd/ and indexes it
./newcmds.sh mycmd --sbin --alias mc -d "What it does"
./scripts/devtool build                              # detects, builds and installs it
```

Nothing else has to be registered: `devtool` scans `commands/`, builds each crate, installs the binary in
`/usr/bin` or `/usr/sbin`, creates the aliases and regenerates `commands/INDEX.md`. More in
[commands/README.md](../commands/README.md).
