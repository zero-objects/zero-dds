# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 ZeroDDS Contributors
#
# Reuse a probe binary that has already been generated and compiled.
#
# Each foreign probe runs its vendor's code generator and then a C or C++
# compiler, which is seconds per invocation — Fast DDS is five. The
# pairwise run of the interop suite's P2 is every ordered pair of four
# implementations over three hundred cases, and every reader has to be
# handed every writer's bytes: sixteen decode runs per case, encoding and
# byte order. Regenerating for each of them spends the whole budget on
# `fastddsgen`.
#
# What actually differs between those sixteen is the payload on stdin.
# The binary does not, so it is built once and kept, keyed by everything
# that goes into it. A key that missed an input would hand back a probe
# built for another case, which is why the key is the *content* of the
# IDL and of the generated statements rather than their paths.
#
# ## The template is an input
#
# Six of the seven drivers hashed the IDL and the generated `fill.inc` /
# `dump.inc` and **not** the template compiled around them, so an edit to
# `rti-template.c`, `omniorb-template.cpp` or any of the others left
# every key unchanged and a re-record handed back binaries built from the
# text before the edit. `wireprobe-ros` already passed both of its
# templates; the other six now do too. Found 2026-08-13, while changing
# three templates at once, which is the run that would have recorded the
# old bytes under the new source and produced exactly the tree
# `tools/interop-results/src/reproducible.rs` exists to refuse.
#
# Off unless `WIREPROBE_CACHE` names a directory, so a single-shot run
# behaves exactly as it did before.

# probe_cache_key <vendor> <direction> <substitution...> -- <file...>
#
# Everything that decides the binary, hashed. Nothing about where it will
# run: two callers with the same inputs must land on the same key.
#
# ## Why a version is in it
#
# A cache entry holds more than the binary now: `diagnostics.sh` stores
# what the toolchain said while building it, so that a hit can report
# what a run that did not compile could not. An entry written before
# that has no such rows, and a hit on one reports nothing — which is
# indistinguishable from a build that had nothing to say, and was
# measured: the first full re-record after the mechanism landed produced
# `0 new row(s)` for the three ORBs and both ROS type supports, because
# every one of their builds was a hit on an entry from before it.
#
# So the mechanism's identity is part of the key. Entries from before it
# are misses, are rebuilt once, and carry their rows from then on. The
# alternative was for somebody to remember to clear the cache, which is
# the same thing without the guarantee.
PROBE_CACHE_VERSION="${PROBE_CACHE_VERSION:-diagnostics-1}"

probe_cache_key() {
    {
        printf '%s\0' "$PROBE_CACHE_VERSION"
        while [ $# -gt 0 ] && [ "$1" != "--" ]; do printf '%s\0' "$1"; shift; done
        shift || true
        for file in "$@"; do cat "$file"; printf '\0'; done
    } | sha256sum | cut -c1-40
}

# probe_cache_dir <key>
#
# Where that binary lives, or the empty string when caching is off.
probe_cache_dir() {
    [ -n "${WIREPROBE_CACHE:-}" ] || { printf ''; return 0; }
    mkdir -p "$WIREPROBE_CACHE"
    printf '%s/%s' "$WIREPROBE_CACHE" "$1"
}

# probe_cache_publish <build dir> <cache dir>
#
# Move a finished build into place. Losing the race is not an error: the
# other builder produced the same binary from the same inputs, which is
# what the key says. `mv -T` refuses to nest one directory inside the
# other, so a lost race leaves the winner's build and discards this one
# rather than burying it.
#
# An entry without its binary is not a winner: a build cut short, or one
# from before the binary and its diagnostics were published together,
# left only `generated/`. Every later build of that key was discarded
# against it, and every run of the case ended in "probe: No such file"
# (309 cases on the Linux test host). Such an entry is moved aside and
# replaced.
probe_cache_publish() {
    if mv -T "$1" "$2" 2>/dev/null; then
        return 0
    fi
    if [ ! -x "$2/probe" ]; then
        local stale
        stale="$(mktemp -d "$(dirname "$2")/stale.XXXXXX")"
        if mv -T "$2" "$stale/entry" 2>/dev/null && mv -T "$1" "$2" 2>/dev/null; then
            rm -rf "$stale"
            return 0
        fi
        rm -rf "$stale"
    fi
    rm -rf "$1"
}
