# Building Senbit

Works on **Fedora (KDE)**, **Ubuntu / Debian** and **Arch**. Nothing depends on
where the repository is cloned: every path is relative to the project root.

```bash
./scripts/devtool.sh        # compile devtool -> scripts/devtool (needs cargo)
./scripts/devtool doctor    # what is missing on this machine?
./scripts/devtool install   # install it (apt / dnf / pacman) + Rust musl target
./scripts/devtool get       # download the pinned sources
./scripts/devtool build     # build everything (kernel ... ISO)
./scripts/devtool run       # boot in QEMU   (--debug writes build/vm/qemu.log)
```

`./scripts/devtool.sh <command>` rebuilds devtool if its sources changed, then
runs the command.

## Pinned versions

Nothing follows "latest". Each component has `config/<name>/version`
(and `sha` or `sha256`); `devtool get` refuses a commit / archive that does not match.

| Component  | Version   | Why |
|------------|-----------|-----|
| Linux      | 6.18.60   | LTS branch (supported until Dec 2028), from the *stable* tree |
| systemd    | v257.12   | Branch used by Debian 13, hence by the `.deb` packages installed with apt |
| BusyBox    | 1.38.0    | the version Senbit's `busybox.config` was made for |
| util-linux | 2.41.6    | latest 2.41.x bug-fix release |
| Parted     | 3.6       | official GNU release archive, sha256 verified |
| apt/dpkg   | Debian 12 "bookworm" repository | glibc 2.36 binaries run on any newer host glibc |

To upgrade one: edit its `version` (and `sha`), then `devtool get <name>`.
The systemd build reads `meson_options.txt` / `meson.options` of the source and
silently skips the options that version does not know, so changing the systemd
version does not break the build.

All URLs and the Debian suite are in `config.toml`.

## Patches

`patches/<component>/NNNN-name.patch`, applied automatically, idempotent.
See `patches/README.md`.

## apt / dpkg: `sudo apt install ...`

`devtool build apt` downloads the official `.deb` of apt, dpkg and their
libraries from the Debian mirror in `config.toml` (`[apt]`). The package index
is checked against `InRelease` (signature verified when `gpgv` and
`debian-archive-keyring` exist on the build machine) and every `.deb` against
its sha256. In the rootfs:

* the dpkg database lists them as installed; glibc, systemd, util-linux, udev
  (supplied by Senbit itself) are declared as installed stubs, and
  `/etc/apt/preferences.d/senbit` forbids replacing them;
* `/etc/apt/sources.list.d/senbit.sources` points to the remote repository
  (`deb.debian.org`, `bookworm`, `bookworm-updates`, `bookworm-security`);
* the `_apt` user (sandboxed downloads) comes from `sysusers.d/senbit.conf`;
* the TLS certificate bundle and the library cache are generated.

Then, in Senbit (network up): `sudo apt update && sudo apt install <package>`.

Maintainer scripts (`postinst`...) are not run for the packages bundled at build
time; dpkg runs them normally for everything installed afterwards.

## What makes `apt install <anything>` work

`apt install curl postgresql` downloads Debian 12 packages and runs their
maintainer scripts (`preinst`, `postinst`...) inside Senbit. The rootfs is
built so that those scripts find what they expect:

