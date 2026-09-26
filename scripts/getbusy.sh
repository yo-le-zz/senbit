#!/usr/bin/env bash

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

KERNEL_DIR="$ROOT_DIR/kernel/linux"
BUSYBOX_DIR="$ROOT_DIR/third_party/busybox"

BUILD_DIR="$ROOT_DIR/build"

KERNEL_BUILD_DIR="$BUILD_DIR/kernel"
BUSYBOX_BUILD_DIR="$BUILD_DIR/busybox"
SENBIT_BUILD_DIR="$BUILD_DIR/senbit"
ROOTFS_BUILD_DIR="$BUILD_DIR/rootfs"
ISO_BUILD_DIR="$BUILD_DIR/iso"

KERNEL_IMAGE="$KERNEL_BUILD_DIR/arch/x86/boot/bzImage"
BUSYBOX_BINARY="$BUSYBOX_BUILD_DIR/_install/bin/busybox"
INITRAMFS="$ROOTFS_BUILD_DIR/initramfs.cpio.gz"
ISO_IMAGE="$ISO_BUILD_DIR/senbit.iso"

JOBS="$(nproc)"

TOTAL_START="$(date +%s)"

# --------------------------------------------------
# Helpers
# --------------------------------------------------

format_time() {
    local elapsed="$1"

    local minutes=$((elapsed / 60))
    local seconds=$((elapsed % 60))

    printf '%02d:%02d' "$minutes" "$seconds"
}

run_step() {
    local name="$1"
    shift

    echo
    echo "========================================"
    echo "  $name"
    echo "========================================"

    local start
    local end

    start="$(date +%s)"

    "$@"

    end="$(date +%s)"

    echo
    echo "==> $name completed in $(format_time $((end - start)))"
}

# --------------------------------------------------
# Kernel
# --------------------------------------------------

build_kernel() {
    mkdir -p "$KERNEL_BUILD_DIR"

    if [[ ! -d "$KERNEL_DIR" ]]; then
        echo "Error: Linux kernel source not found:"
        echo "  $KERNEL_DIR"
        echo
        echo "Run:"
        echo "  ./scripts/getlinux.sh"
        exit 1
    fi

    echo "==> Preparing Linux kernel..."

    if [[ ! -f "$KERNEL_BUILD_DIR/.config" ]]; then
        echo "==> Creating x86_64 kernel configuration..."

        make \
            -C "$KERNEL_DIR" \
            O="$KERNEL_BUILD_DIR" \
            x86_64_defconfig
    fi

    echo "==> Building Linux kernel..."
    echo "    Jobs: $JOBS"

    make \
        -C "$KERNEL_DIR" \
        O="$KERNEL_BUILD_DIR" \
        -j"$JOBS"

    echo
    echo "Kernel:"
    echo "  $KERNEL_IMAGE"
}

# --------------------------------------------------
# BusyBox
# --------------------------------------------------

build_busybox() {
    mkdir -p "$BUSYBOX_BUILD_DIR"

    if [[ ! -d "$BUSYBOX_DIR" ]]; then
        echo "Error: BusyBox source not found:"
        echo "  $BUSYBOX_DIR"
        echo
        echo "Run:"
        echo "  ./scripts/getbusy.sh"
        exit 1
    fi

    echo "==> Preparing BusyBox..."

    if [[ ! -f "$BUSYBOX_BUILD_DIR/.config" ]]; then
        echo "==> Creating BusyBox configuration..."

        make \
            -C "$BUSYBOX_DIR" \
            O="$BUSYBOX_BUILD_DIR" \
            defconfig
    fi

    echo "==> Building BusyBox..."
    echo "    Jobs: $JOBS"

    make \
        -C "$BUSYBOX_DIR" \
        O="$BUSYBOX_BUILD_DIR" \
        -j"$JOBS"

    echo "==> Installing BusyBox into build directory..."

    rm -rf "$BUSYBOX_BUILD_DIR/_install"

    make \
        -C "$BUSYBOX_DIR" \
        O="$BUSYBOX_BUILD_DIR" \
        CONFIG_PREFIX="$BUSYBOX_BUILD_DIR/_install" \
        install

    echo
    echo "BusyBox:"
    echo "  $BUSYBOX_BINARY"
}

# --------------------------------------------------
# Root filesystem
# --------------------------------------------------

