# Development

`devtool` builds Senbit and runs it in QEMU. Everything starts from one script, from any directory of the
clone (Fedora, Ubuntu/Debian and Arch are supported):

```bash
./scripts/devtool.sh        # compiles devtool -> scripts/devtool
./scripts/devtool doctor    # checks this machine, prints the install command
./scripts/devtool install   # installs the missing packages (apt / dnf / pacman)
./scripts/devtool get       # downloads the pinned sources and verifies them
./scripts/devtool build     # builds everything, down to the ISO
```

All paths are relative to the project root. Versions, patches and apt are explained in
[BUILD.md](BUILD.md); every option of the tool is in [devtool/README.md](../devtool/README.md).

## Run Senbit

```bash
./scripts/devtool run --force       # rebuild, then boot in QEMU
./scripts/devtool run --force --new # also start from a blank disk (tests the installer)
```

`config/vm.config` sets the memory, the CPUs, the disk, and `BOOT=bios` or `BOOT=uefi`.

## Build pipeline

```text
Linux kernel → BusyBox → util-linux, Parted → systemd → apt, dpkg
   → Rust userspace → native commands → tools (keymaps, fonts)
   → root filesystem → initramfs → ISO
```

Only what changed is rebuilt (`--force` rebuilds everything). The initramfs is recompressed only when the
root filesystem changed. `./scripts/devtool build package` also builds the update packages and the APT
repository (see [UPDATES.md](UPDATES.md)).

## Tests

Senbit is tested in QEMU, with BIOS and with UEFI firmware, and `cargo test` covers the Rust code.

Covered in the VM:

- boot: kernel, `senbit-init`, root filesystem, API filesystems, systemd as PID 1
- installer: detection, BIOS and UEFI installations, GPT and MBR, GRUB for both, fstab, hostname, locale,
  timezone, keyboard, users
- network (DHCP and static), login, `sudo`, power commands, logging, `log`, `bootreport`
- packages and services: `apt install`, systemd units
- updates: signed repository, `apt upgrade`, snapshot, automatic rollback and the `rollback` command

Still to do: complete TTY and session management, job control, recovery mode, snapshots.

## Contributing

Work on a branch and open a pull request: see [CONTRIBUTING.md](../CONTRIBUTING.md). The repository must
not contain nested Git repositories: the downloaded sources (`kernel/linux`, `external/`, `third_party/`)
and `build/` are ignored; `./scripts/git-unify.sh` cleans an old clone.