| Need of Debian packages | How Senbit provides it |
|---|---|
| merged-`/usr` layout (`/bin`, `/sbin`, `/lib`, `/lib64` are links to `/usr/...`) | created by `build::rootfs` (checked by `verify_rootfs`) |
| glibc: `libc`, `libm`, `libresolv`, `libnss_*`, `ld.so`... | copied from the build host (`apt::install_glibc_runtime`); `libc6` / `libc-bin` are declared installed |
| `libgcc-s1`, `libcrypt1`, `libselinux1`, `libstdc++6`... | real Debian packages (resolved from the dependencies) |
| `perl`, `debconf`, `init-system-helpers` (`deb-systemd-helper`, `invoke-rc.d`, `update-rc.d`) | real Debian packages (`[apt] bootstrap` in `config.toml`) |
| GNU `coreutils`, `sed`, `grep`, `findutils`, `tar`, `gzip`, `bash`, `diffutils`, `debianutils`, `hostname`, `ncurses` | real Debian packages (they replace the BusyBox applets of the same name) |
| `adduser`, `addgroup`, `deluser`, `useradd`, `groupadd`, `usermod`... | native commands `commands/adduser` and `commands/useradd` |
| `getent` | native command `commands/getent` |
| `findmnt` | native command `commands/findmnt` (util-linux is only built for `fdisk`/`sfdisk`) |
| `ldd`, `getconf` (installers detect glibc / the number of CPUs with them) | shell scripts of the overlay (`rootfs/usr/bin`), they read `/usr/share/senbit/glibc-version` |
| `systemctl enable docker` (units that also ship a SysV script) | `rootfs/usr/lib/systemd/systemd-sysv-install`, a wrapper around `update-rc.d`; `/etc/init.d` and `/etc/rc?.d` exist |
| `apt install gcc clang build-essential make` | `libc6` is declared installed with the repository's version (`2.36-9+deb12uN`), so `libc6-dev`'s exact dependency is satisfied; the glibc libraries are also linked in `/usr/lib/x86_64-linux-gnu` where its linker scripts look for them |
| `dummy0` taken for a network card | `CONFIG_DUMMY` is forced off in the kernel; the installer and the init only keep interfaces that have a kernel device (`utils/network.rs`) |
| Docker (`docker.io`) | kernel options for overlayfs, bridge, veth, netfilter/nftables, cgroups (`devtool/src/build/kernel.rs`): the kernel is rebuilt when the list changes |
| `dpkg: warning: files list file for package 'x' missing` | an empty `.list` file for every package Senbit provides; minimal `/etc/dpkg/dpkg.cfg`, `diversions`, `statoverride` |
| `ldconfig.service` | systemd's own unit, made unconditional by a drop-in: the library cache is rebuilt at every boot |
| colors and pager | `/etc/profile.d/senbit.sh`: colored prompt for bash and ash, `--color=auto` aliases, `SYSTEMD_COLORS`, GNU `less` (package `less`) so `systemctl status` no longer prints a screen of `~` |
| system accounts (`_apt`, `www-data`, `daemon`, `mail`...) | `rootfs/usr/lib/sysusers.d/senbit.conf`, applied at every boot (drop-in `systemd-sysusers.service.d/10-senbit.conf`) |
| a running systemd (`systemctl start` from `postinst`) | systemd is PID 1; services are started by the package scripts |
| `systemctl status` for a normal user | package `dbus` (bootstrap), `dbus.socket` enabled at build time, user `messagebus` |
| package lists (`apt install` -> "Unable to locate package") | `senbit-apt-update.service` downloads them at the first boot with network, retries until it works, never blocks the boot |
| mount units (`/usr/sbin/mount`) | links created for BOTH `/usr/bin` and `/usr/sbin` (the path compiled into systemd depends on the build host) |
| network (`eth0`) | `net.ifnames=0` on the kernel command line + `etc/systemd/network/00-senbit.link` |
| files owned by root | the initramfs is packed with `cpio -R 0:0`, extraction keeps setuid bits (`tar -p`) |

Not supported: packages tied to Debian's own systemd/glibc runtime (`systemd-sysv`,
`locales`...; pinned to -1 by `/etc/apt/preferences.d/senbit`). Compilers and
`-dev` packages work (see the table). The build host needs
glibc >= 2.36 (Ubuntu 24.04, Fedora, Arch, Debian 12+): `devtool` warns otherwise.

After changing `[apt] bootstrap` or `PROVIDED`, run `devtool build apt` again
(the stage is re-downloaded because its fingerprint changed).

## Publishing

Sources (`kernel/linux`, `third_party`, `external`), `build/` and the compiled
`scripts/devtool` are ignored by Git. If this repository once tracked the
sources as submodules, run once:

```bash
./scripts/git-unify.sh
```
