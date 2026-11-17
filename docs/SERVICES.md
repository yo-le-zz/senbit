# Services on Senbit (Phase 1)

Senbit's init (`senbit-init`) prepares the machine, mounts the installed system
and `exec()`s **systemd** as PID 1. There is no Senbit-specific service tool: the
standard commands are used.

| Roadmap item | Senbit's answer |
|---|---|
| Service definition format | systemd **unit files** (`/usr/lib/systemd/system/*.service`, overrides in `/etc/systemd/system/`) |
| Service manager | systemd (PID 1) |
| Service dependencies | `Requires=`, `Wants=`, `After=`, `Before=`, `Conflicts=` in the unit |
| Service lifecycle | `systemctl start\|stop\|restart\|enable\|disable\|status`, `journalctl -u <service>` |
| `systemctl` | the real one, from systemd |
| Static networking | asked by the installer; `/etc/network/interfaces` |
| SSH | `apt install openssh-server` |

Packages installed with `apt install` (postgresql, nginx, ssh...) bring their own unit files and
are started by their package scripts.

## Define a service

`/etc/systemd/system/hello.service`:

```ini
[Unit]
Description=Hello service
After=network.target

[Service]
ExecStart=/usr/local/bin/hello
Restart=on-failure

[Install]
WantedBy=multi-user.target
```

```bash
sudo systemctl daemon-reload
sudo systemctl enable --now hello     # start now AND at every boot
systemctl status hello                # works for a normal user too (D-Bus)
journalctl -u hello -f                # logs (root or the systemd-journal group)
```

## Boot report

If a step of the boot failed (network...), the system starts anyway and a box is printed at login:

```bash
bootreport            # what worked, what failed, failed units
bootreport --logs     # systemd journal of this boot
bootreport --failed   # failed units
```

## Networking

The installer detects the real network card (a card with a kernel device: never `lo`, `dummy0`, bridges, `veth*`,
`docker0`), selects it automatically when it is the only one, and writes its real name in the files below.
At boot `senbit-init` reads `/etc/network/interfaces` and runs `ifup` on each detected interface that the file
describes (the others are ignored); the unit `senbit-dhcp@<interface>.service` keeps the DHCP lease alive. Interface
names stay `eth0`, `eth1`... (`net.ifnames=0` + `00-senbit.link`).

DHCP:

```text
auto eth0
iface eth0 inet dhcp
```

Static (the installer writes this and `/etc/resolv.conf`):

```text
auto eth0
iface eth0 inet static
    address 192.168.1.10
    netmask 255.255.255.0
    gateway 192.168.1.1
```

To switch an installed system to a static address:

```bash
sudoedit /etc/network/interfaces                  # as above
echo "nameserver 1.1.1.1" | sudo tee /etc/resolv.conf
sudo systemctl disable --now senbit-dhcp@eth0     # no DHCP client for a static interface
sudo ifdown eth0; sudo ifup eth0
```

Back to DHCP: `iface eth0 inet dhcp`, then `sudo systemctl enable --now senbit-dhcp@eth0`.

## SSH

```bash
sudo apt install openssh-server       # installs and starts the ssh service
systemctl status ssh
```

Keys and `sshd_config` are the standard OpenSSH ones (`/etc/ssh/`).
