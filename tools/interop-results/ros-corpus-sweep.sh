#!/bin/bash
# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 ZeroDDS Contributors
#
# ros-corpus-sweep.sh — the interop corpus through both ROS 2 type
# supports, one row per (case, type support, byte order).
#
#   tools/interop-results/ros-corpus-sweep.sh out.tsv
#   tools/interop-results/ros-corpus-sweep.sh --typesupports fastrtps out.tsv
#
#   --typesupports <list>  default fastrtps,cyclone
#   --jobs <n>             cases in parallel; default the core count
#   --corpus <dir>         default tests/interop-corpus in this checkout
#
# ## One encoding, and why the column has no second half
#
# There is no encoding version 2 anywhere in the stack: Fast CDR 1.0.24
# has none, and `rosidl_typesupport_fastrtps_c`'s generated
# `_cdr_serialize` writes no DHEADER, no EMHEADER and no sentinel under
# any extensibility. So every row here is XCDR1, and
# `wireprobe-ros` refuses a request for XCDR2 rather than answering it
# with XCDR1 bytes — an agreement or a divergence reported for a format
# nobody ran is worse than a refusal.
#
# `rmw_cyclonedds_cpp` writes the **host's** byte order and takes no
# argument for it, so its big-endian half does not exist on a
# little-endian host. That is a property of the implementation and it is
# recorded as a refusal carrying the probe's own sentence, never as a
# blank.
#
# ## The output
#
#   <case>  <typesupport>  xcdr1  <endian>  <status>  <bytes-or-reason>
#
# the same six columns `vendor-corpus-sweep.sh` writes, so one reader
# takes both.
#
# Two of the probe's exit codes produce no row at all, because
# `docs/specs/zerodds-verdicts-1.0.md` gives them no verdict: 2 (the
# probe was called wrong) and 9 (`toolchain-absent`) are each "not a
# measurement, the collection stops". The sweep then leaves `$out`
# exactly as it found it and exits 9 naming the type support and the
# tool — never a cell saying which program is not on this host's PATH.
set -uo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$HERE/../.." && pwd)"
# shellcheck source=../interop-vendors.sh
. "$ROOT/tools/interop-vendors.sh"

TARGET="${WIREPROBE_TARGET:-$ROOT/target/debug}"
export WIREPROBE_FILL="${WIREPROBE_FILL:-$TARGET/wireprobe-fill}"
export WIREPROBE_DUMP="${WIREPROBE_DUMP:-$TARGET/wireprobe-dump}"
export WIREPROBE_ROS_PACKAGE="${WIREPROBE_ROS_PACKAGE:-$TARGET/wireprobe-ros-package}"
export WIREPROBE_CACHE="${WIREPROBE_CACHE:-${TMPDIR:-/tmp}/wireprobe-cache}"
PROBES="$ROOT/tools/wireprobe"
CORPUS="${INTEROP_CORPUS:-$ROOT/tests/interop-corpus}"

typesupports="fastrtps,cyclone"
jobs="$(nproc 2>/dev/null || sysctl -n hw.ncpu 2>/dev/null || echo 4)"
out=""

while [ $# -gt 0 ]; do
    case "$1" in
        --typesupports) typesupports="$2"; shift 2 ;;
        --jobs) jobs="$2"; shift 2 ;;
        --corpus) CORPUS="$2"; shift 2 ;;
        --help|-h) sed -n '5,43p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'; exit 0 ;;
        -*) echo "ros-corpus-sweep: \`$1\` is not an option" >&2; exit 2 ;;
        *) out="$1"; shift ;;
    esac
done
[ -n "$out" ] || { echo "ros-corpus-sweep: an output path is required" >&2; exit 2; }

mkdir -p "$WIREPROBE_CACHE"

cases=$(cd "$CORPUS" && for d in */; do [ -f "$d/type.idl" ] && basename "$d"; done)
case_count=$(printf '%s\n' "$cases" | grep -c . || true)

if [ ! -d "$ROS_PREFIX" ]; then
    echo "ros: NOT PRESENT (ROS_PREFIX=$ROS_PREFIX) — $((case_count * 4)) rows not measured in this run" >&2
    # And the file is left where it is. It used to be truncated here,
    # which turned "not measured in this run" into "deleted from the
    # record" on every machine without ROS — 1388 rows, exit 0, and a
    # tree that still verifies afterwards because the tree is verified
    # against the sources and both had shrunk together. Measured
    # 2026-08-13, in the same sweep that found the same shape in
    # `vendor-corpus-sweep.sh`.
    if [ -f "$out" ]; then
        echo "ros: $(wc -l < "$out") row(s) kept in $out from a run that had it" >&2
    fi
    exit 0
fi

