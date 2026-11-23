# systemd integration

systemd is PID 1 of an installed Senbit. `senbit-init` only prepares the machine and `exec`s it; Senbit
adds a login service and a few units around it.

## Who does what

| Component | Role |
|---|---|
| `senbit-init` | Transition only: mount, prepare, `exec(systemd)`. Also contains the installer. |
| `senbit-login` | Console login, and the event socket `/run/senbit.sock`. Independent crate. |
| `senbit-login.service` | Starts the login on `/dev/console` and restarts it if it stops. |
| `senbit-login@<tty>.service` | Additional console logins (`tty2`, `tty3` enabled), started with `--no-events`: only the main login owns the API socket. |
| `senbit-dhcp@<interface>.service` | Keeps the DHCP lease alive. |
| `senbit-boot-ok.service`, `senbit-update-check.timer` | Update support: see [UPDATES.md](UPDATES.md). |

`senbit-init` no longer contains the login, the shell, the event socket or any service handling.

## Packages and units

- Package units are found in `/usr/lib/systemd/system`, `/etc/systemd/system` and
  `/usr/local/lib/systemd/system`; `/lib/systemd` is a link to `/usr/lib/systemd` (the `.deb` files install
  in `/lib/systemd/system`).
- `systemctl enable`, `disable`, `daemon-reload` and the maintainer scripts of packages work, because
  systemd is PID 1 and `/run/systemd/system` exists.
- Accounts and permissions are created by `usr/lib/sysusers.d/senbit.conf` (groups `shadow`, `sudo`,
  `nogroup`..., user `_apt`) and `usr/lib/tmpfiles.d/senbit.conf` (`/root` 0700, `/tmp` 1777, `/etc/shadow`
  0600, apt sandbox directories).
- `/etc/machine-id` is created empty at installation and filled at the first boot. `/etc/os-release` exists
  (`ID=senbit`).

## Checks in a VM

```bash
cat /proc/1/comm                  # systemd
systemctl is-system-running       # running (or degraded)
systemctl status senbit-login     # active (running)
journalctl -u senbit-login -b     # login logs
systemctl --failed                # failed units, if any
```

`degraded` means at least one standard unit failed (this root filesystem is minimal: no hwdb, for
example). `systemctl --failed` says which; units that are useless here can be masked in
`devtool/src/build/systemd.rs` (`MASKED_UNITS`).

## Why systemd starts without a clear error

The GRUB entries use `loglevel=3`, which hides systemd's detailed errors (written at level *error*): only
the final emergency message is visible. To make the cause visible:

- `senbit-init` mounts what systemd treats as mandatory (`/proc`, `/sys`, `/dev`, `/run`, `/dev/shm`,
  `/dev/pts`, `/sys/fs/cgroup` in cgroup v2) with the exact flags of systemd, and prints the errno itself on
  the console if one fails;
- systemd is started with `--log-target=console --log-level=err`, unless `systemd.log_level=` or
  `systemd.log_target=` is on the kernel command line;
- the kernel build enables the whole option set systemd needs and reports any option the kernel lacks;
- `./scripts/devtool run --debug` sends everything to `build/vm/qemu.log`.

## Build

`devtool build` compiles the Rust crates (static musl targets), installs `/sbin/senbit-login`, then
`verify_rootfs()` checks the final root filesystem: files required by systemd, the ELF interpreter, the
`ExecStart` of every `senbit-*` unit and the activation links.
