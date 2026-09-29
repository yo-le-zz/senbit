#!/usr/bin/env bash

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

UTIL_LINUX_DIR="$ROOT_DIR/third_party/util-linux"
UTIL_LINUX_VERSION_FILE="$ROOT_DIR/config/util-linux/version"

UTIL_LINUX_REPO="https://github.com/util-linux/util-linux.git"


# --------------------------------------------------
# Helpers
# --------------------------------------------------

get_util_linux_version() {
    if [[ ! -f "$UTIL_LINUX_VERSION_FILE" ]]; then
        echo "Error: util-linux version file not found:"
        echo "  $UTIL_LINUX_VERSION_FILE"
        exit 1
    fi

    tr -d '[:space:]' < "$UTIL_LINUX_VERSION_FILE"
}


# --------------------------------------------------
# util-linux
# --------------------------------------------------

get_util_linux() {
    local version

    version="$(get_util_linux_version)"

    if [[ -z "$version" ]]; then
        echo "Error: util-linux version is empty."
        exit 1
    fi

    echo "==> util-linux"
    echo "    Version: $version"
    echo "    Directory: $UTIL_LINUX_DIR"

    if [[ -e "$UTIL_LINUX_DIR/.git" ]]; then
        local current

        current="$(git -C "$UTIL_LINUX_DIR" describe --tags --always 2>/dev/null || true)"

        echo "    Current: $current"

        if [[ "$current" == "v$version" ]]; then
            echo "==> util-linux is already up to date."
            return
        fi

        echo
        echo "==> Updating util-linux..."
        echo "    $current -> v$version"

        if [[ -n "$(git -C "$UTIL_LINUX_DIR" status --porcelain)" ]]; then
            echo
            echo "Error: util-linux source tree contains local modifications:"
            echo
            git -C "$UTIL_LINUX_DIR" status --short
            exit 1
        fi

        git -C "$UTIL_LINUX_DIR" fetch \
            --tags \
            --prune \
            origin

        git -C "$UTIL_LINUX_DIR" checkout --detach "v$version"

        echo "==> util-linux updated."
        return
    fi

    if [[ -e "$UTIL_LINUX_DIR" ]]; then
        echo "Error: util-linux directory already exists but is not a Git repository:"
        echo "  $UTIL_LINUX_DIR"
        exit 1
    fi

    echo
    echo "==> Downloading util-linux..."
    echo "    Repository: $UTIL_LINUX_REPO"
    echo "    Version:    v$version"

    git clone \
        --branch "v$version" \
        --depth 1 \
        "$UTIL_LINUX_REPO" \
        "$UTIL_LINUX_DIR"

    if [[ "$(git -C "$UTIL_LINUX_DIR" describe --tags --exact-match 2>/dev/null || true)" != "v$version" ]]; then
        echo "Error: downloaded util-linux version does not match requested version."
        exit 1
    fi

    echo
    echo "==> util-linux downloaded."
}


# --------------------------------------------------
# Main
# --------------------------------------------------

get_util_linux

echo
echo "util-linux:"
echo "  $UTIL_LINUX_DIR"