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

## 0.5 — Recovery (in progress)

**Goal: a Senbit that fails can be repaired without reinstalling.**

Written, **to be tested in a VM**:

- ● **Recovery boot entry** in GRUB: boots to a root shell (systemd `rescue.target`); it does not count as
  a failed boot
- ● **`snapshot` command**: `create`, `list`, `show`, `restore`, `delete` for the configuration (`/etc`), the
  package list and a record of the boot files; apt takes one before every upgrade
- ● **System API** ([API.md](API.md)): documented protocol, `ping` and `version` requests, a real
  **`panic`** (command and request), and a panic hook for every native command and for `senbit-init`
- ● **Repair from the ISO**: the ISO has an entry "Senbit (repair an installed system)": it finds the
  installed Senbit, checks its filesystem (fully on request), mounts it and gives a root shell inside it
- ● **Filesystem check** (`e2fsck -p`) of the root partition at every boot, before it is mounted; the result
  is in the boot report
- ● **`panic=10`** on every kernel command line: after a kernel panic the machine reboots by itself, so
  the boot counter and the rollback do their work

- ● **Consoles and job control**: two more logins on Alt+F2 and Alt+F3 (`senbit-login@tty2`, `@tty3`).
  Job control was already in the code (the shell gets its own session and controlling terminal): to be
  confirmed in a VM (`sleep 100`, Ctrl+Z, `bg`, `fg`, Ctrl+C)

Still to do:

- [ ] Confirm Docker Engine networking in a VM (the kernel has the nftables and NAT options)

Decision to take before this milestone is closed: snapshots of the *whole disk* need a filesystem that can
do it (Btrfs, or LVM thin volumes). With ext4 only, the `snapshot` command covers configuration and boot
files, not data. The milestone is planned for ext4 only.

**Done when:** in a VM, a broken `/etc/fstab` or a deleted boot file is repaired from the recovery entry or
from the ISO, a configuration change is undone with `snapshot restore`, and `sudo panic` ends with the
machine back on its feet.

## 0.6 — Security (in progress)

**Goal: a safe-by-default server, and a way to see what is exposed.** Details in [SECURITY.md](SECURITY.md).

Written, **to be tested in a VM**:

- ● **Firewall**: a default nftables ruleset (inbound denied, SSH and DHCP allowed), your own rules in
  `/etc/senbit/firewall.d/`, loaded by `senbit-firewall.service` once nftables is installed
- ● **Exposure report**: `bootreport --ports` lists the listening ports and the process behind each
- ● **File integrity**: the Senbit packages ship MD5 sums, `bootreport --verify` runs `dpkg --verify` on them
- ● **Hardening defaults**: sysctl settings (`50-senbit.conf`) and SSH defaults (no root login)
- ● **Opt-in automatic upgrades**: `senbit-auto-upgrade.timer`, off by default

Still to do:

- [ ] nftables installed by the installer, so that a fresh install already has the firewall
- [ ] Show the exposure and integrity results in the boot banner, not only on request
- [ ] Hardening of `sudo` (lockout after repeated failures)

**Done when:** a fresh install has a firewall, `bootreport` lists what listens on the network, and a modified
packaged file is reported at the next login.

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
