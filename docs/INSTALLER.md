# Installer

The Senbit ISO boots on **BIOS and UEFI**. It starts an interactive installer when no installation is found.

```text
                 Senbit ISO
                     │
          ┌──────────┴──────────┐
          ▼                     ▼
        BIOS                  UEFI
     GRUB i386-pc         GRUB x86_64-efi
          └──────────┬──────────┘
                     ▼
               Senbit Linux
```

## How an installation goes

The installer asks **everything first and writes nothing**, then installs after a final confirmation.

1. **Questions**: keyboard, disk, partitioning, root and user accounts, hostname, language, timezone,
   network (DHCP, or static address, gateway and DNS).
2. **Summary**: everything that will be done. *Nothing has been written to the disk yet.*
3. **Decision**: cancel (the default), start over, or install.
4. **Installation**: about fourteen steps, from creating the partitions to writing everything to the disk.
   Each step is displayed as it runs, with its duration.

Cancelling or starting over before the confirmation never modifies the disk.

## What it can do

| Area | Details |
|---|---|
| Disks | Detects the boot mode, the disks and the existing partitions. Automatic partitioning, manual partitioning with `fdisk`, or an existing partition. |
| BIOS | MBR partition table, ext4 root, GRUB with the `i386-pc` target. |
| UEFI | GPT partition table, FAT32 EFI System Partition, ext4 root, GRUB with the `x86_64-efi` target. |
| Accounts | `root` and a main user, in `/etc/passwd`, `/etc/shadow` and `/etc/group`, with hashed passwords. |
| Settings | Hostname, locale, timezone, keyboard layout, network. Written to the target system and restored at every boot. |
| Network | The real network card is detected (a single card is selected without asking). DHCP or static: `/etc/network/interfaces` and `/etc/resolv.conf`. |
| Bootloader | GRUB, with the boot menu written by `/usr/lib/senbit/grub-config`: the installed system already has the boot counter that makes the automatic rollback work (see [UPDATES.md](UPDATES.md)). |

After the installation, the system boots on its own: see [ARCHITECTURE.md](ARCHITECTURE.md) and
[SYSTEM.md](SYSTEM.md).
