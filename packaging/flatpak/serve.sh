#!/usr/bin/env bash
# Serve dist/flatpak-repo over the LAN for fast iteration.
#
#   packaging/flatpak/serve.sh
#
# Then, on the Steam Machine, once:
#   flatpak remote-add --user --no-gpg-verify runefin http://<this-host>:8000/
#   flatpak install --user runefin io.github.lifk.Runefin
#
# Rebuild with packaging/flatpak/build.sh and run `flatpak update --user` on the
# machine; the HTTP server keeps serving the new repository. For a permanent
# repository use HTTPS and sign it (see README.md).
set -euo pipefail
cd "$(dirname "$0")/../.."

port=${PORT:-8000}
host=${HOST:-}
if [[ -z "$host" ]]; then
    host=$(ip -4 route get 1.1.1.1 2>/dev/null \
        | awk '{ for (i = 1; i <= NF; i++) if ($i == "src") { print $(i + 1); exit } }')
fi
host=${host:-127.0.0.1}

if [[ ! -f dist/flatpak-repo/config ]]; then
    ./packaging/flatpak/build.sh
fi

echo "Serving $PWD/dist/flatpak-repo on http://$host:$port/"
echo "On the Steam Machine:"
echo "  flatpak remote-add --user --if-not-exists --no-gpg-verify runefin http://$host:$port/"
echo "  flatpak install --user runefin io.github.lifk.Runefin"
echo "After a rebuild: flatpak update --user io.github.lifk.Runefin"
echo
python3 -m http.server "$port" --directory dist/flatpak-repo
