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
  <a href="#-roadmap">Roadmap</a> •
  <a href="#-development">Development</a> •
  <a href="#-contributing">Contributing</a>
</p>

---

# 🦀 Senbit

**Senbit** is a general-purpose **server operating system based on the Linux kernel**, designed around four core principles:

> ⚡ Performance · 🪶 Lightweight · 🧱 Stability · 🚀 Deployment

Senbit aims to provide a clean, efficient, and secure foundation for servers while preserving the compatibility and ecosystem of Linux.

The project does not aim to reinvent the Linux ecosystem.

Instead, Senbit builds modern infrastructure around the Linux kernel, with a lightweight Rust-first userspace designed specifically for server environments.

---

# 🚧 Current Status

> **Senbit is currently in early development.**

The core operating-system foundation is now functional in QEMU.

The project currently has a working:

* 🐧 Linux kernel
* 🦀 Rust userspace
* 📦 Minimal root filesystem
* 🚀 Senbit init system
* 💿 Interactive installation system
* 🖥️ BIOS and UEFI boot support
* 💾 Automatic disk partitioning
* 🌐 DHCP networking
* 👤 User creation and authentication
* ⌨️ Persistent keyboard layout configuration
* 🏠 Hostname configuration
* 🕐 Locale and timezone configuration
* 🐚 Interactive shell
* 🧪 QEMU development workflow

The current development focus is moving from the initial installation foundation toward a complete boot and service-management system.

---

# 🎯 Vision

Modern infrastructure can involve anything from a single self-hosted server to thousands of machines.

Senbit is designed with both use cases in mind.

The long-term goal is to provide a server operating system that is:

* ⚡ Fast and lightweight
* 🧱 Stable
* 🛡️ Secure by default
* 📦 Compatible with existing Linux software
* 🚀 Easy to deploy
* 🔄 Easy to update
* ↩️ Easy to recover and roll back
* 🖥️ Suitable for physical servers and virtual machines
* 🌐 Suitable for large-scale deployments

Senbit is intentionally **general-purpose**.

The operating system does not target a single type of server.

The same Senbit foundation should eventually be usable for web servers, databases, container hosts, storage systems, game servers, internal infrastructure, or other Linux-compatible workloads.

---

# ✨ Features

## 🪶 Lightweight

Senbit uses a minimal userspace designed specifically for server environments.

The base system avoids unnecessary graphical components and applications.

## 🦀 Rust-first Userspace

Senbit's own userspace is primarily written in Rust.

The Linux kernel remains independent from the Senbit userspace and provides the low-level operating-system functionality.

## 💿 BIOS & UEFI Boot

The installation system supports both major x86 boot modes.

### Legacy BIOS

Senbit can automatically create an MBR partition table and install GRUB using the `i386-pc` target.

### UEFI

Senbit can automatically create a GPT partition table with an EFI System Partition and install GRUB using the `x86_64-efi` target.

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

## 💾 Installation System

Senbit includes an interactive installer capable of:

* Detecting the current boot mode
* Detecting available disks and partitions
* Automatically partitioning a disk
* Manual partitioning through `fdisk`
* Creating GPT partitions for UEFI
* Creating MBR partitions for BIOS
* Formatting EFI partitions as FAT32
* Formatting root partitions as ext4
* Installing the Senbit root filesystem
* Configuring users
* Configuring hostname
* Configuring locale
* Configuring timezone
* Configuring networking
* Installing the bootloader

The installer also supports installing Senbit onto an existing partition.

## 🌐 Networking

The current system supports network initialization through DHCP.

Senbit detects usable network interfaces and configures them using the system's network configuration.

The current network stack uses BusyBox networking utilities and `udhcpc`.

