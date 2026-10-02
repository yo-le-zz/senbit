<p align="center">
  <img src="assets/branding/senbit-logo.svg" alt="Senbit" width="320">
</p>

<p align="center">
  <strong>A lightweight, secure, and server-first operating system built around the Linux kernel.</strong>
</p>

<p align="center">
  <a href="#-features">Features</a> •
  <a href="#-architecture">Architecture</a> •
  <a href="#-current-status">Current Status</a> •
  <a href="#-development">Development</a> •
  <a href="#-roadmap">Roadmap</a> •
  <a href="#-contributing">Contributing</a>
</p>

---

# 🦀 Senbit

**Senbit** is a general-purpose **server operating system based on the Linux kernel**, designed around four core principles:

> ⚡ Performance · 🪶 Lightweight · 🧱 Stability · 🛡️ Security

Senbit provides a minimal Linux foundation with a **Rust-first userspace** designed specifically for server environments.

The project does not attempt to replace the Linux ecosystem.

Instead, Senbit builds its own system infrastructure around the Linux kernel while maintaining compatibility with existing Linux software and technologies.

---

# 🚧 Current Status

> **Senbit is currently in early development.**

The foundational operating system is functional and can be installed and booted in QEMU using both BIOS and UEFI firmware.

The current system includes:

* 🐧 Linux kernel
* 🦀 Rust userspace
* 🚀 Native Senbit PID 1 / init system
* 💿 Interactive installation system
* 🖥️ BIOS and UEFI boot support
* 💾 Automatic and manual partitioning
* 🌐 DHCP networking
* 👤 Users and authentication
* 🔐 Native `sudo`
* ⚡ Native power-management commands
* 📋 Native logging system
* 📝 Native `log` command
* 🔌 Unix socket IPC
* 🧹 Clean shutdown infrastructure
* 🐚 Interactive shell
* 🧪 QEMU development workflow
* 🛠️ Native command build system

The current development focus is moving from the initial operating-system foundation toward a complete **server userspace and service-management infrastructure**.

---

# 🎯 Vision

Senbit aims to provide a lightweight and predictable operating system for servers ranging from a single self-hosted machine to large-scale infrastructure.

The long-term goal is to provide a system that is:

* ⚡ Fast and lightweight
* 🧱 Stable
* 🛡️ Secure by default
* 📦 Compatible with existing Linux software
* 🚀 Easy to deploy
* 🔄 Easy to update
* ↩️ Easy to recover
* 📊 Easy to observe
* 🖥️ Suitable for physical servers and virtual machines
* 🌐 Suitable for large-scale deployments

Senbit is intentionally **general-purpose**.

The same foundation should eventually support:

* Web servers
* Databases
* Container hosts
* Storage systems
* Game servers
* Internal infrastructure
* Virtual machines
* Other Linux-compatible workloads

---

# ✨ Features

## 🪶 Lightweight

Senbit uses a minimal userspace designed around server workloads.

The base installation avoids unnecessary graphical components and applications.

The system is assembled from a minimal root filesystem containing only the components required to boot and operate the server.

---

## 🦀 Rust-first Userspace

Senbit's own userspace is primarily written in **Rust**.

The Linux kernel remains independent from the Senbit userspace and provides the low-level operating-system functionality.

Rust is used for system components such as:

* `senbit-init`
* Installation
* Login
* Shell infrastructure
* System configuration
* Native commands
* Logging
* Shutdown infrastructure
* System IPC
* Future service management

---

# 💿 BIOS & UEFI Boot

Senbit supports both major x86 boot modes.

### Legacy BIOS

The installer can create an MBR partition table and install GRUB using the:

```text
i386-pc
```

target.

### UEFI

The installer can create a GPT partition table with an EFI System Partition and install GRUB using:

```text
x86_64-efi
```

The Senbit ISO itself supports both BIOS and UEFI boot.

```text
                 Senbit ISO
                     │
          ┌──────────┴──────────┐
          ▼                     ▼
       BIOS                  UEFI
          │                     │
     GRUB i386-pc         GRUB x86_64-efi
          │                     │
          └──────────┬──────────┘
                     ▼
              Senbit Linux
```

---

# 💾 Installation System

Senbit includes an interactive installer capable of:

