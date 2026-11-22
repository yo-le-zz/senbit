# System updates (Phase 3, Senbit 0.4.0)

Senbit has **no update command of its own**. The system is updated with the real `apt`, exactly like
any other package: the components of Senbit are Debian packages published in a signed APT repository.

```bash
sudo apt update
sudo apt upgrade
```

## Components

| Package | Contents | Why separate |
|---|---|---|
| `senbit-kernel` | `/boot/vmlinuz` and `/boot/initramfs.cpio.gz` (kernel + `senbit-init`) | the only part that can prevent the boot, so the only one with a rollback at boot |
| `senbit-userspace` | `senbit-login`, the native commands (`commands/`), systemd units, `/usr/lib/senbit/*` scripts, the overlay `rootfs/` | can be updated without touching the kernel |

Both packages are declared installed in `/var/lib/dpkg/status` (see `devtool/src/build/apt.rs`), so
`apt upgrade` sees a new version as soon as a repository provides it. The version of a package is the
version of Senbit (`Cargo.toml`) plus a revision: `0.4.0` gives `0.4.0-1`. `/etc/senbit-release` is
rewritten by the `postinst` of `senbit-userspace`.

Files under `/etc` shipped by `senbit-userspace` are *conffiles*: an update never replaces a file the
administrator changed (`Dpkg::Options { "--force-confold"; }` in `/etc/apt/apt.conf.d/10senbit`).

## What happens around an update

| Step | Mechanism |
|---|---|
| Detection | `senbit-update-check.timer` runs `/usr/lib/senbit/update-check` (apt) every day; the result is in `/var/lib/senbit/updates-available`. |
| Notice | `bootreport` shows `^ N update(s) available ... sudo apt upgrade` at login and in the full report. Nothing is asked at boot. |
| Download + integrity | apt, with the SHA-256 sums of the signed `Release` and `Packages` files. |
| Authenticity | GPG: `Release.gpg` / `InRelease`, key `/usr/share/keyrings/senbit-archive-keyring.gpg` (`Signed-By:`). |
| Snapshot | `DPkg::Pre-Invoke` runs `/usr/lib/senbit/update-snapshot`: dpkg status, versions of `senbit-*`, boot files list, in `/var/lib/senbit/snapshots/<date>` (the last 3 are kept). The `.deb` files stay in `/var/cache/apt/archives`. |
| Previous boot files | the `preinst` of `senbit-kernel` copies `/boot/vmlinuz` to `/boot/vmlinuz.old` and the initramfs to `/boot/initramfs.cpio.gz.old`. |
| Notice refresh | `DPkg::Post-Invoke` runs `update-check --offline`. |

## Automatic rollback at boot

`/usr/lib/senbit/grub-config UUID` writes `/boot/grub/grub.cfg` (the installer and the `senbit-kernel`
`postinst` both call it):

1. GRUB increments `boot_tries` in `/boot/grub/grubenv` at every boot attempt.
2. `senbit-boot-ok.service` runs once `senbit-login.service` is up and removes `boot_tries`
   (`/usr/lib/senbit/boot-ok`).
3. When the counter reaches **3** (the system did not come up three times in a row) and
   `/boot/vmlinuz.old` exists, GRUB boots **"Senbit (previous version)"** by default and passes
   `senbit.fallback=1` on the kernel command line.
4. Once that boot works, `boot-ok` runs `rollback --commit`: the previous version becomes the
   current one (see below), so the next boots stay on it instead of trying the faulty version again.

The "previous version" entry is always in the menu. Choosing it by hand does **not** commit anything.

## The `rollback` command

```bash
sudo rollback            # shows the state, then asks "Go back to the previous version? [y/N]"
sudo rollback --yes      # without the question
rollback --status        # only the state
sudo reboot
```

`rollback` swaps the boot files of the two versions: `/boot/vmlinuz` and `/boot/initramfs.cpio.gz`
become the previous ones, and the version you leave is kept as `.old`. Running `rollback` again goes
**forward** to it. After a rollback `senbit-kernel` is put on hold (`apt-mark hold`), so `apt upgrade`
does not reinstall the faulty version; release it with `sudo apt-mark unhold senbit-kernel` once a
fixed version is published (going forward with `rollback` releases it too).

The packages are not downgraded: the old `.deb` files are usually not in the repository any more, and
the boot files are what the rollback is about.

## Publishing: build the repository

