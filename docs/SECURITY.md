# Security

Senbit aims at a server that is safe by default and that shows what it exposes. This page lists what is in
place, how to change it, and what is not covered. Milestone 0.6 of the [roadmap](ROADMAP.md); written, to be
tested in a virtual machine.

## What is exposed: `bootreport --ports`

```bash
sudo bootreport --ports
```

```text
PROTO  ADDRESS                    REACHABLE   PROCESS
tcp    0.0.0.0:22                 network     sshd (412)
tcp    127.0.0.1:631              this machine cupsd (305)
udp    0.0.0.0:68                 network     udhcpc (190)

2 of 3 listening socket(s) can be reached from the network.
```

It reads `/proc/net/{tcp,tcp6,udp,udp6}` and finds the owner of each socket in `/proc/<pid>/fd`. Root sees
every process; another user only sees their own.

## Firewall

Senbit ships a default ruleset for **nftables**: inbound traffic is denied except answers to what the
machine started, the loopback, ping and IPv6 neighbour discovery, the DHCP client and SSH. Outbound traffic is
not restricted.

```bash
sudo apt install nftables
sudo reboot            # or: sudo systemctl start senbit-firewall
```

`nft` is not part of the base system, so the firewall starts only once nftables is installed:
`senbit-firewall.service` is skipped while `/usr/sbin/nft` does not exist, and the boot is not affected.

| File | Role |
|---|---|
| `/etc/senbit/firewall.nft` | The default ruleset (table `inet senbit`). Do not edit it |
| `/etc/senbit/firewall.d/*.nft` | Your own rules, loaded after it, in alphabetical order |
| `/usr/lib/senbit/firewall` | Checks the file (`nft -c`), then loads it; `firewall status` shows the rules |

To open a port, add a file such as `/etc/senbit/firewall.d/10-web.nft`:

```text
add rule inet senbit input tcp dport { 80, 443 } accept
```

then `sudo systemctl reload senbit-firewall`. A file with an error is refused as a whole: the rules in force
are not touched.

The table is separate from Docker's: Docker keeps managing its own rules (forwarding, NAT). The ruleset has
no `forward` chain on purpose.

## Integrity of the Senbit files: `bootreport --verify`

```bash
sudo bootreport --verify
```

Senbit's two packages (`senbit-kernel`, `senbit-userspace`) ship the MD5 sum of every file, and
`dpkg --verify` compares them with what is on the disk. Configuration files under `/etc` are not reported:
the administrator is allowed to change them. A modified, removed or re-owned file is listed, with the command
that reinstalls it.

The sums are known to dpkg once a package was installed through apt (the first `apt upgrade` of Senbit
installs them). Before that, `--verify` finds nothing to compare.

## Hardening defaults

| File | What it sets |
|---|---|
| `/usr/lib/sysctl.d/50-senbit.conf` | No ICMP redirects, no source routing, SYN cookies, loose reverse-path filtering, no broadcast ping answers, protected hardlinks/symlinks/fifos/regular files, randomized memory layout, kernel addresses hidden. `ip_forward` is left alone (Docker and routers set it) |
| `/etc/ssh/sshd_config.d/10-senbit.conf` | For `openssh-server`: no direct root login, 4 password attempts, no X11 forwarding |
| `sudo` | Asks the **root** password, five minutes of authentication, symbolic links refused in its timestamp directory |

Change a sysctl value in `/etc/sysctl.d/` (the same key there wins). For SSH, log in as a normal user, then
use `sudo`.

## Automatic upgrades (off by default)

```bash
sudo systemctl enable --now senbit-auto-upgrade.timer     # on
sudo systemctl disable --now senbit-auto-upgrade.timer    # off
```

Every day (with a random delay of up to one hour) it runs `apt-get update` then `apt-get upgrade`, keeping
the configuration files you changed. It upgrades **every** installed package, not only the security ones,
and a new kernel is used at the next reboot. Updates of Senbit itself are covered by the snapshot and the
rollback ([UPDATES.md](UPDATES.md)).

## Not covered yet

- nftables is not installed by the installer: a fresh install has no firewall until `apt install nftables`.
- No audit log of administrative actions, no file integrity for the files of Debian packages (use
  `debsums` for those).
- Secure Boot and disk encryption.