build_rootfs() {
    mkdir -p "$ROOTFS_BUILD_DIR"

    local rootfs="$ROOTFS_BUILD_DIR/root"

    echo "==> Preparing root filesystem..."

    rm -rf "$rootfs"

    mkdir -p \
        "$rootfs/bin" \
        "$rootfs/sbin" \
        "$rootfs/etc" \
        "$rootfs/proc" \
        "$rootfs/sys" \
        "$rootfs/dev" \
        "$rootfs/run" \
        "$rootfs/tmp" \
        "$rootfs/usr/bin" \
        "$rootfs/usr/sbin"

    # --------------------------------------------------
    # Senbit rootfs
    # --------------------------------------------------

    if [[ -d "$ROOT_DIR/rootfs" ]]; then
        rsync -a \
            "$ROOT_DIR/rootfs/" \
            "$rootfs/"
    fi

    # --------------------------------------------------
    # BusyBox
    # --------------------------------------------------

    echo "==> Installing BusyBox into root filesystem..."

    if [[ ! -d "$BUSYBOX_BUILD_DIR/_install" ]]; then
        echo "Error: BusyBox installation directory not found:"
        echo "  $BUSYBOX_BUILD_DIR/_install"
        exit 1
    fi

    rsync -a \
        "$BUSYBOX_BUILD_DIR/_install/" \
        "$rootfs/"

    # --------------------------------------------------
    # Init
    # --------------------------------------------------

    if [[ -f "$rootfs/sbin/init" ]]; then
        chmod +x "$rootfs/sbin/init"
    else
        echo "==> No Senbit init found."
        echo "    Creating temporary init..."

        cat > "$rootfs/sbin/init" <<'EOF'
#!/bin/sh

mount -t proc proc /proc
mount -t sysfs sysfs /sys
mount -t devtmpfs devtmpfs /dev

echo
echo "========================================"
echo "              SENBIT"
echo "========================================"
echo
echo "Senbit kernel booted successfully."
echo
echo "Userspace init is currently minimal."
echo

exec /bin/sh
EOF

        chmod +x "$rootfs/sbin/init"
    fi

    # --------------------------------------------------
    # Initramfs
    # --------------------------------------------------

    echo "==> Creating initramfs..."

    rm -f "$INITRAMFS"

    (
        cd "$rootfs"

        find . -print0 |
            cpio --null -ov --format=newc |
            gzip -9
    ) > "$INITRAMFS"

    echo
    echo "Initramfs:"
    echo "  $INITRAMFS"
}

# --------------------------------------------------
# ISO
# --------------------------------------------------

build_iso() {
    if [[ ! -f "$KERNEL_IMAGE" ]]; then
        echo "Error: kernel image not found:"
        echo "  $KERNEL_IMAGE"
        exit 1
    fi

    if [[ ! -f "$INITRAMFS" ]]; then
        echo "Error: initramfs not found:"
        echo "  $INITRAMFS"
        exit 1
    fi

    mkdir -p "$ISO_BUILD_DIR"

    local iso_root="$ISO_BUILD_DIR/root"

    echo "==> Preparing ISO..."

    rm -rf "$iso_root"

    mkdir -p "$iso_root/boot/grub"

    cp \
        "$KERNEL_IMAGE" \
        "$iso_root/boot/bzImage"

    cp \
        "$INITRAMFS" \
        "$iso_root/boot/initramfs.cpio.gz"

    cat > "$iso_root/boot/grub/grub.cfg" <<'EOF'
set timeout=0
set default=0

menuentry "Senbit" {
    linux /boot/bzImage
    initrd /boot/initramfs.cpio.gz
}
EOF

    echo "==> Creating bootable ISO..."

    grub-mkrescue \
        -o "$ISO_IMAGE" \
        "$iso_root"

    mkdir -p "$ROOT_DIR/iso"

    cp \
        "$ISO_IMAGE" \
        "$ROOT_DIR/iso/senbit.iso"

    echo
    echo "ISO:"
    echo "  $ISO_IMAGE"
    echo
    echo "Distribution ISO:"
    echo "  $ROOT_DIR/iso/senbit.iso"
}

# --------------------------------------------------
# Build selection
# --------------------------------------------------

TARGET="${1:-all}"

case "$TARGET" in

    kernel)
        run_step "Linux Kernel" build_kernel
        ;;

    busybox)
        run_step "BusyBox" build_busybox
        ;;

    rootfs)
        run_step "Senbit Root Filesystem" build_rootfs
        ;;

    iso)
        run_step "Senbit ISO" build_iso
        ;;

    all)
        run_step "Linux Kernel" build_kernel
        run_step "BusyBox" build_busybox
        run_step "Senbit Root Filesystem" build_rootfs
        run_step "Senbit ISO" build_iso
        ;;

    *)
        echo "Usage:"
        echo "  ./scripts/build.sh"
        echo "  ./scripts/build.sh all"
        echo "  ./scripts/build.sh kernel"
        echo "  ./scripts/build.sh busybox"
        echo "  ./scripts/build.sh rootfs"
        echo "  ./scripts/build.sh iso"
        exit 1
        ;;

esac

TOTAL_END="$(date +%s)"
TOTAL_ELAPSED=$((TOTAL_END - TOTAL_START))

echo
echo "========================================"
echo "       Senbit Build Complete"
echo "========================================"
echo
echo "Total time: $(format_time "$TOTAL_ELAPSED")"
echo
