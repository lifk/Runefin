#!/bin/sh
# Runs Runefin in the Media session, maps its exit-code ABI, and falls back to Desktop.
# It is started by KWin's --exit-with-session, outside the Flatpak sandbox.
set -u

state="${XDG_STATE_HOME:-$HOME/.local/state}/runefin"
log="$state/media.log"
mode_file="$state/mode"
fails_file="$state/media-fails"
ready_host="$state/media-ready"
ready_flatpak="$HOME/.var/app/io.github.lifk.Runefin/cache/runefin/media-ready"
ready_cache="${XDG_CACHE_HOME:-$HOME/.cache}/runefin/media-ready"
app="io.github.lifk.Runefin"
flatpak="$(command -v flatpak || echo /usr/bin/flatpak)"
mkdir -p "$state" 2>/dev/null || true
log_line() { printf '%s %s\n' "$(date -Is 2>/dev/null || date)" "$*" >>"$log" 2>/dev/null || true; }

fails="$(cat "$fails_file" 2>/dev/null || echo 0)"
case "$fails" in ''|*[!0-9]*) fails=0 ;; esac
log_line "media start; previous failures=$fails"

ready_now() {
    [ -f "$ready_host" ] || [ -f "$ready_flatpak" ] || [ -f "$ready_cache" ]
}
clear_ready() {
    rm -f "$ready_host" "$ready_flatpak" "$ready_cache" 2>/dev/null || true
}
write_mode() {
    printf '%s\n' "$1" >"$mode_file" 2>/dev/null || true
}
fallback_desktop() {
    write_mode desktop
    exit 0
}
trap 'kill "${pid:-}" 2>/dev/null || true; exit 0' TERM INT

while :; do
    clear_ready
    started="$(date +%s 2>/dev/null || echo 0)"
    "$flatpak" run --branch=stable --arch=x86_64 "$app" --media >>"$log" 2>&1 &
    pid=$!
    ready=0
    count=0
    while [ "$count" -lt 15 ]; do
        if ready_now; then
            ready=1
            break
        fi
        if ! kill -0 "$pid" 2>/dev/null; then
            break
        fi
        sleep 1
        count=$((count + 1))
    done
    if [ "$ready" -eq 0 ] && kill -0 "$pid" 2>/dev/null; then
        log_line "no frame in 15 s; stopping the app"
        kill "$pid" 2>/dev/null || true
    fi
    wait "$pid" 2>/dev/null
    rc=$?
    elapsed=0
    if [ "$started" -gt 0 ]; then
        now="$(date +%s 2>/dev/null || echo "$started")"
        elapsed=$((now - started))
    fi
    case "$rc" in
        42)
            log_line "app asked for game mode"
            write_mode game
            exit 0
            ;;
        43)
            log_line "app asked for desktop mode"
            write_mode desktop
            exit 0
            ;;
    esac

    if [ "$ready" -eq 1 ]; then
        # A frame proves the session; reset the KWin boot guard and the app loop guard.
        printf '0\n' >"$fails_file" 2>/dev/null || true
        fails=0
        : >"$ready_host" 2>/dev/null || true
    fi

    if [ "$rc" -eq 0 ] && [ "$ready" -eq 1 ]; then
        log_line "app exited 0 after a frame; restarting"
        sleep 1
        continue
    fi

    # A crash after a long healthy run restarts once without counting as a loop. A crash
    # inside the first minute, or a launch that never drew, counts.
    if [ "$ready" -eq 1 ] && [ "$elapsed" -ge 60 ]; then
        fails=0
    else
        fails=$((fails + 1))
    fi
    printf '%s\n' "$fails" >"$fails_file" 2>/dev/null || true
    log_line "app exit $rc after ${elapsed}s; failures=$fails"
    if [ "$fails" -ge 3 ]; then
        log_line "three failures; falling back to desktop"
        fallback_desktop
    fi
    sleep 2
done
