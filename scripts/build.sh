#!/usr/bin/env bash

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

KERNEL_DIR="$ROOT_DIR/kernel/linux"
BUSYBOX_DIR="$ROOT_DIR/third_party/busybox"

BUILD_DIR="$ROOT_DIR/build"

KERNEL_BUILD_DIR="$BUILD_DIR/kernel"
BUSYBOX_BUILD_DIR="$BUILD_DIR/busybox"
ROOTFS_BUILD_DIR="$BUILD_DIR/rootfs"
ISO_BUILD_DIR="$BUILD_DIR/iso"

KERNEL_IMAGE="$KERNEL_BUILD_DIR/arch/x86/boot/bzImage"
BUSYBOX_BINARY="$BUSYBOX_BUILD_DIR/_install/bin/busybox"

RUST_TARGET="x86_64-unknown-linux-musl"
SENBIT_INIT_BINARY="$ROOT_DIR/target/$RUST_TARGET/release/senbit-init"

INITRAMFS="$ROOTFS_BUILD_DIR/initramfs.cpio.gz"
ISO_IMAGE="$ISO_BUILD_DIR/senbit.iso"

KERNEL_VERSION_FILE="$ROOT_DIR/config/kernel/version"
BUSYBOX_VERSION_FILE="$ROOT_DIR/config/busybox/version"
BUSYBOX_CONFIG="$ROOT_DIR/config/busybox.config"

BUSYBOX_PATCH_DIR="$ROOT_DIR/third_party/patches/busybox"

KERNEL_REPO="https://git.kernel.org/pub/scm/linux/kernel/git/torvalds/linux.git"
BUSYBOX_REPO="https://git.busybox.net/busybox"

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
# Linux
# --------------------------------------------------

get_latest_linux_version() {
    git ls-remote \
        --tags \
        --refs \
        "$KERNEL_REPO" \
        'refs/tags/v[0-9]*' |
        awk '{print $2}' |
        sed 's#refs/tags/##' |
        grep -E '^v[0-9]+\.[0-9]+(\.[0-9]+)?$' |
        sort -V |
        tail -n 1
}


update_linux() {
    if [[ ! -e "$KERNEL_DIR/.git" ]]; then
        echo "Error: Linux source tree not found:"
        echo "  $KERNEL_DIR"
        echo
        echo "Run:"
        echo "  ./scripts/getlinux.sh"
        exit 1
    fi

    local latest
    local current

    latest="$(get_latest_linux_version)"

    if [[ -z "$latest" ]]; then
        echo "Error: unable to determine latest Linux version."
        exit 1
    fi

    current="$(git -C "$KERNEL_DIR" describe --tags --always 2>/dev/null || true)"

    echo "==> Linux version"
    echo "    Current: $current"
    echo "    Latest:  $latest"

    if [[ "$current" == "$latest" ]]; then
        echo "==> Linux is already up to date."
        return
    fi

    echo
    echo "==> Updating Linux kernel..."
    echo "    $current -> $latest"

    git -C "$KERNEL_DIR" fetch \
        --tags \
        --prune \
        origin

    git -C "$KERNEL_DIR" checkout --detach "$latest"

    printf '%s\n' "$latest" > "$KERNEL_VERSION_FILE"

    echo "==> Linux updated."
}


