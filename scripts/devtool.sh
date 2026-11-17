#!/usr/bin/env bash
# ======================================================================
# scripts/devtool.sh - compile `devtool` and install it as scripts/devtool
#
#   ./scripts/devtool.sh                 build (if needed), then print help
#   ./scripts/devtool.sh --force         rebuild even if up to date
#   ./scripts/devtool.sh build           build devtool, then run: devtool build
#   ./scripts/devtool.sh doctor          ...any devtool command works
#
# Afterwards, use the executable directly:
#   ./scripts/devtool doctor | install | get | build | run
#
# Works from any directory, on Fedora (KDE), Ubuntu/Debian and Arch: every
# path is derived from this script's own location.
# ======================================================================
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
MANIFEST="$ROOT_DIR/devtool/Cargo.toml"
TARGET_DIR="$ROOT_DIR/build/devtool-target"
OUTPUT="$SCRIPT_DIR/devtool"

say()  { printf '\033[1;32m==>\033[0m %s\n' "$*"; }
warn() { printf '\033[1;33mWarning:\033[0m %s\n' "$*" >&2; }
die()  { printf '\033[1;31mError:\033[0m %s\n' "$*" >&2; exit 1; }

FORCE=0
if [[ "${1:-}" == "--force" || "${1:-}" == "-f" ]]; then
    FORCE=1
    shift
fi

[[ -f "$MANIFEST" ]] || die "devtool sources not found: $MANIFEST"

# --- cargo ----------------------------------------------------------------
# rustup installs to ~/.cargo/bin, which is not always on PATH in a fresh shell.
if ! command -v cargo >/dev/null 2>&1 && [[ -x "$HOME/.cargo/bin/cargo" ]]; then
    export PATH="$HOME/.cargo/bin:$PATH"
fi

if ! command -v cargo >/dev/null 2>&1; then
    . /etc/os-release 2>/dev/null || true
    case " ${ID:-} ${ID_LIKE:-} " in
        *" arch "*)                      hint="sudo pacman -S --needed rustup && rustup default stable" ;;
        *" fedora "*|*" rhel "*)         hint="sudo dnf install rustup && rustup-init -y" ;;
        *" debian "*|*" ubuntu "*)       hint="sudo apt-get install rustup && rustup default stable   (or https://rustup.rs)" ;;
        *)                               hint="https://rustup.rs" ;;
    esac
    die "cargo (Rust) not found. Install it first:
  $hint"
fi

# --- build only when needed ----------------------------------------------
needs_build() {
    [[ "$FORCE" == 1 || ! -x "$OUTPUT" ]] && return 0
    # any source newer than the installed executable
    [[ -n "$(find "$ROOT_DIR/devtool/src" "$MANIFEST" "$ROOT_DIR/devtool/Cargo.lock" \
        -newer "$OUTPUT" -type f -print -quit 2>/dev/null)" ]]
}

if needs_build; then
    say "Building devtool..."
    # Plain `cargo build` of the tool itself: it does not need the musl target.
    # RUSTUP_TOOLCHAIN is left alone; the project's rust-toolchain.toml applies.
    cargo build --release --manifest-path "$MANIFEST" --target-dir "$TARGET_DIR"

    BUILT="$TARGET_DIR/release/devtool"
    [[ -x "$BUILT" ]] || die "build finished but $BUILT is missing"

    # Atomic replace (the old binary may be running).
    install -m 0755 "$BUILT" "$OUTPUT.new"
    mv -f "$OUTPUT.new" "$OUTPUT"
    say "Installed: scripts/devtool"
else
    say "scripts/devtool is up to date."
fi

# --- hand over -------------------------------------------------------------
if [[ $# -gt 0 ]]; then
    exec "$OUTPUT" "$@"
fi

echo
echo "Next steps:"
echo "  ./scripts/devtool doctor     check this machine"
echo "  ./scripts/devtool install    install missing build dependencies"
echo "  ./scripts/devtool get        download the pinned sources"
echo "  ./scripts/devtool build      build Senbit"
echo "  ./scripts/devtool run        boot it in QEMU"
