#!/usr/bin/env bash

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

KERNEL_DIR="$ROOT_DIR/kernel/linux"
KERNEL_CONFIG="$ROOT_DIR/config/kernel/version"

KERNEL_REPO="https://git.kernel.org/pub/scm/linux/kernel/git/torvalds/linux.git"

if [[ ! -f "$KERNEL_CONFIG" ]]; then
    echo "Error: missing kernel version configuration:"
    echo "  $KERNEL_CONFIG"
    exit 1
fi

KERNEL_VERSION="$(tr -d '[:space:]' < "$KERNEL_CONFIG")"

if [[ -z "$KERNEL_VERSION" ]]; then
    echo "Error: kernel version configuration is empty."
    exit 1
fi

echo "==> Senbit Linux"
echo "    Requested version: $KERNEL_VERSION"
echo

if [[ ! -d "$KERNEL_DIR/.git" ]]; then
    echo "==> Linux source tree not found."
    echo "==> Cloning Linux..."

    mkdir -p "$ROOT_DIR/kernel"

    git clone --branch "$KERNEL_VERSION" --depth 1 \
        "$KERNEL_REPO" \
        "$KERNEL_DIR"

    echo
    echo "==> Linux $KERNEL_VERSION downloaded."
    exit 0
fi

cd "$KERNEL_DIR"

CURRENT_VERSION="$(git describe --tags --always 2>/dev/null || true)"

if [[ "$CURRENT_VERSION" == "$KERNEL_VERSION" ]]; then
    echo "==> Linux $KERNEL_VERSION is already installed."
    exit 0
fi

echo "==> Current kernel: $CURRENT_VERSION"
echo "==> Updating to:    $KERNEL_VERSION"
echo

git fetch --tags --prune origin

git checkout --detach "$KERNEL_VERSION"

echo
echo "==> Linux kernel updated."
echo "    Version: $(git describe --tags --always)"
echo "    Commit:  $(git rev-parse HEAD)"
