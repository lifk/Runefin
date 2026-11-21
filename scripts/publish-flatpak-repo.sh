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
public_url=${FLATPAK_REPO_URL:-https://lifk.github.io/Runefin/}
key_file=packaging/flatpak/runefin-releases.gpg
[ -d dist/flatpak-repo ] || die "dist/flatpak-repo is missing; run packaging/flatpak/build.sh first"
[ -s "$key_file" ] || die "missing $key_file"

# The file a client opens once: `flatpak remote-add --if-not-exists --user runefin <file>`.
# Generated here, not by the caller, so a local publish and CI publish the same document.
{
    echo '[Flatpak Repo]'
    echo 'Title=Runefin'
    echo "Url=$public_url"
    echo 'Homepage=https://github.com/lifk/Runefin'
    echo 'Comment=Jellyfin client for the desktop and the big screen'
    echo "GPGKey=$(base64 -w0 "$key_file")"
} > dist/Runefin.flatpakrepo

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
