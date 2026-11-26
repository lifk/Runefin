#!/bin/sh
# Valve's Game Mode script is built for the Deck's 1280x800 internal panel: it hardcodes that
# canvas and prefers eDP-1. On a TV that is a 16:10 canvas shown on a 16:9 panel, so 16:9 Game
# Mode content gets bars above and below and the whole interface is upscaled. Rewrite the canvas
# to the connected display's preferred mode and prefer that connector, then run the stock script
# with every other line intact. A Deck on its own panel resolves to the same 1280x800, so this
# changes nothing there.
set -u

stock=/usr/lib/steamos/gamescope-session
if [ ! -r "$stock" ]; then
    exec /usr/bin/start-gamescope-session
fi

mode=""
connector=""
for status in /sys/class/drm/card*-*/status; do
    [ "$(cat "$status" 2>/dev/null)" = connected ] || continue
    dir=${status%/status}
    [ -s "$dir/modes" ] || continue
    mode=$(head -n 1 "$dir/modes" 2>/dev/null)
    connector=${dir##*-}
    break
done

w=${mode%x*}
h=${mode#*x}
case "${w:-x}${h:-x}" in
    *[!0-9]*|x)
        # No usable mode: leave Valve's script exactly as shipped.
        exec "$stock"
        ;;
esac

runtime="${XDG_RUNTIME_DIR:-/tmp}/runefin-gamescope-session.$$"
sed -e "s/-w 1280 -h 800/-w $w -h $h/" \
    -e "s/-O '\*',eDP-1/-O $connector/" \
    "$stock" >"$runtime" 2>/dev/null || exec "$stock"
chmod 755 "$runtime" 2>/dev/null || exec "$stock"
exec "$runtime"
