#!/usr/bin/env bash

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

KERNEL_DIR="$ROOT_DIR/kernel/linux"
BUSYBOX_DIR="$ROOT_DIR/third_party/busybox"
UTIL_LINUX_DIR="$ROOT_DIR/third_party/util-linux"
PARTED_DIR="$ROOT_DIR/third_party/parted"

BUILD_DIR="$ROOT_DIR/build"

KERNEL_BUILD_DIR="$BUILD_DIR/kernel"
BUSYBOX_BUILD_DIR="$BUILD_DIR/busybox"
UTIL_LINUX_BUILD_DIR="$BUILD_DIR/util-linux"
PARTED_BUILD_DIR="$BUILD_DIR/parted"
ROOTFS_BUILD_DIR="$BUILD_DIR/rootfs"
ISO_BUILD_DIR="$BUILD_DIR/iso"

KERNEL_IMAGE="$KERNEL_BUILD_DIR/arch/x86/boot/bzImage"
BUSYBOX_BINARY="$BUSYBOX_BUILD_DIR/_install/bin/busybox"

UTIL_LINUX_FDISK_BINARY="$UTIL_LINUX_BUILD_DIR/fdisk.static"
UTIL_LINUX_SFDISK_BINARY="$UTIL_LINUX_BUILD_DIR/sfdisk.static"

PARTED_BINARY="$PARTED_BUILD_DIR/_install/usr/sbin/parted"

RUST_TARGET="x86_64-unknown-linux-musl"
SENBIT_INIT_BINARY="$ROOT_DIR/target/$RUST_TARGET/release/senbit"

TOOLS_DIR="$ROOT_DIR/tools"

KEYMAPS_GENERATED_DIR="$BUILD_DIR/tools/generated/keymaps"
KEYMAPS_ROOTFS_DIR="$ROOTFS_BUILD_DIR/usr/share/keymaps"

INITRAMFS="$BUILD_DIR/initramfs.cpio.gz"
ISO_IMAGE="$ISO_BUILD_DIR/senbit.iso"

KERNEL_VERSION_FILE="$ROOT_DIR/config/kernel/version"
BUSYBOX_VERSION_FILE="$ROOT_DIR/config/busybox/version"
BUSYBOX_CONFIG="$ROOT_DIR/config/busybox.config"
UTIL_LINUX_VERSION_FILE="$ROOT_DIR/config/util-linux/version"
PARTED_VERSION_FILE="$ROOT_DIR/config/parted/version"

BUSYBOX_PATCH_DIR="$ROOT_DIR/third_party/patches/busybox"
PARTED_PATCH_DIR="$ROOT_DIR/third_party/patches/parted"

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

    local current

    current="$(git -C "$KERNEL_DIR" describe --tags --always 2>/dev/null || true)"

    echo "==> Linux version"
    echo "    Current: $current"
    echo "    Using checked-out version."
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
        elif git -C "$BUSYBOX_DIR" apply --reverse --check "$patch"; then
            echo "    Already applied: $patch_name"
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
# util-linux
# --------------------------------------------------

get_util_linux_version() {
    if [[ ! -f "$UTIL_LINUX_VERSION_FILE" ]]; then
        echo "Error: util-linux version file not found:"
        echo "  $UTIL_LINUX_VERSION_FILE"
        exit 1
    fi

    tr -d '[:space:]' < "$UTIL_LINUX_VERSION_FILE"
}


check_util_linux() {
    if [[ ! -e "$UTIL_LINUX_DIR/.git" ]]; then
        echo "Error: util-linux source tree not found:"
        echo "  $UTIL_LINUX_DIR"
        echo
        echo "Initialize Git submodules with:"
        echo "  git submodule update --init --recursive"
        exit 1
    fi

    local version
    version="$(get_util_linux_version)"

    if [[ -z "$version" ]]; then
        echo "Error: util-linux version is empty."
        exit 1
    fi

    local current
    current="$(git -C "$UTIL_LINUX_DIR" describe --tags --exact-match 2>/dev/null || true)"

    echo "==> util-linux"
    echo "    Requested: v$version"
    echo "    Current:   $current"

    if [[ "$current" != "v$version" ]]; then
        echo
        echo "Error: util-linux submodule is not on v$version."
        echo
        echo "Run:"
        echo "  git -C third_party/util-linux checkout v$version"
        exit 1
    fi
}