* Detecting the current boot mode
* Detecting available disks
* Detecting existing partitions
* Automatically partitioning a disk
* Manual partitioning through `fdisk`
* Creating GPT partitions for UEFI
* Creating MBR partitions for BIOS
* Creating EFI System Partitions
* Formatting EFI partitions as FAT32
* Formatting root partitions as ext4
* Installing the Senbit root filesystem
* Creating users
* Configuring passwords
* Configuring hostname
* Configuring locale
* Configuring timezone
* Configuring keyboard layout
* Configuring networking
* Installing the bootloader

Senbit can also be installed onto an existing partition.

---

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

# ⚡ Native System Commands

Senbit is beginning to provide native userspace commands instead of relying entirely on external implementations.

Current commands include:

```text
halt
poweroff
reboot
shutdown
sudo
log
```

Commands are built independently and automatically installed by the Senbit command build system.

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
tools/cmdtool/src/commands/log/README.md
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

The current system supports DHCP networking.

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

Static networking is planned.

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
Welcome to Senbit 0.2.0
Hello, user!

[user@senbit /]$
```

The shell infrastructure already includes Senbit-specific terminal handling and configuration.

A complete TTY/session and job-control system is still under development.

---

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

Senbit includes a dedicated command build system called `cmdtool`.

Commands are organized independently:

```text
tools/cmdtool/src/commands/
├── halt/
├── log/
├── poweroff/
├── reboot/
├── shutdown/
└── sudo/
```

Each command can define its installation location through package metadata.

For example:

```toml
[package.metadata.senbit]
install = "bin"
```

or:

```toml
[package.metadata.senbit]
install = "sbin"
```

The command builder automatically discovers, builds, and installs the commands into the generated root filesystem.

---

# 🛠️ Development

Senbit provides a dedicated development tool:

```text
devtool
```

The development workflow manages the major components required to build and run Senbit.

## Install dependencies

```bash
./install.sh
```

## Run Senbit

The recommended development command is:

```bash
./scripts/devtool run --force
```

This performs the required rebuild and launches Senbit in QEMU.

For a completely fresh VM disk:

```bash
./scripts/devtool run --force --new
```

The `--new` option is useful when testing the installation process from scratch.

---

# 🏗️ Build Pipeline

The Senbit build system assembles the operating system through multiple stages:

```text
Linux Kernel
      │
      ▼
BusyBox
      │
      ▼
Util-linux / Parted
      │
      ▼
Senbit Rust Userspace
      │
      ▼
Native Commands
      │
      ▼
GRUB
      │
      ▼
Root Filesystem
      │
      ▼
Initramfs
      │
      ▼
