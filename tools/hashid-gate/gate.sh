#!/bin/bash
# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 ZeroDDS Contributors
#
# hashid-gate — which name `@hashid` hashes, and what the ids around it
# become, across four implementations.
#
#   tools/hashid-gate/gate.sh                  every case, every probe
#   tools/hashid-gate/gate.sh --cases implicit,collision
#
# One row per (case, implementation):
#
#   case  impl  status  bytes
#
# ## Why this is its own gate
#
# `@hashid` is row I22 of `docs/wire-feature-inventory.md` and the one
# divergence with **no dialect**: the four implementations hash four
# different names and §7.3.1.2.1.1 does not settle which. A divergence
# no setting reproduces cannot be recorded as "this flag gives you that
# peer", so it is recorded as sixteen types and what each of the four
# wrote for them. The sixteen isolate one variable each — an argument
# that matches the member name against one that does not, a hashed id
# before a plain one against after, a name long enough to cross the MD5
# block, a deliberate collision, and the worked example from the clause
# itself.
#
# Every case declares `Sample`, so the type name is not a per-case file.
#
# ## What this replaces
#
# `/root/hashid-cases` and `/root/hashid-out` on the Linux test host.
# The cases moved; the outputs did not, because they are a run and not a
# source — this script reproduces them.
set -uo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$HERE/../.." && pwd)"
# shellcheck source=../interop-vendors.sh
. "$ROOT/tools/interop-vendors.sh"

TARGET="${WIREPROBE_TARGET:-$ROOT/target/debug}"
export WIREPROBE_FILL="${WIREPROBE_FILL:-$TARGET/wireprobe-fill}"
export WIREPROBE_CACHE="${WIREPROBE_CACHE:-${TMPDIR:-/tmp}/wireprobe-cache}"
PROBES="$ROOT/tools/wireprobe"
ZERODDS="${WIREPROBE_ZERODDS:-$TARGET/wireprobe-zerodds}"

impls="zerodds,cyclone,rti,fastdds"
encoding="xcdr2"
endian="le"
cases="all"
out=""

while [ $# -gt 0 ]; do
    case "$1" in
        --impls) impls="$2"; shift 2 ;;
        --cases) cases="$2"; shift 2 ;;
        --encoding) encoding="$2"; shift 2 ;;
        --endian) endian="$2"; shift 2 ;;
        --out) out="$2"; shift 2 ;;
        --help|-h) sed -n '5,32p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'; exit 0 ;;
        *) echo "hashid-gate: \`$1\` is not an option" >&2; exit 2 ;;
    esac
done

if [ "$cases" = all ]; then
    cases="$(cd "$HERE/cases" && for d in */; do [ -f "$d/type.idl" ] && basename "$d"; done | sort | paste -sd, -)"
fi
case_count=$(printf '%s\n' "${cases//,/$'\n'}" | grep -c .)

probe_for() {
    case "$1" in
        zerodds) printf '%s' "$ZERODDS" ;;
        *) printf '%s/wireprobe-%s' "$PROBES" "$1" ;;
    esac
}

present=""
for impl in ${impls//,/ }; do
    if vendor_probe "$impl"; then present="$present $impl"
    else vendor_skip "$impl" "$case_count"; fi
done

emit() {
    printf 'case\timpl\tstatus\tbytes\n'
    local case_name impl dir err
    err="$(mktemp)"
    for case_name in ${cases//,/ }; do
        dir="$HERE/cases/$case_name"
        for impl in $present; do
            if hex="$("$(probe_for "$impl")" --idl "$dir/type.idl" --type Sample \
                        --values "$dir/values.json" --encoding "$encoding" \
                        --endian "$endian" 2>"$err")"; then
                printf '%s\t%s\twrote\t%s\n' "$case_name" "$impl" "$hex"
            else
                printf '%s\t%s\trefused\t%s\n' "$case_name" "$impl" \
                    "$(tr '\n\t' '  ' < "$err" | cut -c1-200)"
            fi
        done
    done
    rm -f "$err"
}

if [ -n "$out" ]; then emit > "$out"; else emit; fi
vendor_report || true
