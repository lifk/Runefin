#!/bin/sh
# The one SDDM session body: Desktop, Gaming or Media, chosen by the mode marker.
# It is installed under ~/.local/share/runefin/modes and runs as the login user.
set -u

state="${XDG_STATE_HOME:-$HOME/.local/state}/runefin"
mode_file="$state/mode"
log="$state/session.log"
mkdir -p "$state" 2>/dev/null || true
log_line() { printf '%s %s\n' "$(date -Is 2>/dev/null || date)" "$*" >>"$log" 2>/dev/null || true; }

mode="$(cat "$mode_file" 2>/dev/null || echo desktop)"
case "$mode" in
    media|desktop|game) ;;
    *) mode=desktop ;;
esac
# A bad marker must never strand the machine in the kiosk.
printf '%s\n' "$mode" >"$mode_file" 2>/dev/null || true
log_line "session start: $mode"

# KWin failing before the supervisor runs has no app exit code to count. The supervisor
# writes media-ready; a pending boot without a newer ready file is one failure.
media_pending="$state/media-pending"
media_ready="$state/media-ready"
media_fails_file="$state/media-fails"
media_fails="$(cat "$media_fails_file" 2>/dev/null || echo 0)"
case "$media_fails" in ''|*[!0-9]*) media_fails=0 ;; esac
if [ "$mode" = media ] && [ -f "$media_pending" ] && [ ! -f "$media_ready" ]; then
    media_fails=$((media_fails + 1))
    printf '%s\n' "$media_fails" >"$media_fails_file" 2>/dev/null || true
    log_line "media did not reach a frame; failures=$media_fails"
fi
if [ "$mode" = media ] && [ "$media_fails" -ge 3 ]; then
    log_line "media failed three times; falling back to desktop"
    printf 'desktop\n' >"$mode_file" 2>/dev/null || true
    mode=desktop
fi

case "$mode" in
    desktop)
        export XDG_SESSION_DESKTOP=plasma XDG_CURRENT_DESKTOP=KDE
        exec startplasma-wayland
        ;;
    game)
        export XDG_SESSION_DESKTOP=gamescope XDG_CURRENT_DESKTOP=gamescope
        if command -v gamescope-session >/dev/null 2>&1; then
            exec gamescope-session
        elif [ -x /usr/lib/steamos/gamescope-session ]; then
            exec /usr/lib/steamos/gamescope-session
        else
            log_line "gamescope-session missing; falling back to desktop"
            printf 'desktop\n' >"$mode_file" 2>/dev/null || true
            export XDG_SESSION_DESKTOP=plasma XDG_CURRENT_DESKTOP=KDE
            exec startplasma-wayland
        fi
        ;;
    media)
        export XDG_SESSION_DESKTOP=runefin-modes XDG_CURRENT_DESKTOP=KDE RUNEFIN_MEDIA=1
        if ! command -v kwin_wayland >/dev/null 2>&1; then
            log_line "kwin_wayland missing; falling back to desktop"
            printf 'desktop\n' >"$mode_file" 2>/dev/null || true
            exec startplasma-wayland
        fi
        rm -f "$media_ready" 2>/dev/null || true
        date +%s >"$media_pending" 2>/dev/null || true
        exec kwin_wayland --exit-with-session "$HOME/.local/share/runefin/modes/supervisor.sh"
        ;;
esac
