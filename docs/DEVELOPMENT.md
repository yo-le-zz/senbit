# Development

> Part of the [Senbit documentation](../README.md#documentation).

# 🛠️ Development

Senbit provides a dedicated development tool:

```text
devtool
```

The development workflow manages the major components required to build and run Senbit.

## First setup (Fedora, Ubuntu/Debian, Arch)

Everything starts from one script, from any directory of the clone:

```bash
./scripts/devtool.sh        # compiles devtool -> scripts/devtool
./scripts/devtool doctor    # checks this machine, prints the install command
./scripts/devtool install   # installs missing packages (apt / dnf / pacman)
./scripts/devtool get       # downloads the PINNED sources and verifies them
./scripts/devtool build     # kernel, BusyBox, util-linux, Parted, systemd, apt, ISO
```

All paths are relative to the project root, so the repository works wherever
it is cloned. See [docs/BUILD.md](docs/BUILD.md) for versions, patches and apt.

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

Written and reviewed but **not yet tested in a VM** (0.4.0):

* [ ] `devtool build package` and the update repository (`senbit-kernel`, `senbit-userspace`)
* [ ] `apt upgrade` of the Senbit components, snapshot before the upgrade, updates notice at login
* [ ] Automatic rollback (GRUB boot counter) and the `rollback` command

Current development areas:

* [ ] Complete TTY/session management
* [ ] Proper shell job control
* [ ] Recovery mode
* [ ] Snapshots (Phase 4)
