#!/usr/bin/env bash
# Install the small HDMI-CEC mapping Runefin needs into ~/.config/cecd/config.toml.
#
# cecd's mapping applies to the whole SteamOS session, so this script deliberately
# changes as little as possible: only the Menu buttons, which Runefin uses for the
# item menu. OK, Back and the D-pad are left at cecd's defaults (Return, Escape and
# the arrow keys) because Runefin's big-screen mode takes Return and arrows itself.
#
# Safe to re-run: it adds only keys that are missing and keeps values you chose.
#
#   bash setup-cecd.sh              # merge, then restart cecd if it is running
#   bash setup-cecd.sh --dry-run    # show what would change, write nothing
#   bash setup-cecd.sh --force      # also replace mappings that differ
#   bash setup-cecd.sh --restore    # undo an older install from this script
#
# It can be piped from the repository:
#   curl -fsSL https://raw.githubusercontent.com/lifk/Runefin/main/packaging/flatpak/setup-cecd.sh | bash
set -euo pipefail

dry_run=false
force=false
restore=false

for arg in "$@"; do
    case "$arg" in
        --dry-run) dry_run=true ;;
        --force) force=true ;;
        --restore) restore=true ;;
        -h|--help)
            sed -n '2,18p' "$0" 2>/dev/null | sed 's/^# \{0,1\}//'
            exit 0
            ;;
        *)
            echo "unknown option: $arg (try --help)" >&2
            exit 2
            ;;
    esac
done

say() { printf '%s\n' "$*"; }
warn() { printf 'warning: %s\n' "$*" >&2; }
die() { printf 'error: %s\n' "$*" >&2; exit 1; }

# --- environment checks ---------------------------------------------------------------

if [ "$(id -u)" -eq 0 ]; then
    die "run this as your desktop user, not with sudo; it installs into your home"
fi
[ -n "${HOME:-}" ] || die "HOME is not set"

os_id=unknown
if [ -r /etc/os-release ]; then
    os_id=$(. /etc/os-release 2>/dev/null && printf '%s' "${ID:-unknown}")
fi
case "$os_id" in
    steamos) ;;
    *) warn "this looks like '$os_id', not SteamOS; the mapping is still installed" ;;
esac

have_user_unit=false
have_system_unit=false
if command -v systemctl >/dev/null 2>&1; then
    systemctl --user list-unit-files cecd.service >/dev/null 2>&1 && have_user_unit=true
    systemctl list-unit-files cecd.service >/dev/null 2>&1 && have_system_unit=true
fi
have_binary=false
command -v cecd >/dev/null 2>&1 && have_binary=true
if ! $have_user_unit && ! $have_system_unit && ! $have_binary && ! $restore; then
    warn "cecd was not found as a service or a command"
    warn "the file is still written; it takes effect when cecd exists (SteamOS 3.8.1+)"
fi

