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

Working (developed and tested in QEMU, BIOS and UEFI):

* 🐧 Linux kernel, 🦀 `senbit-init` (transition from the initramfs) and systemd as PID 1
* 💿 Installer that asks everything first and installs after a final confirmation (BIOS/UEFI, GPT/MBR, DHCP or static network)
* 👤 Login, users, native `sudo`, power commands, `log`, `findmnt`, `adduser`/`useradd`, `getent`
* 🩺 Non-blocking boot with a boot report (`bootreport`): a network failure never stops the boot
* 📦 `apt install <anything>` (Debian 12 packages) and systemd services (`systemctl`, `journalctl`)
* 🔄 System updates with the real `apt upgrade`, snapshot before each update, automatic rollback after
  3 failed boots and the `rollback` command ([docs/UPDATES.md](docs/UPDATES.md))

Website: <https://senbit.pages.dev>

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
| [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) | layers, components, boot sequence, native command system |
| [docs/INSTALLER.md](docs/INSTALLER.md) | installation flow, BIOS and UEFI |
| [docs/API.md](docs/API.md) | the system API used by the commands, the `panic` command |
| [docs/SYSTEM.md](docs/SYSTEM.md) | login, `sudo`, network, boot report, logs, configuration |
| [docs/SYSTEMD.md](docs/SYSTEMD.md) · [docs/SERVICES.md](docs/SERVICES.md) | systemd integration and writing services |
| [docs/PACKAGES.md](docs/PACKAGES.md) | `apt`/`dpkg` on Senbit |
| [docs/UPDATES.md](docs/UPDATES.md) | system updates, snapshot, rollback, update repository |
| [docs/BUILD.md](docs/BUILD.md) · [docs/DEVELOPMENT.md](docs/DEVELOPMENT.md) | pinned versions, patches, build pipeline, tests |
| [devtool/README.md](devtool/README.md) ([fr](devtool/README.fr.md)) | the build tool |
| [commands/INDEX.md](commands/INDEX.md) | every native command |
| [docs/ROADMAP.md](docs/ROADMAP.md) | all phases and the long-term vision |

## Roadmap

Organized by capability, not by date. Priorities: recovery, security, declarative deployment, real hardware.

| Version | Subject | State |
|---|---|---|
| 0.1 – 0.4 | Foundation, packages (`apt`), systemd, updates and rollback | done |
| 0.5 | Recovery: recovery boot, `snapshot`, system API and `panic`, repair from the ISO | in progress |
| 0.6 | Security: firewall, exposure report, file integrity | planned |
| 0.7 | Declarative deployment: unattended install, VM images | planned |
| 0.8 | Real hardware: first physical x86_64 machine | planned |

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
