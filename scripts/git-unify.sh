#!/usr/bin/env bash
# One-time cleanup before pushing Senbit.
#
# Older versions of the repository registered the Linux / BusyBox / util-linux /
# Parted / systemd sources as Git submodules. They are now downloaded by
# `devtool get` at pinned versions and ignored by Git, so the old submodule
# entries must leave the index (the files on disk are NOT deleted).
#
# Safe to run several times. Run it from anywhere inside the repository.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

git rev-parse --is-inside-work-tree >/dev/null 2>&1 || { echo "Not a Git repository."; exit 1; }

for path in kernel/linux third_party/busybox third_party/util-linux third_party/parted external/systemd; do
    if git ls-files --stage -- "$path" | grep -q '^160000'; then
        git rm --cached -q "$path"
        echo "removed submodule entry: $path"
    fi
    git config --remove-section "submodule.$path" 2>/dev/null || true
    rm -rf ".git/modules/$path" 2>/dev/null || true
done

# Build outputs that must never be committed.
for path in devtool/bin scripts/devtool build; do
    if git ls-files --error-unmatch -- "$path" >/dev/null 2>&1; then
        git rm -r --cached -q -- "$path"
        echo "untracked build output: $path"
    fi
done

echo
echo "Done. Review with:  git status"
echo "Then commit and push as usual."
