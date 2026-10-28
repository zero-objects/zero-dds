#!/bin/bash
# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 ZeroDDS Contributors
#
# longdouble-xcdr2 — C7's layout half under encoding version 2.
#
#   tools/inventory-probes/longdouble-xcdr2/run.sh
#
# The CORBA column settled the XCDR1 half: omniORB and TAO both put a
# `long double` at payload offset 8 and give it sixteen octets. No ORB
# has encoding version 2, so the other half needs a DDS product, and
# `wireprobe-<vendor>` cannot be it — `wireprobe-fill` refuses the member
# by name because the *value* a C++ `long double` holds is the
# compiler's x87 extended. The value and the layout are two questions
# and this probe asks only the second one.
#
# A front end that refuses the type is the measurement for that front
# end and is printed as one.
set -uo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$HERE/../../.." && pwd)"
# shellcheck source=../../interop-vendors.sh
. "$ROOT/tools/interop-vendors.sh"
# shellcheck source=../build.sh
. "$ROOT/tools/inventory-probes/build.sh"

work="${INVENTORY_PROBE_WORK:-${TMPDIR:-/tmp}/inventory-probes}/longdouble-xcdr2"
rm -rf "$work"; mkdir -p "$work"

# Cyclone and RTI first: neither has a probe here, and the reason is the
# front end's own, printed rather than assumed.
for vendor in cyclone rti; do
    vendor_probe "$vendor" || { vendor_skip "$vendor" 1; continue; }
    d="$work/$vendor"; mkdir -p "$d"; cp "$HERE/gen.idl" "$d/"
    rc=0
    case "$vendor" in
        cyclone) ( cd "$d" && "$CYCLONE_HOME/bin/idlc" gen.idl ) > "$d/fe.log" 2>&1 || rc=$? ;;
        rti) ( cd "$d" && "$NDDSHOME/bin/rtiddsgen" -language C \
                  -platform "$RTI_TARGET" -replace gen.idl ) > "$d/fe.log" 2>&1 || rc=$? ;;
    esac
    if [ "$rc" -eq 0 ]; then
        printf '%s\tfront end\tACCEPT\t%s\n' "$vendor" \
            "$(grep -m1 -i "long double\|LongDouble" "$d"/gen.h "$d"/*.h 2>/dev/null | cut -c1-120)"
    else
        printf '%s\tfront end\tREJECT\t%s\n' "$vendor" \
            "$(sed 's/\x1b\[[0-9;]*m//g' "$d/fe.log" | grep -m1 -i 'error\|unsupported' | cut -c1-140)"
    fi
done

for vendor in fastdds opendds; do
    vendor_probe "$vendor" || { vendor_skip "$vendor" 2; continue; }
    d="$work/$vendor"; mkdir -p "$d"
    cp "$HERE/gen.idl" "$HERE/probe-$vendor.cpp" "$d/"
    "build_cpp_$vendor" "$d" "$d/gen.idl" "$d/probe-$vendor.cpp" || continue
    ( cd "$d" && ./probe )
done

vendor_report || true
