#!/usr/bin/env bash
# The performance pass: what a launch costs, what a screen costs to paint, and what the server
# costs on the way. Numbers print as `bench ...` lines; the tail of this script summarises them.
#
#   scripts/bench.sh                 # launch + the screens this machine's session can open
#   scripts/bench.sh --runs 5        # more launches, for a median that means something
#
# Live screens need a signed-in config (the app's own). Set the ids to measure a particular
# library, item or query; a screen whose id is unset is skipped:
#
#   RUNEFIN_LIVE_LIBRARY_ID=… RUNEFIN_LIVE_LIBRARY_NAME=Movies scripts/bench.sh
#   RUNEFIN_LIVE_ITEM_ID=… scripts/bench.sh
#
# The player is not here: it needs a window and audio. Its own meters cover it —
# `RUNEFIN_TRACE_PLAYBACK=1` prints sink pacing every two seconds, and `RUNEFIN_BENCH=1` prints
# the same launch and frame lines a window does. Play something with
# `RUNEFIN_LIVE_ROUTE=play RUNEFIN_LIVE_ITEM_ID=<id>`, muted through `volume = 0.0` in the config.
set -euo pipefail
cd "$(dirname "$0")/.."

runs=3
while [ $# -gt 0 ]; do
    case "$1" in
        --runs) runs=$2; shift 2 ;;
        *) echo "usage: $0 [--runs N]" >&2; exit 2 ;;
    esac
done

bin=target/release/runefin
cargo build --release -p runefin >/dev/null
export RUNEFIN_BENCH=1

# One launch, its marks and its first frame. Killed after it has painted and rested.
launch() {
    timeout 6 "$bin" 2>&1 | grep '^bench' || true
}

echo "== launch (windowed, $runs runs)"
last=/tmp/runefin-bench-launch.txt
for _ in $(seq "$runs"); do
    launch > "$last"
    grep '^bench launch total_ms' "$last"
done

echo
echo "== launch breakdown (last run)"
sed -n 's/^bench /  /p' "$last" | grep -E '^  (launch|mark|frame|metric|note)'

# One live screen, painted once from the server's data. `--wait` lets the requests answer before
# the frame is frozen; the numbers that matter are the data's arrival and the frame's own cost.
screen() {
    local name=$1
    shift
    if env "$@" "$bin" --snap "/tmp/runefin-bench-$name.png" --live --wait 3 \
        --size 1280x800 --scale 2 2>&1 |
        grep -E "^bench (landed|net (libraries|resume|rails|watching|recently_played)|frame (update|metric text\.(glyphs|raster_ns)))|layout nodes"; then
        :
    fi
}

echo
echo "== home"
screen home X=1

if [ -n "${RUNEFIN_LIVE_LIBRARY_ID:-}" ]; then
    echo
    echo "== library ${RUNEFIN_LIVE_LIBRARY_NAME:-}"
    screen library RUNEFIN_LIVE_ROUTE=library \
        RUNEFIN_LIVE_LIBRARY_ID="$RUNEFIN_LIVE_LIBRARY_ID" \
        RUNEFIN_LIVE_LIBRARY_NAME="${RUNEFIN_LIVE_LIBRARY_NAME:-Movies}" \
        RUNEFIN_LIVE_LIBRARY_TYPE="${RUNEFIN_LIVE_LIBRARY_TYPE:-movies}"
fi

if [ -n "${RUNEFIN_LIVE_ITEM_ID:-}" ]; then
    echo
    echo "== detail ${RUNEFIN_LIVE_ITEM_ID}"
    screen detail RUNEFIN_LIVE_ROUTE=detail RUNEFIN_LIVE_ITEM_ID="$RUNEFIN_LIVE_ITEM_ID"
fi

echo
echo "== search ${RUNEFIN_LIVE_QUERY:-dune}"
screen search RUNEFIN_LIVE_ROUTE=search RUNEFIN_LIVE_QUERY="${RUNEFIN_LIVE_QUERY:-dune}"

echo
echo "Read it as: launch total is process start to the first presented frame, and a mark is when"
echo "a step inside it finished. In a headless screen a landed line is when the frozen render"
echo "applied the data (after --wait), so the network numbers are the net lines, and frame"
echo "update/encode is the paint and upload, with text.raster_ns the type inside the paint."
