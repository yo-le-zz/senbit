#!/usr/bin/env bash

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

BUILD_SCRIPT="$ROOT_DIR/scripts/build.sh"
VM_CONFIG="$ROOT_DIR/config/vm.config"

if [[ ! -x "$BUILD_SCRIPT" ]]; then
    echo "Error: build script not found or not executable:"
    echo "  $BUILD_SCRIPT"
    exit 1
fi

if [[ ! -f "$VM_CONFIG" ]]; then
    echo "Error: VM configuration not found:"
    echo "  $VM_CONFIG"
    exit 1
fi

# --------------------------------------------------
# VM defaults
# --------------------------------------------------

RAM="512M"
CPUS="2"

DISK="$ROOT_DIR/build/vm/senbit.qcow2"
DISK_SIZE="10G"
DISK_FORMAT="qcow2"

NETWORK="user"

# --------------------------------------------------
# Load VM configuration
# --------------------------------------------------

while IFS='=' read -r key value; do
    # Ignore empty lines
    [[ -z "${key// }" ]] && continue

    # Ignore comments
    [[ "$key" =~ ^[[:space:]]*# ]] && continue

    key="$(echo "$key" | xargs)"
    value="$(echo "$value" | xargs)"

    case "$key" in
        RAM)
            RAM="$value"
            ;;

        CPUS)
            CPUS="$value"
            ;;

        DISK)
            DISK="$value"
            ;;

        DISK_SIZE)
            DISK_SIZE="$value"
            ;;

        DISK_FORMAT)
            DISK_FORMAT="$value"
            ;;

        NETWORK)
            NETWORK="$value"
            ;;

        *)
            echo "Warning: unknown VM option: $key"
            ;;
    esac

done < "$VM_CONFIG"

# --------------------------------------------------
# Resolve paths
# --------------------------------------------------

if [[ "$DISK" != /* ]]; then
    DISK="$ROOT_DIR/$DISK"
fi

ISO="$ROOT_DIR/iso/senbit.iso"

# --------------------------------------------------
# Build Senbit
# --------------------------------------------------

echo
echo "========================================"
echo "          Senbit Run"
echo "========================================"
echo

"$BUILD_SCRIPT"

# --------------------------------------------------
# Verify ISO
# --------------------------------------------------

if [[ ! -f "$ISO" ]]; then
    echo "Error: Senbit ISO not found:"
    echo "  $ISO"
    exit 1
fi

# --------------------------------------------------
# Prepare VM disk
# --------------------------------------------------

mkdir -p "$(dirname "$DISK")"

if [[ ! -f "$DISK" ]]; then
    echo
    echo "==> Creating VM disk..."
    echo "    Path:   $DISK"
    echo "    Size:   $DISK_SIZE"
    echo "    Format: $DISK_FORMAT"

    qemu-img create \
        -f "$DISK_FORMAT" \
        "$DISK" \
        "$DISK_SIZE"
fi

# --------------------------------------------------
# Network
# --------------------------------------------------

NETWORK_ARGS=()

case "$NETWORK" in
    user)
        NETWORK_ARGS+=(
            -nic user
        )
        ;;

    none)
        NETWORK_ARGS+=(
            -nic none
        )
        ;;

    *)
        echo "Error: unsupported NETWORK value: $NETWORK"
        echo
        echo "Supported values:"
        echo "  user"
        echo "  none"
        exit 1
        ;;
esac

# --------------------------------------------------
# Launch QEMU
# --------------------------------------------------

echo
echo "========================================"
echo "          Starting Senbit VM"
echo "========================================"
echo
echo "RAM:     $RAM"
echo "CPUs:    $CPUS"
echo "Disk:    $DISK"
echo "ISO:     $ISO"
echo "Network: $NETWORK"
echo

exec qemu-system-x86_64 \
    -enable-kvm \
    -m "$RAM" \
    -smp "$CPUS" \
    -drive "file=$DISK,format=$DISK_FORMAT" \
    -cdrom "$ISO" \
    "${NETWORK_ARGS[@]}" \
    -serial mon:stdio