work="$(mktemp -d)"
# shellcheck source=../wireprobe/diagnostics.sh
. "$ROOT/tools/wireprobe/diagnostics.sh"
WIREPROBE_DIAGNOSTICS="$work/diagnostics"
export WIREPROBE_DIAGNOSTICS
trap 'rm -rf "$work"' EXIT
: > "$work/invocations"

sweep_one() { # <case> — both type supports, both orders, XCDR1
    local c=$1 d="$CORPUS/$1" t ts o status err rows cell reason
    t=$(cat "$d/type" 2>/dev/null || echo Sample)
    err="$work/err.$c.$$"
    rows="$work/rows.$c.$$"
    for ts in $TYPESUPPORTS; do
        # One invocation per case per type support, which is one run of
        # the ROS code generator and one compile. It used to be one per
        # byte order: the order was a `sed` substitution in the template
        # and so part of the binary's identity. It is an argument now,
        # and `--matrix` runs the one build over both orders.
        "$PROBES/wireprobe-ros" --typesupport "$ts" \
            --idl "$d/type.idl" --type "$t" --values "$d/values.json" \
            --matrix > "$rows" 2>"$err"
        status=$?
        printf '%s\t%s\n' "$c" "$ts" >> "$work/invocations"
        case "$status" in
            # Not a measurement, so not a row.
            # `docs/specs/zerodds-verdicts-1.0.md`: 9 is
            # `toolchain-absent` and 2 is the command line, and of both
            # it says "the collection stops". A child of `xargs` cannot
            # stop its parent, so it names itself here and the parent
            # reads the file after the pipeline.
            2|9)
                printf 'ros-%s\t%s\t%s\n' "$ts" "$status" \
                    "$(head -1 "$err" | tr '\t' ' ' | cut -c1-200 | iconv -f utf-8 -t utf-8 -c)" \
                    >> "$ABSENT"
                continue
                ;;
        esac
        if [ $status -ne 0 ] || [ ! -s "$rows" ]; then
            reason="$(head -1 "$err" | tr '\t' ' ' | cut -c1-200 | iconv -f utf-8 -t utf-8 -c)"
            [ -n "$reason" ] || reason="the probe exited $status and produced no rows"
            for o in le be; do
                printf '%s\tros-%s\txcdr1\t%s\trefused\t%s\n' "$c" "$ts" "$o" "$reason"
            done
            continue
        fi
        for o in le be; do
            cell="$(awk -F'\t' -v b="$o" '$1 == "xcdr1" && $2 == b {print $3; exit}' "$rows")"
            case "$cell" in
                "")   printf '%s\tros-%s\txcdr1\t%s\trefused\t%s\n' "$c" "$ts" "$o" \
                          "the probe printed no xcdr1/$o row" ;;
                '!'*) printf '%s\tros-%s\txcdr1\t%s\trefused\t%s\n' "$c" "$ts" "$o" \
                          "${cell#!}" ;;
                *)    printf '%s\tros-%s\txcdr1\t%s\twrote\t%s\n' "$c" "$ts" "$o" "$cell" ;;
            esac
        done
    done
    rm -f "$err" "$rows"
}

TYPESUPPORTS="${typesupports//,/ }"
# Where a cell that answered exit 2 or 9 says so, for the parent to read
# after the parallel run.
ABSENT="$work/toolchain-absent"
: > "$ABSENT"
export TYPESUPPORTS CORPUS PROBES work ABSENT
export -f sweep_one

printf '%s\n' "$cases" \
    | xargs -P "$jobs" -I{} bash -c 'sweep_one "$@"' _ {} \
    | LC_ALL=C sort > "$work/rows.tsv"

# A type support whose toolchain this host has not got measured nothing.
# `$out` is replaced whole here, so installing what the other one
# produced would delete this one's rows from the record — "not measured
# in this run" turning into "deleted", which is the shape the ROS_PREFIX
# check above already names and was measured on 2026-08-13. So the file
# stays and the run ends non-zero.
if [ -s "$ABSENT" ]; then
    echo "ros-corpus-sweep: a toolchain is absent; $out is left as it stands" >&2
    LC_ALL=C sort -u "$ABSENT" | awk -F'\t' '!seen[$1]++ {print "  " $1 ": exit " $2 " :: " $3}' >&2
    if [ -f "$out" ]; then
        echo "ros-corpus-sweep: $(wc -l < "$out") row(s) kept in $out from a run that had it" >&2
    fi
    exit 9
fi

mv "$work/rows.tsv" "$out"
probe_diagnostics_merge "$WIREPROBE_DIAGNOSTICS" \
    "$(dirname "$out")/toolchain-warnings.tsv" ros-fastrtps ros-cyclone
builds=$(wc -l < "$work/invocations" 2>/dev/null || echo 0)
cells=$(wc -l < "$out")
echo "ros-corpus-sweep: $cells rows into $out" >&2
echo "ros-corpus-sweep: $builds probe builds for $cells cells" >&2
