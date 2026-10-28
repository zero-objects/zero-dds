#!/bin/bash
# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 ZeroDDS Contributors
#
# corba-read.sh — the read direction: every ORB is handed the bytes
# ZeroDDS wrote and asked what it makes of them.
#
#   tools/corba-oracle/corba-read.sh read.tsv
#
# A reader that agrees on the values but not on the bytes is a different
# finding from one that agrees on neither, which is why this is a
# separate run and not a column of `corba-sweep.sh`.
#
# TAO writes host order only and JacORB big endian only, but both *read*
# either — the write-side refusals do not apply here, and that asymmetry
# is the reason the read matrix is wider than the write matrix.
#
# ## What this replaces
#
# `/root/corba-oracle/corba-read.sh`.
set -uo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$HERE/../.." && pwd)"
# shellcheck source=../interop-vendors.sh
. "$ROOT/tools/interop-vendors.sh"

TARGET="${WIREPROBE_TARGET:-$ROOT/target/debug}"
export WIREPROBE_FILL="${WIREPROBE_FILL:-$TARGET/wireprobe-fill}"
export WIREPROBE_DUMP="${WIREPROBE_DUMP:-$TARGET/wireprobe-dump}"
export WIREPROBE_CACHE="${WIREPROBE_CACHE:-${TMPDIR:-/tmp}/wireprobe-cache}"
# Exported, not just assigned: the three ORB probes read
# `WIREPROBE_CORBA_IDL` themselves to turn a corpus type into CORBA IDL,
# and without it in the environment they fall back to a bare name on
# `$PATH` and produce nothing.
export WIREPROBE_CORBA_IDL="${WIREPROBE_CORBA_IDL:-$TARGET/wireprobe-corba-idl}"
CORBA_IDL="$WIREPROBE_CORBA_IDL"
ZERODDS="${WIREPROBE_ZERODDS:-$TARGET/wireprobe-zerodds}"
PROBES="$ROOT/tools/wireprobe"
CORPUS="${INTEROP_CORPUS:-$ROOT/tests/interop-corpus}"

out="${1:-corba-read.tsv}"
mkdir -p "$WIREPROBE_CACHE"
: > "$out"

corpus_cases=$(cd "$CORPUS" && for d in */; do [ -f "$d/type.idl" ] && basename "$d"; done | wc -l)
present=""
for orb in omniorb tao jacorb; do
    if vendor_probe "$orb"; then present="$present $orb"
    else vendor_skip "$orb" $((corpus_cases * 2)); fi
done

work=$(mktemp -d); trap 'rm -rf "$work"' EXIT

for d in "$CORPUS"/*/; do
    c=$(basename "$d")
    [ -f "$d/type.idl" ] || continue
    t=$(cat "$d/type" 2>/dev/null || echo Sample)
    "$CORBA_IDL" --idl "$d/type.idl" --type "$t" --out /dev/null >/dev/null 2>&1 || continue
    for e in le be; do
        "$ZERODDS" --idl "$d/type.idl" --type "$t" --values "$d/values.json" \
            --encoding xcdr1 --endian "$e" --out "$work/z.bin" 2>/dev/null || continue
        for probe in $present; do
            if json=$("$PROBES/wireprobe-$probe" --idl "$d/type.idl" --type "$t" \
                          --in "$work/z.bin" --encoding xcdr1 --endian "$e" 2>"$work/err"); then
                printf '%s\t%s\t%s\tread\t%s\n' "$c" "$probe" "$e" \
                    "$(printf '%s' "$json" | tr -d '\n' | cut -c1-600 | iconv -f utf-8 -t utf-8 -c)" >> "$out"
            else
                # JacORB's launcher writes SLF4J noise to stderr before
                # anything it has to say; dropping it keeps the reason
                # column the ORB's own sentence.
                printf '%s\t%s\t%s\tread\tERR:%s\n' "$c" "$probe" "$e" \
                    "$(grep -v '^SLF4J' "$work/err" | head -1 | tr '\t' ' ' | cut -c1-160 | iconv -f utf-8 -t utf-8 -c)" >> "$out"
            fi
        done
        printf '%s\tzerodds\t%s\tread\t%s\n' "$c" "$e" \
            "$("$ZERODDS" --idl "$d/type.idl" --type "$t" --in "$work/z.bin" \
                  --encoding xcdr1 --endian "$e" 2>/dev/null | tr -d '\n' | cut -c1-600 | iconv -f utf-8 -t utf-8 -c)" >> "$out"
    done
done

echo "done: $(wc -l < "$out") rows" >&2
vendor_report || true