```text
Senbit Init
    │
    ▼
Detect interfaces
    │
    ▼
Read network configuration
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

Static network configuration is planned for a future stage.

## 👤 Users & Authentication

The installer creates:

* The `root` account
* A configurable main user account

User information is stored using standard Linux account files:

```text
/etc/passwd
/etc/shadow
/etc/group
```

Passwords are stored as password hashes rather than plaintext.

After boot, Senbit provides an interactive login system allowing the administrator to select a user and authenticate using their password.

## ⌨️ Keyboard Configuration

The installer allows the keyboard layout to be selected during installation.

The selected layout is:

1. Loaded immediately during installation.
2. Stored in `/etc/default/keyboard`.
3. Restored during system initialization.

For example:

```ini
XKBMODEL="pc105"
XKBLAYOUT="fr"
XKBVARIANT=""
XKBOPTIONS=""
```

Senbit uses BusyBox `loadkmap` and `.bmap` keymaps for the console.

This ensures that the keyboard layout used during installation can also be restored after reboot.

## 🏠 Hostname

Senbit supports hostname configuration during installation.

The hostname is stored in:

```text
/etc/hostname
```

The system initialization process reads the configured hostname during boot.

## 🌍 Locale & Timezone

The installer provides configuration for:

* Locale
* Timezone
* Keyboard layout

These settings are installed into the target filesystem and are intended to persist across reboots.

## 🐚 Shell

Senbit currently provides a minimal shell environment based on BusyBox `/bin/sh`.

After successful authentication, the configured user's shell is started.

Example:

```text
Welcome to Senbit 0.1.0
Hello, user!

~ #
```

A full TTY/session management system is still under development.

---

# 🏗️ Architecture

Senbit is built **around the Linux kernel**, rather than attempting to replace it.

```text
┌────────────────────────────────────────────────┐
│              Server Applications               │
│                                                │
│ Docker · Podman · nginx · databases · ...     │
├────────────────────────────────────────────────┤
│                 Senbit Userspace               │
│                                                │
│ Rust services · system tools · login · init   │
├────────────────────────────────────────────────┤
│            Senbit System Infrastructure        │
│                                                │
│ Installation · networking · updates           │
│ configuration · services · recovery            │
├────────────────────────────────────────────────┤
│                  Linux Kernel                   │
│                                                │
│ Processes · memory · networking · drivers     │
│ filesystems · security · virtualization       │
└────────────────────────────────────────────────┘
```

The Linux kernel provides the low-level operating-system functionality.

Senbit focuses on the userspace and infrastructure surrounding it.

---

# 🐧 Linux Kernel

Senbit uses the Linux kernel as its foundation.

This provides access to the mature Linux ecosystem, including:

* CPU scheduling
* Memory management
* Networking
* Storage
* Filesystems
* Hardware drivers
* Security primitives
* Virtualization
* Process management
* System calls
* Multiple CPU architectures

Senbit does not aim to rewrite functionality that Linux already provides reliably.

Instead, the project maintains its own kernel configuration and may introduce carefully scoped patches when there is a concrete reason to do so.

---

# 🦀 Rust-first Userspace

Senbit's own system components are primarily written in **Rust**.

The current userspace includes components for:

```text
src/
├── filesystem/
├── installation/
├── system/
│   ├── init/
│   └── login/
├── utils/
└── ...
```

The system initialization architecture is being developed around dedicated components for:

* Filesystem initialization
* Hostname
* Keyboard configuration
* Networking
* Services
* Updates
* Login

The goal is to keep system functionality modular rather than placing the entire operating system initialization logic into one executable module.

---

# 🚀 Boot & Initialization

The current Senbit boot process follows this general flow:

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
     │       ↓
     │    Installer
     │
     └── Installed system
             ↓
        Mount root filesystem
             ↓
        Mount proc/sys/dev/run
             ↓
        Verify fstab
             ↓
        Initialize hostname
             ↓
        Restore keyboard layout
             ↓
        Check system configuration
             ↓
        Initialize network
             ↓
        Start services
             ↓
        Login
             ↓
        User shell
```

The initialization system is still evolving.

The next major step is to turn the current initialization sequence into a complete service-management system.

---

# 📦 Linux Software Ecosystem

Senbit intentionally does not create a completely new application ecosystem.

The long-term goal is to integrate with existing Linux software and package ecosystems rather than forcing applications to be rewritten specifically for Senbit.

The project intends to support Debian packages and APT.

Potential applications include:

* 🐳 Docker
* 📦 Podman
* ☸️ Kubernetes
* 🌐 nginx
* 🔐 OpenSSH
* 🗄️ PostgreSQL
* 🐬 MariaDB
* and many other Linux-compatible applications

These applications are not installed by default.

---

# 🛠️ Development

Senbit includes a dedicated development tool called **`devtool`**.

The development workflow is designed around building the kernel, userspace, root filesystem, initramfs, GRUB, and ISO while keeping the individual build components organized.

## Install development dependencies

```bash
./install.sh
```

Installs the required development dependencies.

## Development tool

The recommended development workflow uses:

```bash
./scripts/devtool
```

