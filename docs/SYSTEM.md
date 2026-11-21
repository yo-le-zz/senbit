# The running system

> Part of the [Senbit documentation](../README.md#documentation).
> The native commands are described in [commands/README.md](../commands/README.md) and
> [commands/INDEX.md](../commands/INDEX.md).

# 🚀 Boot & Initialization

Senbit uses its own Rust-based initialization system.

The general boot sequence is:

```text
Linux Kernel
     │
     ▼
Senbit Init
     │
     ▼
Detect installation
     │
     ├── No installation
     │       │
     │       ▼
     │    Installer
     │
     └── Installed system
             │
             ▼
        Detect system disk
             │
             ▼
        Mount root filesystem
             │
             ▼
        Mount /proc
        Mount /sys
        Mount /dev
        Mount /run
             │
             ▼
        Verify /etc/fstab
             │
             ▼
        Initialize system configuration
             │
             ▼
        Initialize hostname
             │
             ▼
        Initialize network
             │
             ▼
        Start system infrastructure
             │
             ▼
        Login
             │
             ▼
        User shell
```

The initialization system is intentionally being built as modular Rust components rather than a collection of unrelated shell scripts.

---

# 🔌 System IPC

Senbit currently uses a Unix domain socket for communication with PID 1.

The socket is:

```text
/run/senbit.sock
```

It is restricted to root and is used by native system commands to request privileged system events.

Current events include:

```text
shutdown
reboot
halt
poweroff
```

The architecture is:

```text
Native command
      │
      ▼
/run/senbit.sock
      │
      ▼
Senbit PID 1
      │
      ▼
System handler
      │
      ▼
Shutdown / reboot / halt / poweroff
```

This provides a foundation for future communication between userspace tools and the Senbit system manager.

---

# 🔐 Privilege Management

Senbit includes a native `sudo` implementation.

Current functionality includes:

* Root password authentication
* Hidden password input
* SUID root execution
* Authentication timestamp caching
* Five-minute authentication timeout
* Protected timestamp storage
* Privilege switching
* Group initialization before privileged execution

Example:

```bash
sudo command
```

The privilege-management infrastructure is still evolving and will receive additional hardening as the userspace develops.

---

# 📝 Logging System

Senbit has a native logging infrastructure designed for system observability.

Logs contain structured information such as:

* Timestamp
* Log level
* Source module
* Message

Supported levels include:

```text
ERROR
WARN
INFO
DEBUG
TRACE
```

Senbit maintains both persistent and runtime logs:

```text
/var/log/senbit/system.log
/run/log/system.log
```

The logging architecture is independent from `systemd` and `journald`.

---

# 📋 `log` Command

Senbit provides a native `log` command for inspecting and managing system logs.

It supports:

* Log display
* Real-time following
* Boot logs
* System logs
* Error filtering
* Warning filtering
* Kernel logs
* Source selection
* Text search
* Regular expressions
* Module filtering
* Level filtering
* Time filtering
* JSON output
* Raw output
* Log statistics
* Log clearing
* Log rotation

Examples:

```bash
log
```

```bash
log errors
```

```bash
log follow
```

```bash
log show --grep network
```

```bash
log kernel -n 50
```

```bash
log stats
```

The complete command documentation is available in:

```text
commands/log/README.md
```

---

# 🧹 Shutdown Infrastructure

Senbit is developing a centralized shutdown infrastructure.

The shutdown path is designed to eventually perform:

```text
Shutdown request
       │
       ▼
Stop services
       │
       ▼
Stop remaining processes
       │
       ▼
Clean temporary resources
       │
       ▼
Invalidate sensitive runtime data
       │
       ▼
Sync filesystems
       │
       ▼
Unmount filesystems
       │
       ▼
Final action
       │
       ├── poweroff
       ├── reboot
       └── halt
```

Current infrastructure already includes:

* Process termination
* Temporary resource cleanup
* `/tmp` cleanup
* Runtime socket cleanup
* Filesystem synchronization
* Filesystem unmounting

Service-aware shutdown will be completed together with the future service manager.

---

# 🌐 Networking

The current system supports **DHCP and static networking**.

The installer asks for the interface and the mode; for a static address it asks the address with its
prefix (`192.168.1.10/24`), the gateway and the DNS servers, and writes `/etc/network/interfaces`
(BusyBox `ifup`) plus `/etc/resolv.conf`. To change it later, edit `/etc/network/interfaces` and run
`ifdown eth0 && ifup eth0` (see [docs/SERVICES.md](docs/SERVICES.md)).

**The network card is detected, never assumed.** Only real network cards are considered (the kernel gives them a
device: `lo`, `dummy0`, bridges, `veth*` and `docker0` never are). When there is one card, the installer selects it
without asking, whatever its name (`eth0`, `ens3`, `enp1s0`...); it only asks when there are several. At boot,
`senbit-init` configures the detected cards that `/etc/network/interfaces` describes and ignores the others.

**A network failure never blocks the boot.** It is recorded in the boot report and shown at login
(see *Boot Report* below). The reachability test pings the **default gateway** (it used to ping the
QEMU address `10.0.2.2`, which failed on any other network).

The initialization process detects network interfaces and configures them using the current Linux networking utilities.

The current implementation uses BusyBox networking tools and `udhcpc`.

```text
Senbit Init
    │
    ▼
Detect interfaces
    │
    ▼
Network configuration
    │
    ▼
ifup
    │
    ▼
udhcpc
    │
    ▼
DHCP lease
    │
    ▼
Network ready
```

---

# 🩺 Boot Report

`senbit-init` only stops the boot when it cannot reach systemd (root filesystem, switch root). Every other step
(fstab, hostname, network, logs, keyboard) records its result in `/run/senbit/boot-report` and the system starts
anyway. At each login a box recalls what happened; it stays on screen until you run `clear`:

```text
┌─ Boot report ───────────────────────────────────────────────────────────┐
│ ✓ 7 OK   ⚠ 1 warning(s)   ✗ 0 failed                                    │
│ ⚠ Network: no default route: the DHCP server did not answer ...         │
└─────────────────────────────────────────────────────────────────────────┘
  bootreport: details | bootreport --logs: systemd logs of this boot | `clear` hides this box
```

```bash
bootreport             # recap: every step (✓ ⚠ ✗), failed systemd units, what to do
bootreport --logs      # systemd journal of this boot (journalctl -b)
bootreport --kernel    # kernel log
bootreport --failed    # failed systemd units
bootreport --history   # previous boots (/var/log/senbit/boot-report.log)
```

`bootreport` is the only Senbit-specific tool of the system administration set: everything else uses the standard
commands (`systemctl`, `journalctl`, `ip`, `apt`, `adduser`...).

---

# 👤 Users & Authentication

The installer creates:

* `root`
* A configurable main user

Standard Linux account files are used:

```text
/etc/passwd
/etc/shadow
/etc/group
```

Passwords are stored as password hashes.

After boot, Senbit provides an interactive login system:

```text
Select user:
Password:
```

After successful authentication, the user's shell session is started.

---

# ⌨️ Keyboard Configuration

The installer allows the keyboard layout to be selected.

The configuration is stored in:

```text
/etc/default/keyboard
```

Example:

```ini
XKBMODEL="pc105"
XKBLAYOUT="fr"
XKBVARIANT=""
XKBOPTIONS=""
```

The selected console keymap is restored during system initialization.

---

# 🏠 Hostname

Senbit supports hostname configuration during installation.

The hostname is stored in:

```text
/etc/hostname
```

The initialization system reads the hostname during boot and configures the running system accordingly.

---

# 🌍 Locale & Timezone

The installer provides configuration for:

* Locale
* Timezone
* Keyboard layout

These settings are installed into the target filesystem and restored during boot.

---

# 🐚 Shell

Senbit currently provides a minimal shell environment based on BusyBox `/bin/sh`.

After authentication, the configured user's shell is started.

Example:

```text
Welcome to Senbit 0.4.0
Hello, user!

[user@senbit /]$
```

The shell infrastructure already includes Senbit-specific terminal handling and configuration.

A complete TTY/session and job-control system is still under development.
