# The running system

What you find on an installed Senbit: login, `sudo`, network, boot report, logs and configuration.
The services are described in [SERVICES.md](SERVICES.md) and the systemd integration in
[SYSTEMD.md](SYSTEMD.md). Every native command is listed in [commands/INDEX.md](../commands/INDEX.md).

## Login and users

The installer creates `root` and one main user. Accounts are the standard files `/etc/passwd`,
`/etc/shadow` and `/etc/group`, with hashed passwords.

`senbit-login` runs on the console. It offers `root` and the normal users (UID 1000 or more) that have a
real shell and a valid password: the system accounts created by packages (`www-data`, `_apt`...) never
appear. After a successful login it starts the user's shell and rebuilds its environment, since systemd
starts services with an almost empty one:

| Variable | Source |
|---|---|
| `HOSTNAME` | `/etc/hostname` |
| `LANG`, `LC_ALL` | `/etc/default/locale` |
| `TZ` | `/etc/timezone` |
| `SENBIT_VERSION` | `/etc/senbit-release` |
| `USER`, `HOME`, `SHELL`... | `/etc/passwd` |

The shell is BusyBox `ash`. Complete TTY and session management and job control are still to do.

## sudo

Senbit has a native `sudo` (setuid root). It asks for the **root password**, hidden while typed, and keeps
the authentication for five minutes (timestamp stored under a protected directory; symbolic links are
refused). Accounts are managed with `adduser` / `useradd` and their relatives, which are Debian-compatible.

## Network

DHCP and static networking are supported. The installer asks for the mode and writes
`/etc/network/interfaces` (BusyBox `ifup`) and `/etc/resolv.conf`. To change it later, edit
`/etc/network/interfaces` and run `ifdown eth0 && ifup eth0` (see [SERVICES.md](SERVICES.md)).

- **The network card is detected, never assumed.** Only real cards count (`lo`, `dummy0`, bridges, `veth*`
  and `docker0` never do). With one card the installer selects it whatever its name; it asks only when
  there are several. At boot, `senbit-init` configures the detected cards that the file describes.
- **A network failure never blocks the boot.** It is recorded in the boot report and shown at login. The
  reachability test pings the default gateway.

## Boot report

`senbit-init` stops the boot only when it cannot reach systemd. Every other step records its result in
`/run/senbit/boot-report` and the system starts anyway. At login, a box recalls what happened; it stays
until you run `clear`:

```text
┌─ Boot report ───────────────────────────────────────────────────────────┐
│ ✓ 7 OK   ⚠ 1 warning(s)   ✗ 0 failed                                    │
│ ⚠ Network: no default route: the DHCP server did not answer ...         │
└─────────────────────────────────────────────────────────────────────────┘
```

```bash
bootreport             # every step, failed units, what to do
bootreport --logs      # systemd journal of this boot (journalctl -b)
bootreport --kernel    # kernel log
bootreport --failed    # failed systemd units
bootreport --history   # previous boots (/var/log/senbit/boot-report.log)
```

`bootreport` also shows when updates are available (see [UPDATES.md](UPDATES.md)). It is the only
Senbit-specific administration tool: everything else uses the standard commands (`systemctl`,
`journalctl`, `ip`, `apt`, `adduser`...).

## Logs

- Services: `journalctl` (the systemd journal).
- Senbit's own components write structured lines (timestamp, level, module, message) to
  `/var/log/senbit/system.log` and `/run/log/system.log`. Levels: `ERROR`, `WARN`, `INFO`, `DEBUG`, `TRACE`.
- The `log` command reads them:

```bash
log                    # recent entries
log errors             # errors only
log follow             # follow in real time
log show --grep network
log kernel -n 50
log stats
```

Filtering by level, module and time, JSON output, rotation and clearing are covered in
[commands/log/README.md](../commands/log/README.md).

## Configuration

| Setting | File |
|---|---|
| Hostname | `/etc/hostname` |
| Keyboard | `/etc/default/keyboard` (`XKBLAYOUT="fr"`...); the console keymap is restored at boot |
| Locale | `/etc/default/locale` |
| Timezone | `/etc/timezone` |
| Version | `/etc/senbit-release` |
| Network | `/etc/network/interfaces`, `/etc/resolv.conf` |

## Debugging a boot

`./scripts/devtool run --debug` writes the whole boot (kernel, `senbit-init`, systemd, journal, login) to
`build/vm/qemu.log`, without cluttering the screen. Without QEMU, add `senbit.debug` to the `linux` line
in GRUB.
