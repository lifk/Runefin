#!/usr/bin/env bash
# Build a signed Flatpak repository and publish it to the gh-pages branch, which GitHub Pages
# serves at https://lifk.github.io/Runefin/. One command per release:
#
#   scripts/publish-flatpak.sh
#
# The signing key lives outside the repository, in
# ~/.local/share/runefin/release-gnupg (override with RUNEFIN_GPG_HOME). Losing it means every
# client has to remove and re-add the remote, so back it up. Only the public half is committed.
#
# The pages branch is replaced on every run: a Flatpak client only needs the current commit,
# and keeping every build in git would grow the site without bound.
set -euo pipefail
cd "$(dirname "$0")/.."

say() { printf '%s\n' "$*"; }
die() { printf 'error: %s\n' "$*" >&2; exit 1; }

key_home=${RUNEFIN_GPG_HOME:-$HOME/.local/share/runefin/release-gnupg}
key_file=packaging/flatpak/runefin-releases.gpg
repo_url=${FLATPAK_REPO_URL:-https://lifk.github.io/Runefin/}

[ -d "$key_home" ] || die "no release keyring at $key_home; see the README's release-key section"
export GNUPGHOME="$key_home"
key=$(gpg --list-secret-keys --with-colons 2>/dev/null | awk -F: '/^sec:/{print $5; exit}')
[ -n "$key" ] || die "no secret key in $key_home"

gpg --export --armor "$key" > "$key_file"
say "signing with $key"

# Publish from a repository that holds only this build. The local repo accumulates every app id
# and version ever built (149 MB after a few releases); a client only needs the current commit.
rm -rf dist/flatpak-repo

GNUPGHOME="$key_home" \
FLATPAK_GPG_KEY="$key" \
FLATPAK_GPG_KEY_FILE="$PWD/$key_file" \
FLATPAK_REPO_URL="$repo_url" \
    ./packaging/flatpak/build.sh

# Publishing itself lives in one place — the .flatpakrepo, the gh-pages branch — and CI calls
# the same script, so a local publish cannot drift from a release.
FLATPAK_REPO_URL="$repo_url" ./scripts/publish-flatpak-repo.sh