ISO
```

The build system is designed to avoid rebuilding components unnecessarily.

---

# 🧪 Testing

Senbit is currently tested primarily in QEMU.

Current tested functionality includes:

* [x] Linux kernel boot
* [x] Senbit PID 1
* [x] Root filesystem mounting
* [x] `/proc`
* [x] `/sys`
* [x] `/dev`
* [x] `/run`
* [x] Installation detection
* [x] BIOS installation
* [x] UEFI installation
* [x] GPT partitioning
* [x] MBR partitioning
* [x] GRUB BIOS installation
* [x] GRUB UEFI installation
* [x] Root filesystem installation
* [x] `/etc/fstab`
* [x] `/etc/senbit-release`
* [x] Hostname
* [x] Locale
* [x] Timezone
* [x] DHCP networking
* [x] User creation
* [x] Password authentication
* [x] Keyboard persistence
* [x] Shell startup
* [x] Native `sudo`
* [x] Native power-management commands
* [x] System Unix socket
* [x] Logging
* [x] Native `log` command
* [x] Clean shutdown infrastructure
* [x] QEMU BIOS testing
* [x] QEMU UEFI testing

Current development areas:

* [ ] Complete TTY/session management
* [ ] Proper shell job control
* [ ] Service manager
* [ ] Static networking
* [ ] SSH
* [ ] Package management
* [ ] System updates
* [ ] Recovery mode
* [ ] Snapshots and rollback

---

# 🗺️ Roadmap

The roadmap focuses on system capabilities rather than fixed release dates.

## Phase 0 — Foundation 🏗️

* [x] Repository structure
* [x] Build system
* [x] Linux kernel integration
* [x] Minimal root filesystem
* [x] Senbit PID 1
* [x] QEMU boot
* [x] BIOS boot
* [x] UEFI boot
* [x] Installation system
* [x] User authentication
* [x] Basic networking
* [x] Interactive shell
* [x] Native command system
* [x] Logging system
* [x] Shutdown infrastructure
* [ ] Complete TTY/session management

## Phase 1 — Server Userspace 🖥️

* [x] Native `sudo`
* [x] Native power-management commands
* [x] Native logging
* [x] `log` command
* [x] Unix socket IPC
* [ ] Service definition format
* [ ] Service manager
* [ ] Service dependencies
* [ ] Service lifecycle management
* [ ] `systemctl`
* [ ] `senbitctl`
* [ ] Static networking
* [ ] SSH

## Phase 2 — Package Ecosystem 📦

* [ ] Define Senbit package format
* [ ] Study APT/dpkg reuse
* [ ] Debian package compatibility
* [ ] Package installation
* [ ] Package removal
* [ ] Package updates
* [ ] Dependency management
* [ ] Repository management
* [ ] Package signatures
* [ ] `senbit-pkg`

## Phase 3 — System Updates 🔄

* [ ] Version management
* [ ] Update detection
* [ ] Update download
* [ ] Integrity verification
* [ ] Authenticity verification
* [ ] Snapshot before update
* [ ] Component-level updates
* [ ] Offline updates
* [ ] Automatic rollback
* [ ] `senbit update`

## Phase 4 — Snapshots & Recovery ↩️

* [ ] Snapshot system
* [ ] Configuration snapshots
* [ ] Package snapshots
* [ ] Service snapshots
* [ ] Snapshot restoration
* [ ] Snapshot integrity verification
* [ ] Recovery mode
* [ ] Boot recovery
* [ ] Automatic rollback

## Phase 5 — Security & Monitoring 🛡️

* [ ] Firewall
* [ ] Linux capabilities management
* [ ] Security auditing
* [ ] Service exposure auditing
* [ ] File integrity tracking
* [ ] Security event management
* [ ] Configuration validation
* [ ] Security hardening

## Phase 6 — Deployment 🚀

* [ ] Declarative configuration
* [ ] Automated installation
* [ ] Automated system configuration
* [ ] Automated package installation
* [ ] Reproducible server deployment
* [ ] Server fleet management
* [ ] Remote administration

## Phase 7 — Multi-Architecture 🌍

### x86_64

* [x] Initial support

### ARM64

* [ ] Kernel support
* [ ] Userspace build
* [ ] Initramfs build
* [ ] QEMU support
* [ ] Image generation
* [ ] Installation testing

### Other architectures

* [ ] Evaluate RISC-V
* [ ] Evaluate ARM32
* [ ] Define officially supported architectures

---

# 🔭 Long-Term Vision

The long-term goal is to make Senbit a complete server operating platform.

A server should eventually be manageable through a predictable lifecycle:

```text
             Deploy
               │
               ▼
           Configure
               │
               ▼
              Run
               │
               ▼
           Monitor
               │
               ▼
            Update
               │
               ▼
            Verify
               │
          ┌────┴────┐
          ▼         ▼
       Success    Failure
          │         │
          ▼         ▼
       Continue   Rollback
```

Senbit should make server operations:

* Predictable
* Automatable
* Observable
* Recoverable
* Reproducible

The project will continue building around the Linux ecosystem rather than attempting to replace it.

---

# 🤝 Contributing

Senbit is developed openly.

Contributions are welcome in:

* 💻 Code
* 🦀 Rust development
* 🐧 Linux kernel work
* 🧪 Testing
* 📚 Documentation
* 🐛 Bug reports
* 💡 Architecture proposals
* 🔐 Security research
* 🏗️ Build infrastructure

Before contributing, read:

* [`CONTRIBUTING.md`](CONTRIBUTING.md)
* [`SECURITY.md`](SECURITY.md)
* [`CODE_OF_CONDUCT.md`](CODE_OF_CONDUCT.md)

The `main` branch is protected.

Development changes should be made on dedicated branches and merged through pull requests.

---

# 📜 License

Senbit's original code is licensed under the:

**Apache License 2.0**

The Linux kernel and third-party components retain their respective licenses.

Those components remain under their respective licenses.

See [`LICENSE`](LICENSE) for the license covering Senbit's original code.

---

# ⭐ Support Senbit

If you are interested in Senbit:

* ⭐ Star the repository
* 💬 Join discussions
* 🐛 Report bugs
* 💡 Propose improvements
* 🧑‍💻 Contribute code
* 📚 Improve documentation

---

<p align="center">

🦀 **Senbit**

<i>Server infrastructure, built around Linux.</i>

</p>
