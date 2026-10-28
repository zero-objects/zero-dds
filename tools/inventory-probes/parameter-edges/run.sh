#!/bin/bash
# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 ZeroDDS Contributors
#
# parameter-edges — the four PL_CDR1 header edges of §I, plus L4, through
# every DDS implementation's own deserialiser.
#
#   tools/inventory-probes/parameter-edges/run.sh
#
# `streams.py` writes each stream in each vendor's own framing and this
# hands it to `wireprobe-<vendor> --in`, which is the read direction the
# pairwise run already uses. Nothing is hand-decoded here: what a vendor
# made of the octets is the JSON its own generated code prints, or the
# refusal its own deserialiser returned.
#
# A refusal is a result and is printed as one. What is never printed is
# nothing: a vendor that is not installed is named by `vendor_report`.
set -uo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$HERE/../../.." && pwd)"
# shellcheck source=../../interop-vendors.sh
. "$ROOT/tools/interop-vendors.sh"

CORPUS="${ZD_CORPUS:-$ROOT/tests/interop-corpus}"
PROBES="${ZD_PROBES:-$ROOT/tools/wireprobe}"
BIN="${ZD_BIN:-$ROOT/target/debug}"
PATH="$BIN:$PROBES:$PATH"
work="${INVENTORY_PROBE_WORK:-${TMPDIR:-/tmp}/inventory-probes}/parameter-edges"
mkdir -p "$work"

printf '%-28s %-9s %s\n' stream vendor 'what its own reader made of it'
for vendor in zerodds cyclone rti fastdds opendds; do
    if [ "$vendor" != zerodds ] && ! vendor_probe "$vendor"; then
        vendor_skip "$vendor" 5; continue
    fi
    python3 "$HERE/streams.py" "$vendor" | while IFS=$'\t' read -r name case hex expected; do
        printf '%s' "$hex" | python3 -c 'import sys;sys.stdout.buffer.write(bytes.fromhex(sys.stdin.read()))' \
            > "$work/$vendor.$name.bin"
        out="$work/$vendor.$name.json"
        if "wireprobe-$vendor" --idl "$CORPUS/$case/type.idl" \
                --type "$(cat "$CORPUS/$case/type")" \
                --in "$work/$vendor.$name.bin" --encoding xcdr1 --endian le \
                --values-out "$out" > "$work/$vendor.$name.log" 2>&1; then
            printf '%-28s %-9s %s\n' "$name" "$vendor" "$(tr -d ' \n' < "$out")"
        else
            printf '%-28s %-9s !%s\n' "$name" "$vendor" \
                "$(sed 's/\x1b\[[0-9;]*m//g' "$work/$vendor.$name.log" \
                   | grep -m1 . | cut -c1-160)"
        fi
    done
done

vendor_report || true
