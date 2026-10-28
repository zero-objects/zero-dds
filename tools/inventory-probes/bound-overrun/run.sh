#!/bin/bash
# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 ZeroDDS Contributors
#
# bound-overrun — D6 and G16, the two bounds, through four serialisers.
#
#   tools/inventory-probes/bound-overrun/run.sh
#
# Neither row could be measured by comparing bytes, because a bound that
# is respected costs no octet. What has to be measured is what each
# implementation does when it is *not* respected, and `wireprobe-<vendor>`
# cannot produce that: the harness refuses an over-long value before any
# vendor sees it, which is exactly what G16's "read from the generated
# source rather than exercised" recorded.
#
# The four implementations answer in three different places — the
# generated holder, the container's own API, the serialiser — and the
# probe prints which, because "refused" from a holder and "refused" from
# a serialiser are not the same claim.
set -uo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$HERE/../../.." && pwd)"
# shellcheck source=../../interop-vendors.sh
. "$ROOT/tools/interop-vendors.sh"
# shellcheck source=../build.sh
. "$ROOT/tools/inventory-probes/build.sh"

work="${INVENTORY_PROBE_WORK:-${TMPDIR:-/tmp}/inventory-probes}/bound-overrun"
rm -rf "$work"; mkdir -p "$work"

build_c_cyclone() { # <dir>
    ( cd "$1" && "$CYCLONE_HOME/bin/idlc" gen.idl > idlc.log 2>&1 \
      && gcc -o probe probe-cyclone.c gen.c -I. -I"$CYCLONE_HOME/include" \
             -L"$CYCLONE_HOME/lib" -lddsc -Wl,-rpath,"$CYCLONE_HOME/lib" > cc.log 2>&1 ) \
      || { echo "  the Cyclone probe did not build" >&2
           grep -m4 -i error "$1"/*.log >&2; return 1; }
}

printf 'vendor\tcase\tresult\tpayload or holder\n'
for vendor in cyclone rti fastdds opendds; do
    vendor_probe "$vendor" || { vendor_skip "$vendor" 2; continue; }
    d="$work/$vendor"; mkdir -p "$d"
    cp "$HERE/gen.idl" "$d/"
    case "$vendor" in
        cyclone) cp "$HERE/probe-cyclone.c" "$d/"; build_c_cyclone "$d" || continue ;;
        rti)     cp "$HERE/probe-rti.c" "$d/"; build_c_rti "$d" "$d/gen.idl" "$d/probe-rti.c" || continue ;;
        fastdds) cp "$HERE/probe-fastdds.cpp" "$d/"
                 build_cpp_fastdds "$d" "$d/gen.idl" "$d/probe-fastdds.cpp" || continue ;;
        opendds) cp "$HERE/probe-opendds.cpp" "$d/"
                 build_cpp_opendds "$d" "$d/gen.idl" "$d/probe-opendds.cpp" || continue ;;
    esac
    ( cd "$d" && ./probe )
done

vendor_report || true
