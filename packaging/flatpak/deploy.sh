#!/usr/bin/env bash
# Build and deploy a local Flatpak repository to a Steam Machine over SSH.
#
#   packaging/flatpak/deploy.sh deck@steam-machine
#
# The first run adds a `runefin` remote and installs the app; later runs sync
# the repository and `flatpak update` it. No web server, no root.
#
# Optional signing:
#   GNUPGHOME=... FLATPAK_GPG_KEY=... FLATPAK_GPG_KEY_FILE=... \
#     packaging/flatpak/deploy.sh deck@steam-machine
set -euo pipefail
cd "$(dirname "$0")/../.."

target=${1:?usage: packaging/flatpak/deploy.sh user@steam-machine}
app=io.github.lifk.Runefin
remote=runefin
remote_dir=.local/share/runefin-repo
remote_stage=.local/share/runefin-repo.stage

./packaging/flatpak/build.sh

# tar over SSH avoids assuming rsync is installed on the immutable SteamOS image.
ssh "$target" "rm -rf ~/$remote_stage && mkdir -p ~/$remote_stage"
tar -C dist/flatpak-repo -cf - . | ssh "$target" "tar -C ~/$remote_stage -xf -"
if [[ -n "${FLATPAK_GPG_KEY_FILE:-}" ]]; then
    ssh "$target" "cat > ~/$remote_stage/runefin.gpg" < "$FLATPAK_GPG_KEY_FILE"
fi
ssh "$target" "rm -rf ~/$remote_dir.old && \
    mv ~/$remote_dir ~/$remote_dir.old 2>/dev/null || true; \
    mv ~/$remote_stage ~/$remote_dir && \
    rm -rf ~/$remote_dir.old"

ssh "$target" bash -s -- "$app" "$remote" "$remote_dir" <<'REMOTE'
set -euo pipefail
app=$1
remote=$2
remote_dir=$3
repo="$HOME/$remote_dir"

if [[ -f "$repo/runefin.gpg" ]]; then
    flatpak remote-add --user --if-not-exists --gpg-import="$repo/runefin.gpg" "$remote" "file://$repo"
    flatpak remote-modify --user --gpg-import="$repo/runefin.gpg" "$remote"
else
    flatpak remote-add --user --if-not-exists --no-gpg-verify "$remote" "file://$repo"
    flatpak remote-modify --user --no-gpg-verify "$remote"
fi

origin=$(flatpak list --user --app --columns=application,origin | awk -v app="$app" '$1 == app { print $2; exit }')
if [[ -z "$origin" ]]; then
    flatpak install --user -y "$remote" "$app"
elif [[ "$origin" == "$remote" ]]; then
    flatpak update --user -y "$app"
else
    echo "Runefin is installed from '$origin', not '$remote'." >&2
    echo "Uninstall it once, then rerun this script:" >&2
    echo "  flatpak uninstall --user $app" >&2
    exit 1
fi
flatpak info --user "$app" | grep -E 'Version|Installed Size|Origin' || true
REMOTE

echo
echo "Run it with:"
echo "  flatpak run $app --big-screen"
echo "Update later with:"
echo "  packaging/flatpak/deploy.sh $target"
