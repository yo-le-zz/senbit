<p align="center">
  <img src="assets/branding/senbit-logo.svg" alt="Senbit" width="320">
</p>

<p align="center">
  <strong>A lightweight, secure, and server-first operating system built around the Linux kernel.</strong>
</p>

<p align="center">
  <a href="#status">Status</a> •
  <a href="#quick-start">Quick start</a> •
  <a href="#documentation">Documentation</a> •
  <a href="#roadmap">Roadmap</a> •
  <a href="#contributing">Contributing</a>
</p>

---

# 🦀 Senbit

**Senbit** is a general-purpose **server operating system based on the Linux kernel**, built on four principles:

> ⚡ Performance · 🪶 Lightweight · 🧱 Stability · 🛡️ Security

It provides a minimal Linux foundation with a **Rust-first userspace** (PID 1, login, native commands),
systemd for the services, and the real `apt`/`dpkg` for software. Senbit does not try to replace the
Linux ecosystem: it stays compatible with existing Linux software.

## Status

> **Senbit 0.4.0 is in early development.** It installs and boots in QEMU (BIOS and UEFI).

Working (tested in QEMU):

* 🐧 Linux kernel, 🦀 Rust PID 1 (`senbit-init`) that starts systemd
* 💿 Installer that asks everything first and installs after a final confirmation (BIOS/UEFI, GPT/MBR, DHCP or static network)
* 👤 Users and authentication, native `sudo`, power commands, `log`, `findmnt`, `adduser`/`useradd`, `getent`
* 🩺 Non-blocking boot with a boot report (`bootreport`): a network failure never stops the boot
* 📦 `apt install <anything>` (Debian 12 packages) and systemd services (`systemctl`, `journalctl`)

Written, **not tested in a VM yet** (0.4.0): system updates through `apt upgrade`, snapshot before an
update, automatic rollback at boot ([docs/UPDATES.md](docs/UPDATES.md)).

## Quick start

All paths are relative to the project root; works on Fedora, Ubuntu/Debian and Arch.

```bash
./scripts/devtool.sh              # compiles devtool -> scripts/devtool
./scripts/devtool doctor          # checks this machine, prints the install command
./scripts/devtool install         # installs the missing packages (apt / dnf / pacman)
./scripts/devtool get             # downloads the pinned sources
./scripts/devtool build           # kernel, BusyBox, util-linux, Parted, systemd, apt, ISO
./scripts/devtool run --force     # rebuilds and starts QEMU (add --new for a blank disk)
./scripts/devtool build package   # update repository: build/repo/ (see docs/UPDATES.md)
```

Add a native command: `./newcmds.sh mycmd` (see [commands/README.md](commands/README.md)).

## Documentation

| Document | Contents |
|---|---|
| [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) | layers, kernel, Rust userspace, native command system |
| [docs/INSTALLER.md](docs/INSTALLER.md) | BIOS/UEFI boot, installation flow |
| [docs/SYSTEM.md](docs/SYSTEM.md) | boot and init, IPC, `sudo`, logging, shutdown, networking, boot report, users, shell |
| [docs/SYSTEMD.md](docs/SYSTEMD.md) · [docs/SERVICES.md](docs/SERVICES.md) | systemd integration and writing services |
| [docs/PACKAGES.md](docs/PACKAGES.md) | `apt`/`dpkg` on Senbit |
| [docs/UPDATES.md](docs/UPDATES.md) | system updates, snapshot, rollback, update repository |
| [docs/BUILD.md](docs/BUILD.md) · [docs/DEVELOPMENT.md](docs/DEVELOPMENT.md) | pinned versions, patches, build pipeline, tests |
| [devtool/README.md](devtool/README.md) ([fr](devtool/README.fr.md)) | the build tool |
| [commands/INDEX.md](commands/INDEX.md) | every native command |
| [docs/ROADMAP.md](docs/ROADMAP.md) | all phases and the long-term vision |

## Roadmap

| Phase | Subject | State |
|---|---|---|
| 0 | Foundation (kernel, init, installer, login, logging) | done, TTY/session management left |
| 1 | Server userspace (services, `systemctl`, static network, SSH) | done |
| 2 | Package ecosystem (`apt`/`dpkg`) | done |
| 3 | System updates (`apt upgrade`, snapshot, rollback) | written, to test in VM |
| 4 | Snapshots and recovery | planned |
| 5 | Security and monitoring | planned |
| 6 | Deployment | planned |
| 7 | Multi-architecture | planned |

Details: [docs/ROADMAP.md](docs/ROADMAP.md).

## Contributing

Contributions are welcome: Rust, kernel, tests, documentation, bug reports, security research.
Read [`CONTRIBUTING.md`](CONTRIBUTING.md), [`SECURITY.md`](SECURITY.md) and
[`CODE_OF_CONDUCT.md`](CODE_OF_CONDUCT.md). The `main` branch is protected: work on a branch and open a pull request.

## License

Senbit's original code is licensed under the **Apache License 2.0** ([`LICENSE`](LICENSE)).
The Linux kernel and the third-party components keep their own licenses.

---

<p align="center">
  🦀 <strong>Senbit</strong><br>
  <i>Server infrastructure, built around Linux.</i>
</p>
