#!/usr/bin/env bash
# ======================================================================
# newcmds.sh - add a native command to Senbit
#
#   ./newcmds.sh mycmd                    commands/mycmd/ (empty Rust crate)
#   ./newcmds.sh mycmd --sbin             installed in /usr/sbin instead of /usr/bin
#   ./newcmds.sh mycmd --alias a,b        also installed as `a` and `b` (links)
#   ./newcmds.sh mycmd --setuid           setuid root (like sudo)
#   ./newcmds.sh mycmd -d "description"   description shown in commands/INDEX.md
#   ./newcmds.sh --list                   commands detected
#   ./newcmds.sh --remove mycmd           delete commands/mycmd/
#   ./newcmds.sh --reindex                regenerate commands/INDEX.md
#
# Nothing else to do: `scripts/devtool build` (or `devtool build commands`)
# detects every folder of commands/ that has a Cargo.toml, compiles it and
# installs it in the rootfs. The index (commands/INDEX.md) is regenerated at
# each build. The crate must produce a binary named like the command.
#
# Works from any directory: every path comes from this script's location.
# ======================================================================
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
COMMANDS_DIR="$ROOT_DIR/commands"
DEVTOOL="$ROOT_DIR/scripts/devtool"

say()  { printf '\033[1;32m==>\033[0m %s\n' "$*"; }
warn() { printf '\033[1;33mWarning:\033[0m %s\n' "$*" >&2; }
die()  { printf '\033[1;31mError:\033[0m %s\n' "$*" >&2; exit 1; }

usage() { sed -n '2,19p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'; }

# Command and alias names: lowercase letters, digits and - _ . + (no leading - or .)
valid_name() { [[ "$1" =~ ^[a-z0-9_][a-z0-9._+-]*$ ]]; }

# Is the name already used by a command, or an alias of one?
name_taken() {
    local wanted="$1" manifest
    for manifest in "$COMMANDS_DIR"/*/Cargo.toml; do
        [[ -f "$manifest" ]] || continue
        local dir
        dir="$(basename "$(dirname "$manifest")")"
        [[ "$dir" == _* ]] && continue
        [[ "$dir" == "$wanted" ]] && return 0
        # package name, [[bin]] names and aliases = ["x", "y"]
        if grep -E '^[[:space:]]*(name|aliases)[[:space:]]*=' "$manifest" \
            | grep -oE '"[^"]+"' | tr -d '"' | grep -qx -- "$wanted"; then
            return 0
        fi
    done
    return 1
}

reindex() {
    if [[ -x "$DEVTOOL" ]]; then
        "$DEVTOOL" commands index 2>/dev/null \
            || warn "could not update commands/INDEX.md now (rebuild devtool: ./scripts/devtool.sh --force). It is regenerated at the next build."
    else
        warn "scripts/devtool not built: commands/INDEX.md will be regenerated at the next build."
    fi
}

list_commands() {
    local manifest dir
    for manifest in "$COMMANDS_DIR"/*/Cargo.toml; do
        [[ -f "$manifest" ]] || continue
        dir="$(basename "$(dirname "$manifest")")"
        [[ "$dir" == _* ]] && continue
        printf '%s\n' "$dir"
    done
}

# --- arguments -----------------------------------------------------------
NAME=""; INSTALL="bin"; ALIASES=""; SETUID=0; DESCRIPTION=""
MODE="new"