build_util_linux() {
    mkdir -p "$UTIL_LINUX_BUILD_DIR"

    check_util_linux

    echo
    echo "==> Preparing util-linux..."

    if [[ ! -f "$UTIL_LINUX_BUILD_DIR/Makefile" ]]; then
        echo "==> Configuring util-linux..."

        (
            cd "$UTIL_LINUX_BUILD_DIR"

            "$UTIL_LINUX_DIR/configure" \
                --prefix=/usr \
                --bindir=/usr/bin \
                --sbindir=/usr/sbin \
                --libdir=/usr/lib \
                --disable-all-programs \
                --enable-fdisks \
                --enable-libfdisk \
                --enable-libsmartcols \
                --enable-libuuid \
                --enable-static-programs=fdisk,sfdisk
        )
    fi

    echo
    echo "==> Building static util-linux tools..."
    echo "    Jobs: $JOBS"

    make \
        -C "$UTIL_LINUX_BUILD_DIR" \
        fdisk.static \
        sfdisk.static \
        -j"$JOBS"

    if [[ ! -x "$UTIL_LINUX_FDISK_BINARY" ]]; then
        echo "Error: util-linux fdisk.static was not produced:"
        echo "  $UTIL_LINUX_FDISK_BINARY"
        exit 1
    fi

    if [[ ! -x "$UTIL_LINUX_SFDISK_BINARY" ]]; then
        echo "Error: util-linux sfdisk.static was not produced:"
        echo "  $UTIL_LINUX_SFDISK_BINARY"
        exit 1
    fi

    echo
    echo "==> Verifying static binaries..."

    if file "$UTIL_LINUX_FDISK_BINARY" | grep -q "dynamically linked"; then
        echo "Error: fdisk.static is dynamically linked."
        exit 1
    fi

    if file "$UTIL_LINUX_SFDISK_BINARY" | grep -q "dynamically linked"; then
        echo "Error: sfdisk.static is dynamically linked."
        exit 1
    fi

    echo
    echo "util-linux:"
    echo "  fdisk:  $UTIL_LINUX_FDISK_BINARY"
    echo "  sfdisk: $UTIL_LINUX_SFDISK_BINARY"
}


# --------------------------------------------------
# GNU Parted
# --------------------------------------------------

get_parted_version() {
    if [[ ! -f "$PARTED_VERSION_FILE" ]]; then
        echo "Error: Parted version file not found:"
        echo "  $PARTED_VERSION_FILE"
        exit 1
    fi

    tr -d '[:space:]' < "$PARTED_VERSION_FILE"
}


check_parted() {
    if [[ ! -e "$PARTED_DIR/.git" ]]; then
        echo "Error: GNU Parted source tree not found:"
        echo "  $PARTED_DIR"
        echo
        echo "Initialize Git submodules with:"
        echo "  git submodule update --init --recursive"
        exit 1
    fi

    local version
    version="$(get_parted_version)"

    if [[ -z "$version" ]]; then
        echo "Error: GNU Parted version is empty."
        exit 1
    fi

    local current
    current="$(git -C "$PARTED_DIR" describe --tags --exact-match 2>/dev/null || true)"

    echo "==> GNU Parted"
    echo "    Requested: v$version"
    echo "    Current:   $current"

    if [[ "$current" != "v$version" ]]; then
        echo
        echo "Error: GNU Parted submodule is not on v$version."
        echo
        echo "Run:"
        echo "  git -C third_party/parted checkout v$version"
        exit 1
    fi
}


