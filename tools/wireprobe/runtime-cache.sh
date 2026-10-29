# shellcheck shell=bash
# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 ZeroDDS Contributors

# runtime_cache_ready DIR TEST ARTIFACT...
# Test the actual outputs, not whether an interrupted build left a directory.
runtime_cache_ready() {
    local directory="$1" test="$2" artifact
    shift 2
    [ -d "$directory" ] && [ ! -L "$directory" ] && [ "$#" -gt 0 ] || return 1
    if [ "$test" = -p ]; then
        [ -x "$directory/probe" ] || return 1
        test=-s
    fi
    for artifact in "$@"; do
        case "$test" in
            -x) [ -f "$directory/$artifact" ] && [ -x "$directory/$artifact" ] || return 1 ;;
            -s) [ -f "$directory/$artifact" ] && [ -s "$directory/$artifact" ] || return 1 ;;
            *) return 1 ;;
        esac
    done
}

# runtime_cache_publish STAGE CACHE TEST ARTIFACT...
# Builders work outside the lock. Only publication is serialized, so a
# losing builder never nests its stage inside the winner's directory.
# The caller owns and cleans STAGE when it was not consumed. Invalid old
# entries are quarantined, not recursively deleted. Readers never see a
# partially copied entry, and valid entries are never replaced.
runtime_cache_publish() (
    local stage="$1" cache="$2" lock="$2.publish-lock" attempt=0 rejected
    shift 2
    if ! runtime_cache_ready "$stage" "$@"; then
        echo "runtime cache: incomplete build at $stage" >&2
        exit 6
    fi
    # The lock is a file the shell creates exclusively (`noclobber` opens
    # with O_EXCL), not a directory from mkdir: uutils mkdir before 0.11.0,
    # the mkdir of Ubuntu 26.04, reports success to a concurrent loser, and
    # two publishers held the lock at once.
    while ! (set -o noclobber; : > "$lock") 2>/dev/null; do
        attempt=$((attempt + 1))
        if [ "$attempt" -ge 100 ]; then
            echo "runtime cache: publication lock unavailable at $lock; inspect it before retrying" >&2
            exit 6
        fi
        sleep 0.1
    done
    trap 'rm "$lock"' EXIT
    trap 'exit 6' HUP INT TERM
    if runtime_cache_ready "$cache" "$@"; then
        exit 0
    fi
    if [ -e "$cache" ] || [ -L "$cache" ]; then
        rejected=$(mktemp -d "$cache.rejected.XXXXXX") || exit 6
        mv "$cache" "$rejected/entry" || exit 6
        echo "runtime cache: preserved invalid entry at $rejected/entry" >&2
    fi
    mv "$stage" "$cache" || exit 6
    runtime_cache_ready "$cache" "$@" || exit 6
)
