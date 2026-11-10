#!/usr/bin/env bash
# Renders the UI headlessly (no window, fixed virtual clock).
#
#   scripts/snapshots.sh             # the eight committed views -> docs/snapshots/
#   scripts/snapshots.sh --all       # every scene -> /tmp/runefin-snapshots
#   scripts/snapshots.sh --all DIR   # every scene -> DIR
#
# Only the core set is committed: it is the contract a reviewer reads in the pull request.
# Everything else stays a local tool, so the repository does not carry 29 MB of PNGs that
# nobody diffs. When a change touches a screen, render it with --all and look at it.
set -euo pipefail
cd "$(dirname "$0")/.."

all=false
out=docs/snapshots
case "${1:-}" in
    --all)
        all=true
        out="${2:-${TMPDIR:-/tmp}/runefin-snapshots}"
        ;;
    "") ;;
    *)
        echo "usage: $0 [--all [DIR]]" >&2
        exit 2
        ;;
esac

echo "writing to $out"
mkdir -p "$out"

cargo build -p runefin
bin=target/debug/runefin

render() {
    local scene=$1 name=$2 width=$3 height=$4
    "$bin" --snap "$out/$name.png" --scene "$scene" --size "${width}x${height}" --scale 2
    echo "wrote $out/$name.png"
}

# Big screen: the same point layout at a 1280x720 design height, 1.5x, so the PNG is a 1080p TV
# frame. The mode is a per-process flag; each invocation is its own scene.
render_big() {
    local scene=$1 name=$2
    "$bin" --snap "$out/$name-big.png" --scene "$scene" --size 1280x720 --scale 1.5 --big-screen
    echo "wrote $out/$name-big.png"
}

# The same frames at 4K: the design height is unchanged, so a layout bug that only appears when
# the scale is 3x (focus rings, hairlines, glyph hinting) is still caught.
render_big_4k() {
    local scene=$1 name=$2
    "$bin" --snap "$out/$name-big-4k.png" --scene "$scene" --size 1280x720 --scale 3 --big-screen
    echo "wrote $out/$name-big-4k.png"
}

# Focus is runtime state, not scene state: these render past the first poll so the app's focus
# request lands, and the optional script walks the router before the frame.
render_focus() {
    local scene=$1 name=$2 script=${3:-}
    local args=(--snap "$out/$name-big.png" --scene "$scene" --size 1280x720 --scale 1.5
        --big-screen --at 0.6)
    if [ -n "$script" ]; then
        args+=(--script "$script")
    fi
    "$bin" "${args[@]}"
    echo "wrote $out/$name-big.png"
}

# The committed set: one frame per main view, the two big-screen frames the README and the
# AppStream metadata link to, and the focus pair that pins the router's visual contract.
core() {
    local focus_script
    focus_script=$(mktemp)
    printf '%s\n' '{"events":[{"t":0.15,"type":"key","name":"Up"}]}' > "$focus_script"
    render home home 1280 800
    render detail detail 1280 800
    render search search 1280 800
    render live-tv live-tv 1280 800
    render settings settings 1280 800
    render in-app-osd in-app-osd 1280 800
    render_big home home
    render_big library library
    render_focus home focus-card
    render_focus home focus-sidebar "$focus_script"
    rm -f "$focus_script"
}