config_dir="${XDG_CONFIG_HOME:-$HOME/.config}/cecd"
target="$config_dir/config.toml"
case "$target" in
    "$HOME"/*) ;;
    *) die "refusing to write outside your home: $target" ;;
esac
if [ -e "$target" ] && [ ! -f "$target" ]; then
    die "$target exists but is not a regular file"
fi
if [ -f "$target" ] && [ ! -r "$target" ]; then
    die "$target is not readable"
fi
if [ -f "$target" ] && grep -q $'\r' "$target"; then
    warn "$target has CRLF line endings; new lines will use LF"
fi

restart_cecd() {
    if $have_user_unit; then
        if systemctl --user restart cecd 2>/dev/null; then
            say "restarted cecd"
        else
            warn "could not restart cecd; run 'systemctl --user restart cecd' in a Desktop Mode terminal"
        fi
    elif $have_system_unit; then
        if systemctl restart cecd 2>/dev/null; then
            say "restarted cecd"
        else
            warn "could not restart the cecd service; run 'systemctl restart cecd' in a Desktop Mode terminal"
        fi
    else
        say "restart cecd yourself when it is installed (systemctl --user restart cecd)"
    fi
}

# --- undo an older install ------------------------------------------------------------

if $restore; then
    [ -f "$target" ] || { say "nothing to restore: $target does not exist"; exit 0; }
    latest=""
    if [ -d "$config_dir" ]; then
        latest=$(find "$config_dir" -maxdepth 1 -name 'config.toml.bak-*' -printf '%T@ %p\n' 2>/dev/null \
            | sort -rn | head -1 | cut -d' ' -f2-)
    fi
    if [ -n "$latest" ] && [ -f "$latest" ]; then
        if $dry_run; then
            say "would restore $latest over $target"
            diff -u "$target" "$latest" || true
            exit 0
        fi
        keep="$target.before-restore-$(date +%Y%m%d%H%M%S)"
        cp -p "$target" "$keep"
        cp -p "$latest" "$target"
        say "restored $latest"
        say "the file it replaced is $keep"
    elif grep -q "Added by Runefin" "$target"; then
        if $dry_run; then
            say "would remove $target (Runefin created it)"
            exit 0
        fi
        rm -f "$target"
        say "removed $target; cecd returns to its defaults"
    else
        die "no backup found and $target is not one this script created; remove the [mappings] keys by hand"
    fi
    restart_cecd
    exit 0
fi

mkdir -p "$config_dir" || die "could not create $config_dir"
[ -w "$config_dir" ] || die "$config_dir is not writable"

# --- the mapping ---------------------------------------------------------------------
#
# Only the Menu buttons are remapped: cecd's own defaults already send Return, Escape and
# the arrow keys, which Runefin's big-screen mode handles. Anything else would change what
# the remote does inside the Steam UI while the app is closed.

keys="contents-menu media-context-sensitive-menu"
values="438 438"   # KEY_CONTEXT_MENU

force_flag=0
$force && force_flag=1

input=/dev/null
[ -f "$target" ] && input="$target"

tmp_merged=$(mktemp) || die "could not create a temporary file"
tmp_report=$(mktemp) || die "could not create a temporary file"
cleanup() { rm -f "$tmp_merged" "$tmp_report"; }
trap cleanup EXIT

set +e
awk -v keys="$keys" -v vals="$values" -v force="$force_flag" '
function trim(s) { sub(/^[ \t\r]+/, "", s); sub(/[ \t\r]+$/, "", s); return s }
BEGIN {
    n = split(keys, K, " "); split(vals, V, " ")
    for (i = 1; i <= n; i++) want[K[i]] = V[i]
}
{ raw[NR] = $0 }
END {
    for (i = 1; i <= NR; i++) {
        if (trim(raw[i]) == "[mappings]") {
            if (sec) { print "duplicate [mappings] table; fix the file before merging" > "/dev/stderr"; exit 3 }
            sec = i
        }
    }
    if (!sec) {
        for (i = 1; i <= NR; i++) print raw[i]
        if (NR > 0) print ""
        print "# Added by Runefin (packaging/flatpak/setup-cecd.sh)"
        print "[mappings]"
        for (j = 1; j <= n; j++) print K[j] " = " V[j]
        exit 0
    }
    end = NR
    for (i = sec + 1; i <= NR; i++) {
        if (substr(trim(raw[i]), 1, 1) == "[") { end = i - 1; break }
    }
    while (end > sec && trim(raw[end]) == "") end--
    for (i = sec + 1; i <= end; i++) {
        line = raw[i]
        if (match(line, /^[ \t]*[A-Za-z0-9_-]+[ \t]*=/)) {
            k = line; sub(/^[ \t]*/, "", k); sub(/[ \t]*=.*/, "", k)
            v = line; sub(/^[^=]*=[ \t]*/, "", v); sub(/[ \t]*#.*/, "", v); v = trim(v)
            if (k in want) {
                seen[k] = 1
                if (v != want[k]) {
                    if (force == 1) {
                        print "replaced " k ": " v " -> " want[k] > "/dev/stderr"
                        raw[i] = k " = " want[k]
                    } else {
                        print "kept existing " k " = " v " (Runefin suggests " want[k] ")" > "/dev/stderr"
                        conflicts++
                    }
                }
            }
        }
    }
    for (i = 1; i <= end; i++) print raw[i]
    for (j = 1; j <= n; j++) {
        if (!(K[j] in seen)) { print K[j] " = " V[j]; missing++ }
    }
    for (i = end + 1; i <= NR; i++) print raw[i]
    if (missing) print "added " missing " mapping(s)" > "/dev/stderr"
    exit (conflicts ? 5 : 0)
}
' "$input" >"$tmp_merged" 2>"$tmp_report"
status=$?
set -e

if [ "$status" -eq 3 ]; then
    cat "$tmp_report" >&2
    die "$target has more than one [mappings] table; merge them by hand first"
fi
if [ "$status" -ne 0 ] && [ "$status" -ne 5 ]; then
    cat "$tmp_report" >&2
    die "could not merge the mapping (awk exit $status)"
fi

[ -s "$tmp_report" ] && cat "$tmp_report" >&2

if [ -f "$target" ] && cmp -s "$tmp_merged" "$target"; then
    say "no changes needed: $target already has the mapping"
    if [ "$status" -eq 5 ]; then
        say "some values differ from Runefin's suggestion; re-run with --force to replace them"
    fi
    exit 0
fi

if $dry_run; then
    say "--- would write $target"
    if [ -f "$target" ]; then
        diff -u "$target" "$tmp_merged" || true
    else
        sed 's/^/  /' "$tmp_merged"
    fi
    exit 0
fi

if [ -f "$target" ]; then
    backup="$target.bak-$(date +%Y%m%d%H%M%S)"
    [ -e "$backup" ] && backup="$backup-$$"
    cp -p "$target" "$backup" || die "could not write a backup at $backup"
    say "backup: $backup"
fi

cp "$tmp_merged" "$target.tmp.$$" && mv "$target.tmp.$$" "$target"
say "wrote: $target"

restart_cecd
say
say "SteamOS Settings > Display / Power > HDMI CEC: remote input must be on, and CEC"
say "enabled on the TV. Runefin takes Return as OK; the D-pad, Back and the player's"
say "transport keys keep cecd's defaults."
