#!/usr/bin/env bash
# Build Runefin as a Flatpak. The bundle and repo land in dist/:
#   dist/Runefin.flatpak            install with `flatpak install --user`
#   dist/flatpak-repo/              host this directory to auto-update
# Optional: set FLATPAK_GPG_KEY to sign the repo and bundle.
set -euo pipefail
cd "$(dirname "$0")/../.."

manifest=packaging/flatpak/io.github.lifk.Runefin.yml
build_dir=.flatpak-build
repo=dist/flatpak-repo
bundle=dist/Runefin.flatpak
mkdir -p dist

builder=(flatpak run)
if [[ -n "${GNUPGHOME:-}" ]]; then
    builder+=(--env=GNUPGHOME="$GNUPGHOME")
fi
builder+=(--command=flatpak-builder org.flatpak.Builder)
sign=()
gpg_homedir=()
if [[ -n "${GNUPGHOME:-}" ]]; then
    gpg_homedir=(--gpg-homedir="$GNUPGHOME")
fi
if [[ -n "${FLATPAK_GPG_KEY:-}" ]]; then
    sign=(--gpg-sign="$FLATPAK_GPG_KEY")
fi

"${builder[@]}" --force-clean "${sign[@]}" "${gpg_homedir[@]}" --repo="$repo" --default-branch=stable "$build_dir" "$manifest"
flatpak build-update-repo --prune \
    --title=Runefin \
    --comment="Jellyfin client for the desktop and the big screen" \
    --homepage=https://github.com/lifk/Runefin \
    "${sign[@]}" "$repo"
flatpak build-bundle "${sign[@]}" "$repo" "$bundle" io.github.lifk.Runefin stable

# A .flatpakref Discover can open. Point FLATPAK_REPO_URL at the hosted repository when
# publishing; the default is the local repo, which is enough for a one-machine install test.
repo_url=${FLATPAK_REPO_URL:-file://$PWD/$repo}
{
    echo '[Flatpak Ref]'
    echo 'Title=Runefin'
    echo 'Name=io.github.lifk.Runefin'
    echo 'Branch=stable'
    echo "Url=$repo_url"
    echo 'IsRuntime=false'
    if [[ -n "${FLATPAK_GPG_KEY_FILE:-}" ]]; then
        echo "GPGKey=$(base64 -w0 "$FLATPAK_GPG_KEY_FILE")"
    fi
} > dist/Runefin.flatpakref

echo
echo "wrote $bundle"
echo "wrote $repo"
echo "wrote dist/Runefin.flatpakref"
echo "install: flatpak install --user $bundle"
