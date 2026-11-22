# Roadmap

> Part of the [Senbit documentation](../README.md#documentation).

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
* [x] Service definition format (systemd unit files, see [SERVICES.md](SERVICES.md))
* [x] Service manager (systemd, started by Senbit's init)
* [x] Service dependencies (`Requires=`, `Wants=`, `After=`, `Before=`)
* [x] Service lifecycle management (`systemctl start|stop|restart|enable|disable|status`, `journalctl -u`)
* [x] `systemctl` (the real one; works for normal users thanks to D-Bus)
* [x] Standard administration commands, no Senbit-specific wrapper (`systemctl`, `journalctl`, `ip`, `apt`, `adduser`/`useradd`, `getent`)
* [x] Static networking (asked by the installer, `/etc/network/interfaces`)
* [x] SSH (`apt install openssh-server`: installed and started by the Debian package scripts)

## Phase 2 — Package Ecosystem 📦

* [x] Define Senbit package format (the Debian `.deb`, see [PACKAGES.md](PACKAGES.md))
* [x] Study APT/dpkg reuse (reused as is: [PACKAGES.md](PACKAGES.md))
* [x] Debian package compatibility (merged-/usr, glibc runtime, maintainer-script tools, system accounts, D-Bus)
* [x] Package installation (`apt install curl postgresql`)
* [x] Package removal (`apt remove`, `apt purge`, `apt autoremove`)
* [x] Package updates (`apt update`, `apt upgrade`; the package lists are downloaded automatically at the first boot)
* [x] Dependency management (apt's resolver, `apt --fix-broken install`)
* [x] Repository management (`/etc/apt/sources.list.d/*.sources`)
* [x] Package signatures (repositories verified by apt with GPG keys, `Signed-By:`)
* [x] Standard `apt` / `dpkg` as the package manager (no Senbit-specific wrapper)

## Phase 3 — System Updates 🔄

Updates go through the real `apt` ([UPDATES.md](UPDATES.md)). **Written and reviewed, not tested in a VM yet.**

* [x] Version management (`Cargo.toml` → `/etc/senbit-release`, packages `senbit-kernel` and `senbit-userspace`)
* [x] Update detection (daily timer, notice at login: `bootreport`)
* [x] Update download (`apt update && apt upgrade`)
* [x] Integrity verification (SHA-256 of the apt indexes)
* [x] Authenticity verification (GPG-signed repository, `Release.gpg` / `InRelease`)
* [x] Snapshot before update (`/var/lib/senbit/snapshots`, last 3 kept)
* [x] Component-level updates (kernel/init and userspace are separate packages)
* [x] Offline updates (`apt install ./x.deb` or a `file:` repository)
* [x] Automatic rollback at boot (GRUB attempt counter, 3 tries)
* [x] No Senbit-specific update command: `apt upgrade` is the update command

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
