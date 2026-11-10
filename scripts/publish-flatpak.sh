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

# The file a client opens once: `flatpak remote-add --if-not-exists --user runefin <file>`.
{
    echo '[Flatpak Repo]'
    echo 'Title=Runefin'
    echo "Url=$repo_url"
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
git -C "$pages" remote add origin https://github.com/lifk/Runefin.git
git -C "$pages" push -q --force origin gh-pages
say "published $(du -sh "$pages" | cut -f1) to $repo_url"
say "clients: flatpak remote-add --if-not-exists --user runefin ${repo_url}runefin.flatpakrepo"
say "         flatpak install --user runefin io.github.lifk.Runefin"
say "updates: flatpak update --user"
