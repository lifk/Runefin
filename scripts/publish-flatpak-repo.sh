#!/usr/bin/env bash
# Replace the gh-pages branch with dist/flatpak-repo, which is what clients read for updates.
#
# Runs from scripts/publish-flatpak.sh after a local signed build, and from the release
# workflow after the same build inside the Flatpak sandbox. The caller builds and signs; this
# only moves the result. RUNEFIN_REPO_URL overrides the push URL (the workflow passes a token
# URL there).
set -euo pipefail
cd "$(dirname "$0")/.."

say() { printf '%s\n' "$*"; }
die() { printf 'error: %s\n' "$*" >&2; exit 1; }

repo_url=${RUNEFIN_REPO_URL:-https://github.com/lifk/Runefin.git}
[ -d dist/flatpak-repo ] || die "dist/flatpak-repo is missing; run packaging/flatpak/build.sh first"
[ -s dist/Runefin.flatpakrepo ] || die "dist/Runefin.flatpakrepo is missing"

pages=$(mktemp -d)
trap 'rm -rf "$pages"' EXIT
cp -a dist/flatpak-repo/. "$pages/"
cp dist/Runefin.flatpakrepo "$pages/runefin.flatpakrepo"
touch "$pages/.nojekyll"
git -C "$pages" init -q
git -C "$pages" checkout -q -b gh-pages
git -C "$pages" add -A
git -C "$pages" \
    -c user.name="Runefin" \
    -c user.email="lifk@users.noreply.github.com" \
    commit -qm "Flatpak repository: $(sed -n 's/^Version = //p' dist/flatpak-repo/config 2>/dev/null || date -Idate)"
git -C "$pages" remote add origin "$repo_url"
git -C "$pages" push -q --force origin gh-pages
# A push that GitHub accepts but does not move is worse than a failure: clients keep seeing the
# previous build. Compare the remote ref with what was just committed.
remote_head=$(git -C "$pages" ls-remote origin gh-pages | cut -f1)
local_head=$(git -C "$pages" rev-parse HEAD)
[ "$remote_head" = "$local_head" ] || die "gh-pages did not move: remote $remote_head, local $local_head"
say "published $(du -sh "$pages" | cut -f1) to the gh-pages branch"
