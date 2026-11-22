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

# Every gitlink (mode 160000) in the index is a nested repository recorded as a
# submodule. Hosts that initialise submodules (Cloudflare Pages, for example) stop with
# "No url found for submodule path ..." when .gitmodules does not describe it.
while read -r mode _ _ path; do
    [ "$mode" = "160000" ] || continue
    git rm --cached -q -- "$path"
    echo "removed submodule entry: $path"
    git config --remove-section "submodule.$path" 2>/dev/null || true
    rm -rf ".git/modules/$path" 2>/dev/null || true
done < <(git ls-files --stage)

# .gitmodules is useless once no submodule is left.
if git ls-files --error-unmatch -- .gitmodules >/dev/null 2>&1; then
    if ! git ls-files --stage | grep -q '^160000'; then
        git rm --cached -q -- .gitmodules
        echo "untracked: .gitmodules"
    fi
fi

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