```bash
./scripts/devtool build            # everything needed (kernel, initramfs, commands, login)
./scripts/devtool build package    # -> build/repo/
```

```text
build/repo/
├── pool/main/senbit-kernel/senbit-kernel_<version>_amd64.deb
├── pool/main/senbit-userspace/senbit-userspace_<version>_amd64.deb
└── dists/<suite>/
    ├── Release  Release.gpg  InRelease        (the last two only if signed)
    └── main/binary-amd64/Packages  Packages.gz
```

No Debian tool is needed on the build machine: the `.deb` files are made with `tar`, `gzip` and `ar`.
The publication is a plain directory: any web server, or a USB stick (`file:` repository).

Configuration, in `config.toml`:

```toml
[update]
repository = "https://updates.example.org/senbit"   # "" = no update channel in the image
suite      = "stable"
component  = "main"
gpg_key    = "ABCD1234..."                           # "" = not signed (local tests only)
keyring    = "config/update/senbit-archive-keyring.gpg"
```

* Sign: `gpg_key` is the id of a secret key of the build machine (`gpg --list-secret-keys`).
* Public key shipped in the image: `gpg --export <key> > config/update/senbit-archive-keyring.gpg`,
  then rebuild the image. When `repository` is set, the image gets `senbit-core.sources` pointing to it.

## Publishing with GitHub Actions (GitHub Pages)

`.github/workflows/publish-updates.yml` builds Senbit, makes the packages and publishes `build/repo/`
on the GitHub Pages of the repository: `https://<owner>.github.io/<repository>`. No server of your own.

One-time setup:

1. Settings → Pages → Source: **GitHub Actions**.
2. Create a signing key without passphrase and put it in the secret `GPG_PRIVATE_KEY`
   (Settings → Secrets and variables → Actions):
   `gpg --armor --export-secret-keys <key>`. Without it the repository is published **unsigned**
   and apt will refuse it unless the source is marked `Trusted: yes`.
3. Raise the version in `Cargo.toml`, commit, then `git tag v0.4.1 && git push --tags`
   (or run the workflow by hand from the Actions tab).

The workflow writes the repository URL, the key id and the public key into `config.toml` for its own build
only; the images you build locally keep `[update] repository = ""` unless you set it yourself. To get an
image that already points to the repository, set `repository` to the same URL in your `config.toml` and
put the public key in `config/update/senbit-archive-keyring.gpg`.

The first run takes a long time (it compiles the kernel).

## Offline updates

```bash
sudo apt install ./senbit-userspace_0.4.1-1_amd64.deb ./senbit-kernel_0.4.1-1_amd64.deb
```

or copy `build/repo/` on a USB stick and add a `file:` source:

```text
# /etc/apt/sources.list.d/senbit-usb.sources
Types: deb
URIs: file:///mnt/usb/repo
Suites: stable
Components: main
Signed-By: /usr/share/keyrings/senbit-archive-keyring.gpg
```

## Trying an update locally

```bash
# on the host
cd build/repo && python3 -m http.server 8000
```

```text
# in Senbit (QEMU: the host is 10.0.2.2)
# /etc/apt/sources.list.d/senbit-test.sources
Types: deb
URIs: http://10.0.2.2:8000
Suites: stable
Components: main
Trusted: yes            # only when the repository is not signed
```

1. Install a first build, then raise the version in `Cargo.toml` and run `devtool build package` again.
2. In the VM: `sudo apt update && apt list --upgradable` (the two `senbit-*` packages), `sudo apt upgrade`.
3. Check `/var/lib/senbit/snapshots/`, `/boot/vmlinuz.old`, `bootreport` (the notice must disappear), reboot.
4. Rollback test: break `/boot/vmlinuz` of the new version, reboot three times, GRUB must select
   the previous version; after it booted, `rollback --status` must say "rolled back" and the next
   reboot must stay on the previous version. Then `sudo rollback` goes forward again.

## Checklist before a release

- `grub-config`, `rollback`, `boot-ok`: `sh -n`, then read the generated `grub.cfg` (`ROOT=/tmp/x sh grub-config <uuid>`).
- `dpkg-deb -I` and `dpkg-deb -c` on both `.deb` (on a Debian machine): control fields, conffiles, file modes.
- `apt-ftparchive`-style check: `Packages` sizes and SHA-256 match the files.
- `senbit-userspace` conffiles: only files meant to be edited by the administrator.
