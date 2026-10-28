#!/bin/bash
# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 ZeroDDS Contributors
#
# key-map — N10, `@key` on a `map` member, through four DDS front ends.
#
#   tools/inventory-probes/key-map/run.sh
#
# The shape is `key-union`'s, for the same reason: a construct most front
# ends refuse produces no bytes, and the refusal is the measurement. What
# this group adds over that one is the fourth column — `fastddsgen`
# accepts the file, so for Fast DDS the measurement is not a message but
# the key it generates, and the probe prints that too.
#
# The case cannot live in `tests/interop-corpus/`: the corpus walk
# lowers every case's IDL and this one is refused in lowering, by design.
set -uo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$HERE/../../.." && pwd)"
# shellcheck source=../../interop-vendors.sh
. "$ROOT/tools/interop-vendors.sh"

work="${INVENTORY_PROBE_WORK:-${TMPDIR:-/tmp}/inventory-probes}/key-map"
rm -rf "$work"; mkdir -p "$work"

printf '%-9s %-8s %s\n' vendor verdict 'what its front end said'
for vendor in cyclone rti fastdds opendds; do
    vendor_probe "$vendor" || { vendor_skip "$vendor" 1; continue; }
    d="$work/$vendor"; mkdir -p "$d"; cp "$HERE/types.idl" "$d/"
    rc=0
    case "$vendor" in
        cyclone) ( cd "$d" && "$CYCLONE_HOME/bin/idlc" types.idl ) > "$d/fe.log" 2>&1 || rc=$? ;;
        rti) ( cd "$d" && "$NDDSHOME/bin/rtiddsgen" -language C -platform "$RTI_TARGET" \
                  -replace types.idl ) > "$d/fe.log" 2>&1 || rc=$? ;;
        fastdds) ( cd "$d" && "$FASTDDSGEN" -replace types.idl ) > "$d/fe.log" 2>&1 || rc=$? ;;
        opendds) ( cd "$d" && "$OPENDDS_HOME/bin/opendds_idl" \
                  -I"$OPENDDS_HOME/include" --idl-version 4 types.idl ) > "$d/fe.log" 2>&1 || rc=$? ;;
    esac
    if [ "$rc" -ne 0 ]; then
        printf '%-9s %-8s %s\n' "$vendor" REJECT \
            "$(sed 's/\x1b\[[0-9;]*m//g' "$d/fe.log" \
               | grep -m1 -i 'error\|unsupported\|not supported\|syntax' | tr '\n\t' '  ' | cut -c1-150)"
    else
        # Accepting it is not the end of the measurement. What the key
        # *is* decides whether the clause was enforced somewhere else,
        # and for Fast DDS that is one line of `genCdrAux.ipp`.
        printf '%-9s %-8s %s\n' "$vendor" ACCEPT \
            "$(grep -A 8 'void serialize_key' "$d"/typesCdrAux.ipp 2>/dev/null \
               | grep -m1 'scdr <<' | sed 's/^ *//' | cut -c1-120)"
    fi
done

vendor_report || true