# Every scene, for local review only.
all() {
# The connect screen is a small window; the home shell needs room for three rails.
render idle connect-idle 560 520
render connect-discovery connect-discovery 1280 800
render connect-quick connect-quick 560 520
render signing-in connect-signing-in 560 520
render error connect-error 560 520
render home home 1280 800
render home-art home-art 1280 800
render home-art-light home-art-light 1280 800
render detail-art detail-art 1280 800
render home-contract home-contract 1280 800
render home-hero home-hero 1280 800
render home-light home-light 1280 800
render home-contrast home-contrast 1280 800
render toast-undo toast-undo 1280 800
render home-shelf home-shelf 1280 800
render home-collections home-collections 1280 800
render card-menu card-menu 1280 800
render toast toast 1280 800
render cast-picker cast-picker 1280 800
render home home-1440 1440 900
render home-collapsed home-collapsed 1280 800
render home-loading home-loading 1280 800
render home-empty home-empty 1280 800
render home-error home-error 1280 800
render library library 1280 800
render library library-narrow 800 600
render library-shows library-shows 1280 800
render playlist playlist 1280 800
render playlist-edit playlist-edit 1280 800
render playlist-picker playlist-picker 1280 800
render playlist-create playlist-create 1280 800
render library-genres library-genres 1280 800
render library-filters library-filters 1280 800
render library-filters-views library-filters-views 1280 800
render library-select library-select 1280 800
render library-badges library-badges 1280 800
render watchlist watchlist 1280 800
render library library-1024 1024 768
render library library-1440 1440 900
render library-list library-list 1280 800
render library-loading library-loading 1280 800
render library-empty library-empty 1280 800
render library-error library-error 1280 800
render detail detail 1280 800
render detail-preplay detail-preplay 1280 800
render detail-versions detail-versions 1280 800
render detail-extras detail-extras 1280 800
render detail-collections detail-collections 1280 800
render detail-theme detail-theme 1280 800
render person person 1280 800
render person-seerr person-seerr 1280 1000
render seerr-person seerr-person 1280 800
render detail-tech detail-tech 1280 800
render detail-similar detail-similar 1280 800
render detail-similar detail-narrow 800 600
render collection collection 1280 800
render detail-loading detail-loading 1280 800
render detail-error detail-error 1280 800
render now-playing now-playing 1280 800
render now-playing-paused now-playing-paused 1280 800
render search search 1280 800
render search-seerr search-seerr 1280 900
render search-expanded search-expanded 1280 800
render search-idle search-idle 1280 800
render search-hints search-hints 1280 800
render search-empty search-empty 1280 800
render settings settings 1280 800
render settings settings-keyboard 1280 2600
render settings-seerr settings-seerr 1280 3600
render remote remote 1280 800
render in-app-launching in-app-launching 1280 800
render in-app-osd in-app-osd 1280 800
render in-app-mini in-app-mini 1280 800
render in-app-intro in-app-intro 1280 800
render in-app-gear in-app-gear 1280 800
render in-app-skip-credits in-app-skip-credits 1280 800
render in-app-segments in-app-segments 1280 800
render in-app-still-watching in-app-still-watching 1280 800
render in-app-sleep-timer in-app-sleep-timer 1280 800
render in-app-preplay in-app-preplay 1280 800
render in-app-resume-conflict in-app-resume-conflict 1280 800
render in-app-transfer in-app-transfer 1280 800
render in-app-audio-modes in-app-audio-modes 1280 800
render in-app-quality in-app-quality 1280 800
render in-app-loop in-app-loop 1280 800
render in-app-goto in-app-goto 1280 800
render in-app-why in-app-why 1280 800
render in-app-trickplay in-app-trickplay 1280 800
render in-app-subtitles in-app-subtitles 1280 800
render in-app-subtitle-style in-app-subtitle-style 1280 800
render in-app-image-subtitle in-app-image-subtitle 1280 800
render in-app-subtitles-clear in-app-subtitles-clear 1280 800
render in-app-subtitles-ass in-app-subtitles-ass 1280 800
render in-app-subtitle-panel in-app-subtitle-panel 1280 800
render in-app-subtitle-many in-app-subtitle-many 1280 800
render in-app-next in-app-next 1280 800
render in-app-audio in-app-audio 1280 800
render in-app-stats in-app-stats 1280 800
render in-app-subtitle-search in-app-subtitle-search 1280 800
render in-app-chapters in-app-chapters 1280 800
render in-app-queue in-app-queue 1280 800
render in-app-queue-panel in-app-queue-panel 1280 800
render in-app-buffering in-app-buffering 1280 800
render in-app-narrow in-app-narrow 1024 700
render in-app-narrow in-app-narrow-small 800 600
render in-app-live in-app-live 1280 800
render live-tv live-tv 1280 800
render live-tv-favorites live-tv-favorites 1280 800
render live-guide live-guide 1280 800
render live-guide-day live-guide-day 1280 800
render shortcuts shortcuts 1280 800

# Seerr companion: catalogue, request panel and progress rows.
render seerr seerr 1280 800
render seerr-browse seerr-browse 1280 900
render seerr-search seerr-search 1280 800
render seerr-detail-tv seerr-detail-tv 1280 800
render seerr-detail-movie seerr-detail-movie 1280 800
render requests requests 1280 800
render requests-live requests-live 1280 800
render requests-empty requests-empty 1280 800
render requests-error requests-error 1280 800
render requests-unconfigured requests-unconfigured 1280 800
render requests requests-narrow 800 600
render seerr-130 seerr-130 1280 800
render detail-seerr detail-seerr 1280 800

# Big-screen frames (1080p). 4K is generated on demand with --scale 3; the layout is identical.
render_big home home
render_big home-art home-art
render_big home-art-light home-art-light
render_big detail-art detail-art
render_big library library
render_big detail detail
render_big search search
render_big settings settings
render_big live-tv live-tv
render_big idle connect-idle
render_big in-app-osd in-app-osd
render_big in-app-gear in-app-gear
render_big card-menu card-menu
render_big shortcuts shortcuts

render_big_4k home home
render_big_4k home-art home-art
render_big_4k home-art-light home-art-light
render_big_4k detail-art detail-art
render_big_4k library library
render_big_4k detail detail
render_big_4k search search
render_big_4k settings settings
render_big_4k live-tv live-tv
render_big_4k idle connect-idle
render_big_4k in-app-osd in-app-osd
}

if $all; then
    all
else
    core
fi