The tool manages common development operations such as building and running Senbit.

For example, to rebuild the system and launch it in QEMU:

```bash
./scripts/devtool run --force
```

This performs the required rebuild before starting the virtual machine.

## Build system

The Senbit build process assembles:

```text
Linux Kernel
     │
     ▼
BusyBox
     │
     ▼
Senbit Rust Userspace
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

The build system is designed to rebuild only the components that require rebuilding when possible.

---

# 🧪 Current Development Environment

Senbit is currently developed and tested primarily using:

* x86_64
* Linux hosts
* QEMU
* BIOS firmware
* UEFI/OVMF firmware

The project currently prioritizes reliable virtual-machine testing before expanding testing to physical hardware.

---

# 🧪 Testing

The current system has been tested in QEMU using both BIOS and UEFI firmware.

Implemented and tested functionality includes:

* [x] Linux kernel boot
* [x] Senbit init
* [x] Root filesystem mounting
* [x] `/proc` mounting
* [x] `/sys` mounting
* [x] `/dev` mounting
* [x] `/run` mounting
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
* [x] Hostname configuration
* [x] Locale configuration
* [x] Timezone configuration
* [x] DHCP networking
* [x] Root account creation
* [x] User account creation
* [x] Password authentication
* [x] Keyboard layout persistence
* [x] User shell startup
* [x] QEMU BIOS testing
* [x] QEMU UEFI testing

Current areas still being developed:

* [ ] Complete TTY/session management
* [ ] Proper shell job control
* [ ] Service manager
* [ ] Static networking
* [ ] SSH
* [ ] System update implementation
* [ ] Recovery mode

---

# 🧩 Multi-Architecture

Senbit is designed to support multiple architectures suitable for server workloads.

The initial development priority is:

```text
x86_64
```

Additional architectures are planned when they provide practical value for server deployments.

The project should avoid unnecessary architectural assumptions that would make future portability difficult.

---

# 🔄 Updates

Senbit is designed around reliable system updates.

The current userspace contains the initial update infrastructure and version detection, while the complete update mechanism remains under development.

Planned capabilities include:

## 🌐 Online updates

Systems should eventually retrieve updates from configured sources through the network.

## 💾 Offline updates

Systems should also be able to receive updates without an Internet connection.

For example, an administrator could provide a new Senbit image or update media through a USB drive.

## 🧩 Component updates

The update infrastructure is intended to replace only the components that actually changed whenever possible.

A kernel update should not unnecessarily require unrelated system components to be replaced.

---

# ↩️ Snapshots & Recovery

Senbit is designed around the idea that important system changes should be recoverable.

Future snapshot infrastructure may support:

```text
Current System
      │
      ▼
   Snapshot
      │
      ▼
    Update
      │
      ├───────────────┐
      │               │
      ▼               ▼
   ✅ Success       ❌ Failure
      │               │
      ▼               ▼
 Continue          Rollback
```

Planned capabilities include:

* Automatic snapshots
* Snapshot retention
* Configurable snapshot storage
* Rollback
* Failed-update recovery
* Boot recovery

---

# 🛡️ Security

Security is a core Senbit objective.

A fresh installation should contain only what is necessary for the operating system itself.

Planned security infrastructure includes:

* 🔥 Firewall management
* 🔎 Security auditing
* 📊 System monitoring
* 🔏 File integrity verification
* 🧩 Per-application file tracking
* 🔐 Permission management
* 🔄 Update verification

Security functionality will be introduced progressively as the underlying system infrastructure matures.

---

# 📊 Monitoring & Auditing

Senbit is intended to provide administrators with useful information about the state of their systems.

Planned monitoring and auditing capabilities include:

* System activity
* Resource usage
* Service status
* Security events
* File integrity
* Software changes
* Package updates
* Configuration changes
* Administrative actions

The goal is to make the basic state of a server visible without requiring a large collection of unrelated tools for fundamental system information.

---

# 🏎️ Performance

Senbit approaches performance primarily through simplicity.

The goal is not to blindly optimize every component.

Instead, Senbit aims to reduce unnecessary software, services, overhead, and duplication while keeping the mature Linux kernel underneath.

```text
Minimal Base System
        +
Efficient Userspace
        +
Linux Hardware Support
        +
Efficient Updates
        +
Deployment Automation
        │
        ▼