while [[ $# -gt 0 ]]; do
    case "$1" in
        -h|--help)   usage; exit 0 ;;
        --list)      MODE="list" ;;
        --reindex)   MODE="reindex" ;;
        --remove)    MODE="remove" ;;
        --sbin)      INSTALL="sbin" ;;
        --bin)       INSTALL="bin" ;;
        --setuid)    SETUID=1 ;;
        --alias|--aliases)
            [[ $# -ge 2 ]] || die "$1 needs a value (e.g. --alias a,b)"
            ALIASES="$2"; shift ;;
        -d|--description)
            [[ $# -ge 2 ]] || die "$1 needs a value"
            DESCRIPTION="$2"; shift ;;
        -*)          die "unknown option: $1 (see --help)" ;;
        *)
            [[ -z "$NAME" ]] || die "only one command name at a time (got '$NAME' and '$1')"
            NAME="$1" ;;
    esac
    shift
done

[[ -d "$COMMANDS_DIR" ]] || die "commands/ folder not found: $COMMANDS_DIR"

case "$MODE" in
    list)    list_commands; exit 0 ;;
    reindex) reindex; exit 0 ;;
    remove)
        [[ -n "$NAME" ]] || die "usage: ./newcmds.sh --remove <name>"
        valid_name "$NAME" || die "invalid command name: $NAME"
        [[ -d "$COMMANDS_DIR/$NAME" ]] || die "no such command: commands/$NAME"
        rm -rf -- "${COMMANDS_DIR:?}/$NAME"
        say "commands/$NAME removed."
        reindex
        exit 0 ;;
esac

# --- creation --------------------------------------------------------------
[[ -n "$NAME" ]] || { usage; exit 1; }
valid_name "$NAME" || die "invalid command name '$NAME' (lowercase letters, digits and - _ . + only)"
[[ ! -e "$COMMANDS_DIR/$NAME" ]] || die "commands/$NAME already exists"
name_taken "$NAME" && die "the name '$NAME' is already used by another command or alias"

ALIAS_LIST=()
if [[ -n "$ALIASES" ]]; then
    IFS=',' read -r -a ALIAS_LIST <<<"$ALIASES"
    for alias in "${ALIAS_LIST[@]}"; do
        valid_name "$alias" || die "invalid alias '$alias'"
        [[ "$alias" != "$NAME" ]] || die "alias '$alias' is the command name"
        name_taken "$alias" && die "the alias '$alias' is already used by another command"
    done
fi

[[ -n "$DESCRIPTION" ]] || DESCRIPTION="Senbit command $NAME"
DESCRIPTION="${DESCRIPTION//\"/\'}"     # keeps Cargo.toml valid

DIR="$COMMANDS_DIR/$NAME"
mkdir -p "$DIR/src"

{
    printf '[package]\n'
    printf 'name = "%s"\n' "$NAME"
    printf 'version = "0.1.0"\n'
    printf 'edition = "2024"\n'
    printf 'description = "%s"\n' "$DESCRIPTION"
    printf '\n[package.metadata.senbit]\n'
    printf 'install = "%s"\n' "$INSTALL"
    if [[ ${#ALIAS_LIST[@]} -gt 0 ]]; then
        printf 'aliases = ['
        for i in "${!ALIAS_LIST[@]}"; do
            [[ $i -eq 0 ]] || printf ', '
            printf '"%s"' "${ALIAS_LIST[$i]}"
        done
        printf ']\n'
    fi
    [[ $SETUID -eq 1 ]] && printf 'setuid = true\n'
    printf '\n[dependencies]\n'
} >"$DIR/Cargo.toml"

cat >"$DIR/src/main.rs" <<RS
//! $NAME - $DESCRIPTION
//!
//! Folder: commands/$NAME/  (detected and compiled automatically by devtool)
//! Installed as: /usr/$INSTALL/$NAME

use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();

    if args.iter().any(|a| a == "-h" || a == "--help") {
        println!("usage: $NAME [options]");
        return ExitCode::SUCCESS;
    }
    if args.iter().any(|a| a == "-V" || a == "--version") {
        println!("$NAME {}", env!("CARGO_PKG_VERSION"));
        return ExitCode::SUCCESS;
    }

    // TODO: implement $NAME here.
    println!("$NAME: not implemented yet");
    ExitCode::SUCCESS
}
RS

say "commands/$NAME created (installed in /usr/$INSTALL)."
reindex

cat <<EOT

Next:
  1. edit      commands/$NAME/src/main.rs
  2. build     ./scripts/devtool build        (or: ./scripts/devtool build commands)
  3. dependencies: add them under [dependencies] in commands/$NAME/Cargo.toml
EOT
