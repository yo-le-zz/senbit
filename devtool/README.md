# devtool

`devtool` is the single build tool of Senbit (Rust): it checks the machine, downloads the **pinned**
sources, builds everything and boots it in QEMU. It works on Fedora, Ubuntu/Debian and Arch, and every
path is relative to the project root.

[🇫🇷 Version française](README.fr.md)

## Features

* **Colors and progress**: green `==>` for steps, yellow for "up to date / skipped", red for errors.
  Long builds show a spinner and, under it, one live line: the percentage when the tool prints one
  (`[12/345]`, `[ 45%]`), otherwise the number of lines, the elapsed time and the last line.
  The full output of a failed step is saved in `build/logs/` and printed.
* **Smart rebuild**: kernel, BusyBox, util-linux, Parted, systemd and the Rust userspace are rebuilt
  only when something changed (new commit, patch, local change, missing output). The state is in
  `build/.state/cache.json`; delete it or pass `--force` to rebuild everything.
* **Missing or empty sources are fetched by themselves**: a fresh `git clone` of Senbit leaves the
  source folders (`external/linux`, ...) empty. `devtool build` downloads what is missing (at the pinned
  version) before compiling; `autogen.sh` is run for util-linux when `configure` is missing or outdated.
* **Pinned versions**: `config/<component>/version` (+ `sha`/`sha256`). Nothing follows "latest".
  Patches are in `patches/<component>/` (see `patches/README.md`).
* **BusyBox config detection**: editing `config/busybox.config` is picked up automatically.
* **`--jobs`/`-j`** sets the number of parallel jobs (default: CPU count); **`--force`** clears the cache.
* **UEFI**: `BOOT=uefi` in `config/vm.config` boots QEMU with OVMF; the ISO is a hybrid BIOS+UEFI image.

## Build devtool

```bash
./scripts/devtool.sh            # compiles devtool and installs it as scripts/devtool
./scripts/devtool.sh --force    # rebuilds even if it looks up to date
```

Rust (`rustc`/`cargo`, or [rustup](https://rustup.rs)) is the only prerequisite. The project root is
found automatically (`--root PATH`, `$SENBIT_ROOT`, or by walking up from the executable and from the
current directory).

## Commands

```text
devtool doctor                 check this machine, print the exact install command
devtool install                install the missing packages (apt / dnf / pacman); not as root
devtool get [component]        download the pinned sources: all (default), linux, busybox,
                               util-linux, parted, systemd
devtool build [target]         build (see below)
devtool commands [list|index]  list the native commands / regenerate commands/INDEX.md
devtool run                    build, then boot in QEMU
```

### `devtool build [target] [-j N] [--force]`

Targets: `all` (default), `fast`, `kernel`, `busybox`, `util-linux`, `parted`, `systemd`, `apt`, `rust`,
`commands`, `tools`, `rootfs`, `iso`, `package`.

`all` builds, in this order: kernel → BusyBox → util-linux → Parted → systemd → apt/dpkg → Rust userspace
→ native commands (`commands/`, one crate per command, detected automatically: `./newcmds.sh <name>`)
→ tools (`tools/`, keymaps and fonts) → rootfs/initramfs → ISO.

* `fast`: only Rust userspace, commands, tools, rootfs and ISO (no kernel, BusyBox, util-linux,
  Parted, systemd). For quick iterations.
* `package`: the update packages `senbit-kernel` and `senbit-userspace` and their APT repository in
  `build/repo/` (needs a previous `devtool build`; not part of `all`). See [docs/UPDATES.md](../docs/UPDATES.md).
* `apt`: apt + dpkg from the remote Debian repository (see [docs/BUILD.md](../docs/BUILD.md)).

### `devtool run [--new] [--debug] [--fast] [--force] [-j N]`

Builds (target `all`, or `fast` with `--fast`), then boots Senbit in QEMU. `config/vm.config` (`KEY=value`):

```text
RAM=512M
CPUS=2
DISK=build/vm/senbit.qcow2
DISK_SIZE=10G
CPU=host            # QEMU CPU model: "host" exposes the real CPU instructions (needed by some binaries)
DISK_FORMAT=qcow2
NETWORK=user
BOOT=bios           # bios or uefi
```

* `--new`: delete the VM state and recreate the disk (test the installer from scratch).
* `--debug`: serial log in `build/vm/qemu.log`, UEFI diagnostics, OVMF/QEMU logs, `qemu-cmdline.txt`.
* `--fast`: rebuild only userspace/commands/tools/rootfs/ISO.

With `BOOT=uefi` the OVMF CODE/VARS pair is chosen as a coherent set and the NVRAM
(`build/vm/OVMF_VARS.fd`) is checked at every run (recreated if incompatible; the old one is kept as
`OVMF_VARS.fd.stale`).

## Troubleshooting

* **A step recompiles although nothing changed**: delete `build/.state/cache.json` or pass `--force`; check
  for uncommitted local changes in the sources.
* **`util-linux` / `parted` / `systemd` / kernel sources are empty or missing**: run `devtool get`
  (`devtool build` also does it by itself). The source folders must not be committed as nested git
  repositories: if `git submodule` complains about `external/systemd`, run
  `./scripts/git-unify.sh` (or by hand: `git rm --cached external/systemd kernel/linux third_party/busybox third_party/util-linux third_party/parted`).
* **`rootfs` fails on keymaps**: run `devtool build tools` first.
* **`BOOT=uefi`: "no OVMF firmware found"**: install `ovmf` (part of `devtool install`).
* **QEMU says "Guest has not initialized the display (yet)" and `qemu.log` is empty**: the UEFI firmware
  did not start. Run `devtool run --debug` (firmware pair, NVRAM state, EFI entry of the ISO);
  `devtool run --new` also resets the NVRAM. If a UEFI shell appears, install `grub-efi-amd64-bin` and
  `mtools` and rebuild the ISO.
* **A program says "CPU ISA level is lower than required"**: keep `CPU=host` in `config/vm.config`.
* **Docker fails on `iptables` / nftables**: the kernel must have been rebuilt with nftables IPv4/IPv6
  (`devtool build kernel`).
* **I need the whole output of a failed step**: `build/logs/`.
