#!/usr/bin/env bash
# Cheap shape rules that must hold before a commit. Run from anywhere.
#
#   scripts/guardrails.sh             # check
#   scripts/guardrails.sh --update    # raise every over-cap ceiling to today's line count
#
# The file-size list is a ratchet: a listed file may shrink, never grow. `--update` is the
# explicit way to raise a ceiling when a file legitimately grew; split it instead when you can.
set -euo pipefail
cd "$(dirname "$0")/.."

update=0
case "${1:-}" in
    --update) update=1 ;;
    "") ;;
    *)
        echo "usage: $0 [--update]" >&2
        exit 2
        ;;
esac

fail=0
exceptions=scripts/file-size-exceptions.txt

# One Rune rev: the app and the AccessKit patch must name the same commit.
pin=$(grep -o 'rune = { git = "https://github.com/lifk/rune", rev = "[0-9a-f]*"' \
    crates/runefin/Cargo.toml | grep -o '[0-9a-f]\{40\}' | head -1 || true)
patch=$(grep -o 'accesskit_atspi_common = { git = "https://github.com/lifk/rune", rev = "[0-9a-f]*"' \
    Cargo.toml | grep -o '[0-9a-f]\{40\}' | head -1 || true)
if [ -z "$pin" ] || [ "$pin" != "$patch" ]; then
    echo "guardrails: rune pin ($pin) and accesskit_atspi_common patch ($patch) differ" >&2
    fail=1
fi

# No ambient crate-root imports in the app.
if grep -R --include='*.rs' -n 'use crate::\*' crates/runefin/src >/dev/null; then
    echo "guardrails: use crate::* hides what a module depends on:" >&2
    grep -R --include='*.rs' -n 'use crate::\*' crates/runefin/src >&2
    fail=1
fi

# File-size ratchet. Warn above 800, error above 1500 unless the file is listed with a ceiling.
declare -A ceilings=()
if [ -f "$exceptions" ]; then
    while read -r path ceiling; do
        [ -z "$path" ] && continue
        case "$path" in \#*) continue ;; esac
        ceilings["$path"]="$ceiling"
    done < "$exceptions"
fi

files=$(git ls-files 'crates/*/src/*.rs' 'crates/*/src/**/*.rs')
new_exceptions=()
for file in $files; do
    [ -f "$file" ] || continue
    lines=$(wc -l < "$file")
    ceiling=${ceilings[$file]:-}
    if [ -n "$ceiling" ]; then
        if [ "$lines" -gt "$ceiling" ]; then
            echo "guardrails: $file has $lines lines, over its $ceiling ceiling; split it or run --update" >&2
            fail=1
        elif [ "$lines" -lt "$ceiling" ]; then
            echo "guardrails: $file is down to $lines lines; lower its ceiling from $ceiling" >&2
        fi
    elif [ "$lines" -gt 1500 ]; then
        echo "guardrails: $file has $lines lines, over the 1500-line cap; split it or add an exception" >&2
        fail=1
    elif [ "$lines" -gt 800 ]; then
        echo "guardrails: warning: $file has $lines lines (warn at 800)" >&2
    fi
    if [ "$update" = 1 ] && [ "$lines" -gt 1500 ]; then
        new_exceptions+=("$file $lines")
    fi
done

if [ "$update" = 1 ]; then
    {
        echo "# Files over the 1500-line cap, with the ceiling they may not grow past."
        echo "# Lower a ceiling when a file shrinks; scripts/guardrails.sh --update raises one."
        printf '%s\n' "${new_exceptions[@]}" | sort
    } > "$exceptions"
    echo "guardrails: wrote $(($(wc -l < "$exceptions") - 2)) ceilings"
fi

exit "$fail"