build_kernel() {
    mkdir -p "$KERNEL_BUILD_DIR"

    update_linux

    echo
    echo "==> Preparing Linux kernel..."

    if [[ ! -f "$KERNEL_BUILD_DIR/.config" ]]; then
        echo "==> Creating x86_64 kernel configuration..."

        make \
            -C "$KERNEL_DIR" \
            O="$KERNEL_BUILD_DIR" \
            x86_64_defconfig
    fi

    echo
    echo "==> Updating kernel configuration..."

    make \
        -C "$KERNEL_DIR" \
        O="$KERNEL_BUILD_DIR" \
        olddefconfig

    echo
    echo "==> Building Linux kernel..."
    echo "    Jobs: $JOBS"
    echo
    echo "    Incremental build enabled."

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

get_latest_busybox_version() {
    git ls-remote \
        --tags \
        --refs \
        "$BUSYBOX_REPO" |
        awk '{print $2}' |
        sed 's#refs/tags/##' |
        grep -E '^1_[0-9]+_[0-9]+$' |
        sort -V |
        tail -n 1
}


update_busybox() {
    if [[ ! -e "$BUSYBOX_DIR/.git" ]]; then
        echo "Error: BusyBox source tree not found:"
        echo "  $BUSYBOX_DIR"
        echo
        echo "Run:"
        echo "  ./scripts/getbusy.sh"
        exit 1
    fi

    local latest
    local current

    latest="$(get_latest_busybox_version)"

    if [[ -z "$latest" ]]; then
        echo "Error: unable to determine latest BusyBox version."
        exit 1
    fi

    current="$(git -C "$BUSYBOX_DIR" describe --tags --always 2>/dev/null || true)"

    echo "==> BusyBox version"
    echo "    Current: $current"
    echo "    Latest:  $latest"

    if [[ "$current" != "$latest" ]]; then
        echo
        echo "==> Updating BusyBox..."
        echo "    $current -> $latest"

        if [[ -n "$(git -C "$BUSYBOX_DIR" status --porcelain)" ]]; then
            echo
            echo "Error: BusyBox source tree contains local modifications:"
            echo
            git -C "$BUSYBOX_DIR" status --short
            echo
            echo "Senbit patches must be stored in:"
            echo "  third_party/patches/busybox/"
            echo
            echo "The working tree must be clean before updating BusyBox."
            exit 1
        fi

        git -C "$BUSYBOX_DIR" fetch \
            --tags \
            --prune \
            origin

        git -C "$BUSYBOX_DIR" checkout --detach "$latest"

        local version
        version="${latest//_/.}"

        printf '%s\n' "$version" > "$BUSYBOX_VERSION_FILE"

        echo "==> BusyBox updated."
    else
        echo "==> BusyBox is already up to date."
    fi
}


apply_busybox_patches() {
    if [[ ! -d "$BUSYBOX_PATCH_DIR" ]]; then
        echo "==> No BusyBox patches directory."
        return
    fi

    local patch
    local patch_name

    shopt -s nullglob

    local patches=(
        "$BUSYBOX_PATCH_DIR"/*.patch
    )

    shopt -u nullglob

    if [[ "${#patches[@]}" -eq 0 ]]; then
        echo "==> No BusyBox patches to apply."
        return
    fi

    echo
    echo "==> Applying Senbit BusyBox patches..."

    for patch in "${patches[@]}"; do
        patch_name="$(basename "$patch")"

        echo
        echo "    Applying: $patch_name"

        if git -C "$BUSYBOX_DIR" apply --check "$patch"; then
            git -C "$BUSYBOX_DIR" apply "$patch"
        else
            echo
            echo "Error: BusyBox patch cannot be applied:"
            echo "  $patch"
            echo
            echo "BusyBox source version:"
            git -C "$BUSYBOX_DIR" describe --tags --always
            echo
            echo "The patch may need to be updated for this BusyBox version."
            exit 1
        fi
    done

    echo
    echo "==> BusyBox patches applied."
}


build_busybox() {
    mkdir -p "$BUSYBOX_BUILD_DIR"

    update_busybox

    apply_busybox_patches

    if [[ ! -f "$BUSYBOX_CONFIG" ]]; then
        echo "Error: BusyBox configuration not found:"
        echo "  $BUSYBOX_CONFIG"
        exit 1
    fi

    echo
    echo "==> Preparing BusyBox..."

    if [[ ! -f "$BUSYBOX_BUILD_DIR/.config" ]]; then
        echo "==> Installing Senbit BusyBox configuration..."

        cp \
            "$BUSYBOX_CONFIG" \
            "$BUSYBOX_BUILD_DIR/.config"
    fi

    echo
    echo "==> Building BusyBox..."
    echo "    Jobs: $JOBS"
    echo
    echo "    Incremental build enabled."

    make \
        -C "$BUSYBOX_DIR" \
        O="$BUSYBOX_BUILD_DIR" \
        -j"$JOBS"

    echo
    echo "==> Installing BusyBox..."

    make \
        -C "$BUSYBOX_DIR" \
        O="$BUSYBOX_BUILD_DIR" \
        CONFIG_PREFIX="$BUSYBOX_BUILD_DIR/_install" \
        install

    if [[ ! -f "$BUSYBOX_BINARY" ]]; then
        echo "Error: BusyBox binary was not produced:"
        echo "  $BUSYBOX_BINARY"
        exit 1
    fi

    echo
    echo "BusyBox:"
    echo "  $BUSYBOX_BINARY"
}


# --------------------------------------------------
# Rust
# --------------------------------------------------

build_rust() {
    echo "==> Building Senbit Rust userspace..."

    if [[ ! -f "$ROOT_DIR/Cargo.toml" ]]; then
        echo "Error: Cargo.toml not found:"
        echo "  $ROOT_DIR/Cargo.toml"
        exit 1
    fi

    (
        cd "$ROOT_DIR"

        cargo build \
            --release \
            --target "$RUST_TARGET"
    )

    if [[ ! -x "$SENBIT_INIT_BINARY" ]]; then
        echo "Error: Senbit init binary was not produced:"
        echo "  $SENBIT_INIT_BINARY"
        exit 1
    fi

    echo
    echo "Senbit init:"
    echo "  $SENBIT_INIT_BINARY"
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

    if [[ -d "$ROOT_DIR/rootfs" ]]; then
        rsync -a \
            "$ROOT_DIR/rootfs/" \
            "$rootfs/"
    fi

    echo "==> Installing BusyBox into root filesystem..."

    if [[ ! -d "$BUSYBOX_BUILD_DIR/_install" ]]; then
        echo "Error: BusyBox installation directory not found:"
        echo "  $BUSYBOX_BUILD_DIR/_install"
        exit 1
    fi

    rsync -a \
        "$BUSYBOX_BUILD_DIR/_install/" \
        "$rootfs/"

    echo "==> Installing Senbit Rust init..."
    
    if [[ ! -x "$SENBIT_INIT_BINARY" ]]; then
        echo "Error: Senbit init binary not found:"
        echo "  $SENBIT_INIT_BINARY"
        echo
        echo "Run:"
        echo "  ./scripts/build.sh rust"
        exit 1
    fi
    
    rm -f "$rootfs/init"
    
    cp \
        "$SENBIT_INIT_BINARY" \
        "$rootfs/init"
    
    chmod +x "$rootfs/init"
    
    ln -sf ../init "$rootfs/sbin/init"

    echo
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
    linux /boot/bzImage console=tty0
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
# Main
# --------------------------------------------------

TARGET="${1:-all}"

case "$TARGET" in
    kernel)
        run_step "Linux Kernel" build_kernel
        ;;

    busybox)
        run_step "BusyBox" build_busybox
        ;;

    rust)
        run_step "Senbit Rust Userspace" build_rust
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
        run_step "Senbit Rust Userspace" build_rust
        run_step "Senbit Root Filesystem" build_rootfs
        run_step "Senbit ISO" build_iso
        ;;

    *)
        echo "Usage:"
        echo "  ./scripts/build.sh"
        echo "  ./scripts/build.sh all"
        echo "  ./scripts/build.sh kernel"
        echo "  ./scripts/build.sh busybox"
        echo "  ./scripts/build.sh rust"
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
