# Packages on Senbit (Phase 2)

## Package format: the Debian `.deb`

Senbit does not define a new package format. The "Senbit package format" is the
Debian `.deb` (architecture `amd64`), managed by **dpkg** and **apt**:

* the database is `/var/lib/dpkg` (what is installed, which file belongs to which package);
* dependencies, conflicts and versions are resolved by apt;
* maintainer scripts (`preinst`, `postinst`, `prerm`, `postrm`) run on the machine;
* repositories are signed (GPG) and verified by apt before anything is installed.

### Study: why apt/dpkg are reused

| | Own format + manager | Reuse apt/dpkg (chosen) |
|---|---|---|
| Software available on day one | none | tens of thousands of packages (curl, postgresql, nginx...) |
| Dependency resolver | to write and maintain | proven |
| Signatures, pinning, holds | to write | provided |
| Cost for Senbit | very high | making the rootfs compatible (see [BUILD.md](BUILD.md)) |

There is **no Senbit-specific package command**: the real `apt`, `apt-get`, `apt-cache` and `dpkg` are used.
The packages that Senbit itself provides (glibc of the build host, systemd, util-linux, adduser...) are
declared installed in `/var/lib/dpkg/status` and pinned so apt never replaces them.

## Using it

```bash
sudo apt update                    # done automatically at the first boot (senbit-apt-update.service)
sudo apt install curl
sudo apt install postgresql        # installs AND starts the service
apt search <term>                  # apt show <package>
sudo apt remove <package>          # purge also deletes the configuration
sudo apt upgrade                   # also updates Senbit itself (docs/UPDATES.md)
dpkg -L <package>                  # files of a package
dpkg -S /usr/bin/curl              # package that owns a file
```

`apt install` answers "Unable to locate package" while the lists are empty: that happens only when the
network was down at boot. `bootreport` shows it, and `senbit-apt-update.service` retries every 30 seconds
until the network is back (or run `sudo apt update` yourself).

## Senbit's own updates

`senbit-kernel` and `senbit-userspace` are packages like the others, published in the Senbit repository
(`/etc/apt/sources.list.d/senbit-core.sources`, when the image was built with `[update] repository`).
`sudo apt upgrade` updates them; snapshot, rollback and publishing are described in [UPDATES.md](UPDATES.md).

## Repositories and signatures

The default repository is Debian 12 "bookworm" (`/etc/apt/sources.list.d/senbit.sources`,
URL and suite in `config.toml` `[apt]`), signed with `debian-archive-keyring`.

To add a repository, put its key in `/etc/apt/keyrings/` and describe it in `/etc/apt/sources.list.d/<name>.sources`:

```text
Types: deb
URIs: https://download.docker.com/linux/debian
Suites: bookworm
Components: stable
Architectures: amd64
Signed-By: /etc/apt/keyrings/docker.asc
```

`Signed-By:` makes apt verify the packages of that repository with that key only. Then `sudo apt update`.

## Limits

* Packages tied to Debian's own systemd/glibc runtime are refused (`systemd-sysv`, `locales`...).
  Compilers and development packages (`gcc`, `clang`, `build-essential`, `libc6-dev`) work: Senbit declares
  `libc6` with the repository's version and ships the headers of Debian's glibc 2.36 over its own newer runtime.
* Only `amd64`.
* The build host must have glibc >= 2.36 (the glibc of the build host is the one Senbit ships).
