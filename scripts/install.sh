#!/usr/bin/env bash

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

START_TIME="$(date +%s)"

echo "========================================"
echo "       Senbit Build Environment"
echo "========================================"
echo

if [[ "${EUID}" -eq 0 ]]; then
    echo "Error: do not run install.sh as root."
    echo "Run it as your normal user."
    exit 1
fi

echo "==> Updating package lists..."
sudo apt-get update

echo
echo "==> Installing/updating build dependencies..."

sudo apt-get install -y \
    build-essential \
    bc \
    bison \
    flex \
    libelf-dev \
    libssl-dev \
    libncurses-dev \
    dwarves \
    cpio \
    gzip \
    xz-utils \
    bzip2 \
    git \
    wget \
    curl \
    rsync \
    file \
    xorriso \
    grub-pc-bin \
    grub-common \
    mtools \
    qemu-system-x86 \
    qemu-utils \
    rustc \
    cargo

echo
echo "==> Checking Linux kernel..."

if [[ ! -x "$ROOT_DIR/scripts/getlinux.sh" ]]; then
    echo "Error: scripts/getlinux.sh is missing or not executable."
    exit 1
fi

"$ROOT_DIR/scripts/getlinux.sh"

END_TIME="$(date +%s)"
ELAPSED=$((END_TIME - START_TIME))

MINUTES=$((ELAPSED / 60))
SECONDS=$((ELAPSED % 60))

echo
echo "========================================"
echo "   Senbit environment ready"
echo "========================================"
echo
printf 'Time: %02d:%02d\n' "$MINUTES" "$SECONDS"
echo
echo "No compilation was performed."
echo
echo "Build Senbit with:"
echo "  ./scripts/build.sh"
echo