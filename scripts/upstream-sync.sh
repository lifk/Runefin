#!/usr/bin/env bash
# Rebase our Rune fork onto upstream dev and prove it still builds.
#
# Run weekly, and always before starting Rune work. Review the diff after rebasing (see
# docs/design-player.md §7).
set -euo pipefail

fork=${RUNE_FORK:-$HOME/Projects/rune}
branch=${RUNE_BRANCH:-runefin/player}
upstream_url=${RUNE_UPSTREAM:-https://github.com/anomalyco/rune.git}


cd "$fork"

if ! git remote get-url upstream >/dev/null 2>&1; then
    git remote add upstream "$upstream_url"
    echo "added remote upstream -> $upstream_url"
fi
git fetch --prune upstream

# `dev` is a mirror: fast-forward only. A local commit on dev means someone edited upstream's
# branch by accident, and the next rebase will be a mess.
git checkout -q dev
git merge --ff-only upstream/dev

git checkout -q "$branch"
if ! git rebase dev; then
    echo
    echo "rebase stopped at a conflict. Resolve it, then:"
    echo "  git rebase --continue   (or 'git rebase --abort' to back out)"
    exit 1
fi

echo
echo "patch on $branch versus dev:"
git diff --stat dev..."$branch"

if [ "${SKIP_CHECK:-0}" != "1" ]; then
    echo
    cargo check -p rune --all-targets
fi

echo
echo "pin for Runefin's Cargo.toml:"
git rev-parse "$branch"
