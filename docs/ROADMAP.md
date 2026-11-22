# Roadmap

The roadmap is organized by **capability, not by date**: a version is released when its goal is reached.
Each milestone is small on purpose, and is finished only when:

- it works in a QEMU virtual machine, with BIOS and with UEFI firmware;
- it is documented in `docs/`;
- it is in the release notes (`docs/releases/`).

Priorities, set by the project owner: **recovery**, then **security**, then **declarative deployment**,
then **real hardware**. Until then, the target stays QEMU.

## Done

| Version | Subject |
|---|---|
| 0.1 – 0.3 | Foundation and server basics: build system (`devtool`), Linux kernel, `senbit-init`, BIOS and UEFI boot, installer, login, native commands, logging; real `apt` and `dpkg` with Debian 12 packages; systemd as PID 1, `systemctl`, `journalctl`, DHCP and static networking, SSH with `apt install openssh-server` |
| 0.4 | System updates: `senbit-kernel` and `senbit-userspace` packages, signed update repository on GitHub Pages, `apt upgrade`, snapshot before an update, automatic rollback after 3 failed boots, the `rollback` command |

## 0.5 — Recovery

**Goal: a Senbit that fails can be repaired without reinstalling.**

- [ ] **Recovery boot entry** in GRUB: boots to a root shell (systemd `rescue.target`)
- [ ] **Repair from the ISO**: the live system finds an installed Senbit, mounts it and gives a shell in it
- [ ] **`snapshot` command**: `snapshot create`, `list`, `show`, `restore` for the configuration (`/etc`), the package list and the boot files. The snapshots already taken before an update become usable with it
- [ ] **Filesystem check** after an unclean shutdown, before the root filesystem is used
- [ ] Complete TTY and session handling, and shell job control (left over from the foundation)
- [ ] Confirm Docker Engine networking in a VM (the kernel has the nftables and NAT options)

Decision to take before this milestone is closed: snapshots of the *whole disk* need a filesystem that can
do it (Btrfs, or LVM thin volumes). With ext4 only, the `snapshot` command covers configuration and boot
files, not data. The milestone is planned for ext4 only.

**Done when:** in a VM, a broken `/etc/fstab` or a deleted boot file is repaired from the recovery entry or
from the ISO, and a configuration change is undone with `snapshot restore`.

## 0.6 — Security

**Goal: a safe-by-default server, and a way to see what is exposed.**

- [ ] **Firewall**: nftables with a documented default ruleset (inbound denied except what the administrator
  opens; SSH when it is installed)
- [ ] **Exposure report**: listening ports and the service that owns each, shown by `bootreport`
- [ ] **File integrity**: Senbit packages ship checksums, and `dpkg --verify` on them is part of
  `bootreport`; a modified system file shows as a warning
- [ ] **Hardening defaults**: sysctl settings, stricter defaults for `sudo` and for SSH when installed
  (no root login), documented
- [ ] Opt-in automatic security updates (a systemd timer that runs `apt upgrade`; off by default)

**Done when:** a fresh install has a firewall, `bootreport` lists what listens on the network, and changing
`/usr/lib/senbit/rollback` (or any packaged file) is reported at the next login.

## 0.7 — Declarative deployment

**Goal: install the same server twice, without answering questions.**

- [ ] **Unattended install** from an answers file (disk, accounts, hostname, network, locale), read from the
  kernel command line or from a second disk
- [ ] **Export** of the answers of an installation to that file, so that a machine can be reinstalled the same way
- [ ] **VM disk image** (`devtool build image`): a ready-to-boot qcow2 with the answers file injected
- [ ] Published checksum for every ISO and image

**Done when:** one command creates a VM, installs Senbit with no question, and ends with a working login and
SSH.

## 0.8 — Real hardware

**Goal: boot and install on one physical x86_64 machine.**

- [ ] Boot the ISO from a USB stick on a real PC, in UEFI mode
- [ ] Review the kernel drivers: storage (SATA, NVMe, USB) and the common network cards
- [ ] Decide how firmware files (`linux-firmware`) are handled
- [ ] Document what is not supported (Secure Boot, to start with)
- [ ] A hardware test checklist, and the list of machines that were tried

**Done when:** a complete installation, a reboot, an update and a rollback were done on a physical machine.

## Later, not planned

Ideas, with no order and no date: ARM64 (kernel, userspace, images), other architectures, management of
several machines, remote administration, a declarative configuration of the running system (beyond the
install), Btrfs or LVM snapshots of the whole disk. They are taken up only when the milestones above are done.

## Always

- Documentation kept in line with the code, and release notes for every version.
- `cargo test` and the build in GitHub Actions stay green.
- No Senbit-specific command where a standard one exists: `rollback` and `snapshot` are the exceptions,
  because nothing standard does the job.
