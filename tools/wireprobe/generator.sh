# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 ZeroDDS Contributors
#
# generator.sh — the generator binary a column's probe runs, kept current.
#
# Sourced, not executed.
#
# ## Why this exists
#
# Each own-column probe runs a generator out of `target/debug` and built
# it only `if [ ! -x "$gen" ]` — that is, only when it was missing. A
# binary that exists and is older than the sources it was built from was
# used as it stood, silently.
#
# That cost two measurements in one afternoon and both times it looked
# like something else:
#
# - `java_round_trip` failed every one of its 24 cases with "`--read` is
#   not an option", from a generator built before `--read` existed. The
#   defect looked like a broken harness.
# - A mutation planted in the C generator to check that a new gate would
#   catch it did not reach the gate at all, and the gate reported the
#   mutation as caught-nothing. A test instrument that passes because it
#   measured the previous build is the failure mode a gate exists to
#   prevent.
#
# ## What it does instead
#
# Rebuilds when the binary is older than any source under the crate or
# under `crates/`, which is where the walk and the backends live. Not an
# unconditional `cargo build`: a no-op build measures 4 s here and a
# corpus sweep is thousands of invocations, so it would cost hours to
# answer "nothing changed".
#
# The rebuild is announced on stderr. A probe that quietly rebuilt would
# leave a caller wondering where a pause came from, and a sweep's log is
# where the pause has to be explained.
#
# After the build the binary takes the time the build started. `cargo`
# leaves a binary it finds current untouched, with its old time, and a
# source under `crates/` the generator does not depend on stays newer:
# every call of a 2147-cell sweep announced a rebuild and ran a no-op
# `cargo build`. The build must also have produced the binary the probe
# runs. A `CARGO_TARGET_DIR` that differs between the two sends the build
# elsewhere and leaves the probe on the previous one; that stops here.

# probe_generator <crate name> <binary path> <repository root>
#
# Prints nothing; builds if needed. Exits non-zero if the build fails,
# because a probe with no generator has nothing to measure.
probe_generator() {
    local crate="${1:-}" binary="${2:-}" root="${3:-}"
    local newest
    # `find -newer` rather than a timestamp comparison in the shell:
    # portable across the BSD and GNU userlands this tree runs on, and
    # it stops at the first hit.
    if [ -x "$binary" ]; then
        newest=$(find "$root/crates" "$root/tools/wireprobe" \
            -name '*.rs' -newer "$binary" -print -quit 2>/dev/null)
        [ -z "$newest" ] && return 0
        echo "${crate}: the generator is older than $newest; rebuilding" >&2
    fi
    local stamp json exe built="" ok=1
    stamp=$(mktemp) || return 1
    json=$(mktemp) || { rm -f "$stamp"; return 1; }
    if cargo build -q --message-format=json-render-diagnostics \
        --manifest-path "$root/Cargo.toml" -p "$crate" >"$json"; then
        while IFS= read -r exe; do
            built="$built $exe"
            [ "$exe" -ef "$binary" ] && ok=0
        done < <(sed -n 's/.*"executable":"\([^"]*\)".*/\1/p' "$json")
        if [ "$ok" -eq 0 ]; then
            touch -r "$stamp" "$binary"
        else
            echo "${crate}: cargo built${built:- nothing}, the probe runs $binary" >&2
        fi
    fi
    rm -f "$stamp" "$json"
    return "$ok"
}