Efficient Server Platform
```

Performance claims will eventually be backed by reproducible benchmarks.

---

# 🗺️ Roadmap

The roadmap focuses on capabilities rather than fixed release dates.

## Phase 0 — Foundation 🏗️

* [x] Repository structure
* [x] Build system
* [x] Development environment
* [x] Linux kernel integration
* [x] Senbit kernel configuration
* [x] Minimal root filesystem
* [x] Senbit init
* [x] QEMU boot
* [x] Console access
* [x] BIOS boot
* [x] UEFI boot
* [x] Installation system
* [x] User authentication
* [x] Basic networking
* [x] Interactive shell
* [ ] Complete TTY/session management
* [ ] Clean shutdown

## Phase 1 — Basic Server 🖥️

* [x] Networking
* [x] DHCP configuration
* [x] Storage initialization
* [x] Filesystem support
* [x] Users and groups
* [x] Basic system initialization
* [ ] Static networking
* [ ] SSH
* [ ] Service management

## Phase 2 — Package Ecosystem 📦

* [ ] Debian package compatibility
* [ ] APT integration
* [ ] Package installation
* [ ] Package removal
* [ ] Package updates
* [ ] Automatic update configuration

## Phase 3 — System Updates 🔄

* [ ] System update infrastructure
* [ ] Kernel updates
* [ ] Component-level updates
* [ ] Offline updates
* [ ] USB / installation-media updates
* [ ] Update verification
* [ ] Recovery mechanisms

## Phase 4 — Snapshots & Recovery ↩️

* [ ] Automatic snapshots
* [ ] Snapshot retention
* [ ] Configurable snapshot storage
* [ ] Rollback
* [ ] Failed-update recovery
* [ ] Boot recovery

## Phase 5 — Security & Monitoring 🛡️

* [ ] Firewall integration
* [ ] Security auditing
* [ ] System monitoring
* [ ] File integrity tracking
* [ ] Application-level file association
* [ ] Integrity verification
* [ ] Security event management

## Phase 6 — Deployment 🚀

* [ ] Configuration-based installation
* [ ] Automated system configuration
* [ ] Automated package installation
* [ ] Reproducible deployment
* [ ] Server fleet deployment
* [ ] Remote administration improvements

## Phase 7 — Multi-Architecture 🌍

* [x] x86_64
* [ ] ARM64
* [ ] Additional server architectures
* [ ] Architecture-specific optimizations

---

# 🔭 Long-Term Vision

Senbit's long-term objective is to provide more than a minimal Linux distribution.

The goal is to create a **server operating platform** where the complete lifecycle of a machine can be managed predictably:

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
            ▼
         Recover
            │
            ▼
       Deploy Again
```

Senbit should make these operations:

* Predictable
* Automatable
* Observable
* Recoverable
* Suitable for both individual servers and large infrastructures

The project will continue to build around the Linux ecosystem instead of replacing it.

---

# 🤝 Contributing

Senbit is built in the open.

Contributions are welcome, including:

* 💻 Code
* 🐧 Linux kernel work
* 🦀 Rust development
* 🧪 Testing
* 📚 Documentation
* 🐛 Bug reports
* 💡 Ideas
* 🔐 Security research
* 🏗️ Architecture discussions

Before contributing, please read:

👉 [`CONTRIBUTING.md`](CONTRIBUTING.md)

For security vulnerabilities:

👉 [`SECURITY.md`](SECURITY.md)

For community guidelines:

👉 [`CODE_OF_CONDUCT.md`](CODE_OF_CONDUCT.md)

---

# 💬 Community

GitHub Discussions is the main place for project discussions.

Available categories include:

* 📢 Announcements
* 💡 Ideas & Proposals
* 🛠️ Development
* ❓ Q&A
* 🐛 Help & Troubleshooting
* 💬 General

Technical discussions, architectural proposals, questions, and project ideas are welcome.

---

# 📜 License

Senbit's original code is licensed under the:

**Apache License 2.0**

The Linux kernel and third-party components retain their respective licenses.

Senbit can therefore contain components under different licenses depending on their origin.

See [`LICENSE`](LICENSE) for the license covering Senbit's original code.

---

# ⭐ Support Senbit

If you are interested in the project:

⭐ Star the repository
💬 Join the Discussions
🐛 Report bugs
💡 Propose ideas
🧑‍💻 Contribute code
📚 Improve the documentation

---

<p align="center">

🦀 **Senbit**

<i>Server infrastructure, built around Linux.</i>

</p>
