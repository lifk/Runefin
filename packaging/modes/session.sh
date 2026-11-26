#!/bin/sh
# The one SDDM session body: Desktop, Gaming or Media, chosen by the mode marker.
# It is installed under ~/.local/share/runefin/modes and runs as the login user.
set -u
export XDG_SESSION_TYPE=wayland

home="${HOME:-/home/deck}"
state="${XDG_STATE_HOME:-$home/.local/state}/runefin"
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
        export XDG_SESSION_DESKTOP=plasma XDG_CURRENT_DESKTOP=KDE DESKTOP_SESSION=plasma
        if [ -x /usr/lib/plasma-dbus-run-session-if-needed ]; then
            exec /usr/lib/plasma-dbus-run-session-if-needed /usr/bin/startplasma-wayland
        fi
        exec /usr/bin/startplasma-wayland
        ;;
    game)
        export XDG_SESSION_DESKTOP=gamescope XDG_CURRENT_DESKTOP=gamescope DESKTOP_SESSION=gamescope
        # SteamOS 3.9 runs its session through start-gamescope-session: it sets
        # XDG_SESSION_TYPE=x11, updates the systemd environment, sets the gamescope
        # portal and starts the user target. Running gamescope-session directly skips
        # all of that and hangs at the splash.
        if [ -x /usr/bin/start-gamescope-session ]; then
            exec /usr/bin/start-gamescope-session
        elif command -v gamescope-session >/dev/null 2>&1; then
            exec gamescope-session
        elif [ -x /usr/lib/steamos/gamescope-session ]; then
            exec /usr/lib/steamos/gamescope-session
        else
            log_line "gamescope-session missing; falling back to desktop"
            printf 'desktop\n' >"$mode_file" 2>/dev/null || true
            export XDG_SESSION_DESKTOP=plasma XDG_CURRENT_DESKTOP=KDE DESKTOP_SESSION=plasma
            if [ -x /usr/lib/plasma-dbus-run-session-if-needed ]; then
                exec /usr/lib/plasma-dbus-run-session-if-needed /usr/bin/startplasma-wayland
            fi
            exec startplasma-wayland
        fi
        ;;
    media)
        export XDG_SESSION_DESKTOP=runefin-modes XDG_CURRENT_DESKTOP=KDE RUNEFIN_MEDIA=1
        if ! command -v kwin_wayland >/dev/null 2>&1; then
            log_line "kwin_wayland missing; falling back to desktop"
            printf 'desktop\n' >"$mode_file" 2>/dev/null || true
            export XDG_SESSION_DESKTOP=plasma XDG_CURRENT_DESKTOP=KDE DESKTOP_SESSION=plasma
            if [ -x /usr/lib/plasma-dbus-run-session-if-needed ]; then
                exec /usr/lib/plasma-dbus-run-session-if-needed /usr/bin/startplasma-wayland
            fi
            exec /usr/bin/startplasma-wayland
        fi
        rm -f "$media_ready" 2>/dev/null || true
        date +%s >"$media_pending" 2>/dev/null || true
        exec kwin_wayland --exit-with-session "$home/.local/share/runefin/modes/supervisor.sh"
        ;;
esac