apply_parted_patches() {
    if [[ ! -d "$PARTED_PATCH_DIR" ]]; then
        echo "==> No GNU Parted patches directory."
        return
    fi

    local patch
    local patch_name

    shopt -s nullglob

    local patches=(
        "$PARTED_PATCH_DIR"/*.patch
    )

    shopt -u nullglob

    if [[ "${#patches[@]}" -eq 0 ]]; then
        echo "==> No GNU Parted patches to apply."
        return
    fi

    echo
    echo "==> Applying Senbit GNU Parted patches..."

    for patch in "${patches[@]}"; do
        patch_name="$(basename "$patch")"

        echo
        echo "    Applying: $patch_name"

        if git -C "$PARTED_DIR" apply --check "$patch"; then
            git -C "$PARTED_DIR" apply "$patch"
        elif git -C "$PARTED_DIR" apply --reverse --check "$patch"; then
            echo "    Already applied: $patch_name"
        else
            echo
            echo "Error: GNU Parted patch cannot be applied:"
            echo "  $patch"
            echo
            echo "GNU Parted source version:"
            git -C "$PARTED_DIR" describe --tags --always
            echo
            echo "The patch may need to be updated for this Parted version."
            exit 1
        fi
    done

    echo
    echo "==> GNU Parted patches applied."
}


build_parted() {
    mkdir -p "$PARTED_BUILD_DIR"

    check_parted
    apply_parted_patches

    echo
    echo "==> Preparing GNU Parted..."

    if [[ ! -d "$PARTED_DIR/gnulib" ]]; then
        echo
        echo "Error: GNU Parted gnulib directory not found:"
        echo "  $PARTED_DIR/gnulib"
        echo
        echo "GNU Parted requires gnulib to bootstrap."
        echo "Run the Parted source setup script before building."
        exit 1
    fi

    if [[ ! -f "$PARTED_DIR/configure" ]]; then
        echo "==> Generating GNU Parted build system..."

        (
            cd "$PARTED_DIR"
            ./bootstrap
        )
    fi

    if [[ ! -f "$PARTED_BUILD_DIR/Makefile" ]]; then
        echo "==> Configuring GNU Parted..."

        (
            cd "$PARTED_BUILD_DIR"

            "$PARTED_DIR/configure" \
                --prefix=/usr \
                --bindir=/usr/bin \
                --sbindir=/usr/sbin \
                --libdir=/usr/lib \
                --disable-device-mapper \
                --disable-debug
        )
    fi

    echo
    echo "==> Building GNU Parted..."
    echo "    Jobs: $JOBS"

    make \
        -C "$PARTED_BUILD_DIR" \
        -j"$JOBS"

    echo
    echo "==> Installing GNU Parted..."

    rm -rf "$PARTED_BUILD_DIR/_install"

    make \
        -C "$PARTED_BUILD_DIR" \
        DESTDIR="$PARTED_BUILD_DIR/_install" \
        install

    if [[ ! -x "$PARTED_BINARY" ]]; then
        echo "Error: GNU Parted binary was not produced:"
        echo "  $PARTED_BINARY"
        exit 1
    fi

    echo
    echo "GNU Parted:"
    echo "  $PARTED_BINARY"
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
# Build tools
# --------------------------------------------------

build_tools() {
    echo "==> Searching for Senbit build tools..."

    if [[ ! -d "$TOOLS_DIR" ]]; then
        echo "==> No tools directory found."
        return 0
    fi

    shopt -s nullglob

    local tools=(
        "$TOOLS_DIR"/*
    )

    shopt -u nullglob

    for tool_dir in "${tools[@]}"; do
        [[ -d "$tool_dir" ]] || continue
        [[ -f "$tool_dir/Cargo.toml" ]] || continue

        local tool_name
        tool_name="$(basename "$tool_dir")"

        echo
        echo "----------------------------------------"
        echo "  Tool: $tool_name"
        echo "----------------------------------------"

        echo "==> Compiling..."

        (
            cd "$ROOT_DIR" || exit 1

            cargo build \
                --release \
                --manifest-path "$tool_dir/Cargo.toml"

            local binary
            binary="$(
                cargo metadata \
                    --format-version 1 \
                    --no-deps \
                    --manifest-path "$tool_dir/Cargo.toml" |
                python3 -c '
import json
import sys

data = json.load(sys.stdin)

for package in data["packages"]:
    for target in package["targets"]:
        if "bin" in target["kind"]:
            print(target["name"])
            raise SystemExit
'
            )"

            if [[ -z "$binary" ]]; then
                echo "Error: unable to determine compiled binary name."
                exit 1
            fi

            local binary_path="$tool_dir/target/release/$binary"

            if [[ ! -x "$binary_path" ]]; then
                echo "Error: compiled tool binary not found:"
                echo "  $binary_path"
                exit 1
            fi

            echo "==> Running..."
            echo "    $binary_path"

            "$binary_path"
        )
    done

    echo
    echo "==> All Senbit build tools completed."
}

# --------------------------------------------------
# Root filesystem
# --------------------------------------------------

build_rootfs() {
    mkdir -p "$ROOTFS_BUILD_DIR"

    local rootfs="$ROOTFS_BUILD_DIR"

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
        "$rootfs/usr/sbin" \
        "$rootfs/usr/share/keymaps"

    if [[ -d "$ROOT_DIR/rootfs" ]]; then
        rsync -a \
            "$ROOT_DIR/rootfs/" \
            "$rootfs/"
    fi

    echo "==> Installing generated keymaps..."
    
    if [[ ! -d "$KEYMAPS_GENERATED_DIR" ]]; then
        echo "Error: generated keymaps directory not found:"
        echo "  $KEYMAPS_GENERATED_DIR"
        exit 1
    fi
    
    shopt -s nullglob
    local keymaps=(
        "$KEYMAPS_GENERATED_DIR"/*.bmap
    )
    shopt -u nullglob
    
    if [[ "${#keymaps[@]}" -eq 0 ]]; then
        echo "Error: no generated keymaps found:"
        echo "  $KEYMAPS_GENERATED_DIR"
        exit 1
    fi
    
    cp \
        "${keymaps[@]}" \
        "$KEYMAPS_ROOTFS_DIR/"
    
    echo "==> Installed ${#keymaps[@]} keymaps."

    echo "==> Installing BusyBox into root filesystem..."

    if [[ ! -d "$BUSYBOX_BUILD_DIR/_install" ]]; then
        echo "Error: BusyBox installation directory not found:"
        echo "  $BUSYBOX_BUILD_DIR/_install"
        exit 1
    fi

    rsync -a \
        "$BUSYBOX_BUILD_DIR/_install/" \
        "$rootfs/"

    echo "==> Installing util-linux..."

    if [[ ! -x "$UTIL_LINUX_FDISK_BINARY" ]]; then
        echo "Error: util-linux fdisk.static not found:"
        echo "  $UTIL_LINUX_FDISK_BINARY"
        exit 1
    fi

    if [[ ! -x "$UTIL_LINUX_SFDISK_BINARY" ]]; then
        echo "Error: util-linux sfdisk.static not found:"
        echo "  $UTIL_LINUX_SFDISK_BINARY"
        exit 1
    fi

    cp \
        "$UTIL_LINUX_FDISK_BINARY" \
        "$rootfs/usr/sbin/fdisk"

    cp \
        "$UTIL_LINUX_SFDISK_BINARY" \
        "$rootfs/usr/sbin/sfdisk"

    chmod +x \
        "$rootfs/usr/sbin/fdisk" \
        "$rootfs/usr/sbin/sfdisk"

    echo "==> Installing GNU Parted..."

    if [[ ! -x "$PARTED_BINARY" ]]; then
        echo "Error: GNU Parted binary not found:"
        echo "  $PARTED_BINARY"
        exit 1
    fi

    cp \
        "$PARTED_BINARY" \
        "$rootfs/usr/sbin/parted"

    chmod +x \
        "$rootfs/usr/sbin/parted"

    echo "==> Installing GNU Parted runtime dependencies..."

    local parted_lib
    while read -r parted_lib; do
        [[ -z "$parted_lib" ]] && continue

        local lib_path="${parted_lib#*=> }"
        lib_path="${lib_path%% *}"

        if [[ "$lib_path" != /* ]]; then
            continue
        fi

        if [[ ! -f "$lib_path" ]]; then
            continue
        fi

        local lib_dest="$rootfs$lib_path"

        mkdir -p "$(dirname "$lib_dest")"
        cp -L "$lib_path" "$lib_dest"

        echo "  $lib_path"
    done < <(ldd "$PARTED_BINARY" 2>/dev/null | grep '=>' || true)

    local parted_loader
    parted_loader="$(ldd "$PARTED_BINARY" 2>/dev/null | awk '/ld-linux/ {print $1; exit}')"

    if [[ -n "$parted_loader" && -f "$parted_loader" ]]; then
        mkdir -p "$rootfs$(dirname "$parted_loader")"
        cp -L "$parted_loader" "$rootfs$parted_loader"
        echo "  $parted_loader"
    fi

    echo "==> Installing Senbit Rust init..."

    if [[ ! -x "$SENBIT_INIT_BINARY" ]]; then
        echo "Error: Senbit init binary not found:"
        echo "  $SENBIT_INIT_BINARY"
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

    mkdir -p "$(dirname "$INITRAMFS")"

    (
        cd "$rootfs"
        find . -print0 | cpio --null -o -H newc | gzip -9 > "$INITRAMFS"
    )
    
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
    linux /boot/bzImage console=ttyS0,115200 console=tty0 loglevel=3
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

    util-linux)
        run_step "util-linux" build_util_linux
        ;;

    parted)
        run_step "GNU Parted" build_parted
        ;;

    rust)
        run_step "Senbit Rust Userspace" build_rust
        ;;

    tools)
        run_step "Senbit Build Tools" build_tools
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
        run_step "util-linux" build_util_linux
        run_step "GNU Parted" build_parted
        run_step "Senbit Rust Userspace" build_rust
        run_step "Senbit Build Tools" build_tools
        run_step "Senbit Root Filesystem" build_rootfs
        run_step "Senbit ISO" build_iso
        ;;

    *)
        echo "Usage:"
        echo "  ./scripts/build.sh"
        echo "  ./scripts/build.sh all"
        echo "  ./scripts/build.sh kernel"
        echo "  ./scripts/build.sh busybox"
        echo "  ./scripts/build.sh util-linux"
        echo "  ./scripts/build.sh parted"
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