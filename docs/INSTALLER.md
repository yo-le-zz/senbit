# Installer and boot modes

> Part of the [Senbit documentation](../README.md#documentation).

# 💿 BIOS & UEFI Boot

Senbit supports both major x86 boot modes.

### Legacy BIOS

The installer can create an MBR partition table and install GRUB using the:

```text
i386-pc
```

target.

### UEFI

The installer can create a GPT partition table with an EFI System Partition and install GRUB using:

```text
x86_64-efi
```

The Senbit ISO itself supports both BIOS and UEFI boot.

```text
                 Senbit ISO
                     │
          ┌──────────┴──────────┐
          ▼                     ▼
       BIOS                  UEFI
          │                     │
     GRUB i386-pc         GRUB x86_64-efi
          │                     │
          └──────────┬──────────┘
                     ▼
              Senbit Linux
```

---

# 💾 Installation System

Senbit includes an interactive installer capable of:

* Detecting the current boot mode
* Detecting available disks
* Detecting existing partitions
* Automatically partitioning a disk
* Manual partitioning through `fdisk`
* Creating GPT partitions for UEFI
* Creating MBR partitions for BIOS
* Creating EFI System Partitions
* Formatting EFI partitions as FAT32
* Formatting root partitions as ext4
* Installing the Senbit root filesystem
* Creating users
* Configuring passwords
* Configuring hostname
* Configuring locale
* Configuring timezone
* Configuring keyboard layout
* Configuring networking
* Installing the bootloader

Senbit can also be installed onto an existing partition.

## Installation flow (since 0.3.0)

The installer asks **everything first and writes nothing**, then installs **after a final confirmation**:

```text
1. Questions   keyboard, disk, partitioning, root/user accounts, hostname,
               language, timezone, network (DHCP or static address/gateway/DNS)
2. Summary     everything that will be done, "Nothing has been written to the disk yet"
3. Decision    Cancel (default) / Start over / INSTALL
4. Installation  [ 1/14] Creating the partitions ... [14/14] Writing everything to the disk
```

Cancelling or starting over before the confirmation never modifies the disk. Each step is
displayed as it runs, with its duration.

## After the installation

The installer writes the boot menu with `/usr/lib/senbit/grub-config` (the same script is used by the
`senbit-kernel` package), so a freshly installed system already has the boot counter that makes the
automatic rollback possible: see [UPDATES.md](UPDATES.md).
