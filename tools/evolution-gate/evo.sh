#!/bin/bash
# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 ZeroDDS Contributors
#
# evo.sh — one evolution case, two IDL files, every ordered pair.
#
# An old type writes and a new type reads, then the new type writes and
# the old type reads. What the reader hands back is the measurement.
# No single-type case can express this: the whole point of `@appendable`
# and `@mutable` is what happens when the two ends disagree about the
# type, and a corpus case has one type at both ends.
#
#   tools/evolution-gate/evo.sh evo-appendable-tail
#   IMPLS="zerodds cyclone" ENCODINGS=xcdr2 tools/evolution-gate/evo.sh evo-mutable-key
#
# A case directory holds
#   old.idl   the older type
#   new.idl   the newer type, same type name
#   type      the scoped type name, identical in both files
#   old.json  a sample of the old type
#   new.json  a sample of the new type
#
# ## What this replaces
#
# `/root/evo-work/evo.sh` on the Linux test host, which produced the 969
# foreign rows `docs/interop-evidence.md` §2.5 counts. That copy read
# its probes out of a checkout under `/root/evo-work/zerodds`; this one
# reads them out of the checkout it is in.
set -uo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$HERE/../.." && pwd)"
# shellcheck source=../interop-vendors.sh
. "$ROOT/tools/interop-vendors.sh"

TARGET="${WIREPROBE_TARGET:-$ROOT/target/debug}"
export WIREPROBE_FILL="${WIREPROBE_FILL:-$TARGET/wireprobe-fill}"
export WIREPROBE_DUMP="${WIREPROBE_DUMP:-$TARGET/wireprobe-dump}"
export WIREPROBE_CACHE="${WIREPROBE_CACHE:-${TMPDIR:-/tmp}/wireprobe-cache}"
PROBES="$ROOT/tools/wireprobe"
ZERODDS="${WIREPROBE_ZERODDS:-$TARGET/wireprobe-zerodds}"

CASE="${1:-}"
[ -n "$CASE" ] || { sed -n '5,25p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'; exit 2; }
DIR="$HERE/cases/$CASE"
[ -f "$DIR/type" ] || { echo "evo: no case \`$CASE\` under $HERE/cases" >&2; exit 2; }
TYPE="$(cat "$DIR/type")"
IMPLS="${IMPLS:-zerodds cyclone rti fastdds}"
ENCODINGS="${ENCODINGS:-xcdr1 xcdr2}"
ENDIANS="${ENDIANS:-le}"

probe_for() {
    case "$1" in
        zerodds) printf '%s' "$ZERODDS" ;;
        *) printf '%s/wireprobe-%s' "$PROBES" "$1" ;;
    esac
}

# Which peer's answers the ZeroDDS probe gives. A vendor probe has one
# shape and refuses a dialect that is not its own, which is what makes a
# dialect row mean something — it says which of this project's settings
# reproduces that peer, not which peer was asked to pretend.
DIALECT="${DIALECT:-cyclone}"
dialect_for() {
    case "$1" in
        zerodds) printf -- '--dialect %s' "$DIALECT" ;;
        *) printf '' ;;
    esac
}

# An implementation that is not installed is named, with the rows it
# costs: two directions x two type orders x encodings x byte orders.
run_impls=""
for impl in $IMPLS; do
    if vendor_probe "$impl"; then
        run_impls="$run_impls $impl"
    else
        n_enc=$(printf '%s\n' $ENCODINGS | grep -c .)
        n_end=$(printf '%s\n' $ENDIANS | grep -c .)
        vendor_skip "$impl" $((2 * n_enc * n_end * (1 + $(printf '%s\n' $IMPLS | grep -c .))))
    fi
done

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

for encoding in $ENCODINGS; do
  for endian in $ENDIANS; do
    for pair in "old:new" "new:old"; do
      from="${pair%%:*}"; to="${pair##*:}"
      for writer in $run_impls; do
        bytes="$work/$writer.$from.$encoding.$endian.bin"
        # The word splitting is the point: `dialect_for` returns either
        # nothing or two arguments, and quoting it would pass an empty
        # string as an option.
        # shellcheck disable=SC2046
        if ! "$(probe_for "$writer")" --idl "$DIR/$from.idl" --type "$TYPE" \
                --values "$DIR/$from.json" --encoding "$encoding" --endian "$endian" \
                $(dialect_for "$writer") --out "$bytes" > /dev/null 2>"$work/err"; then
            printf '%s\t%s\t%s\t%s->%s\t%s\t-\tunwritten\t%s\n' \
                "$CASE" "$encoding" "$endian" "$from" "$to" "$writer" \
                "$(tr '\n' ' ' < "$work/err" | cut -c1-160)"
            rm -f "$bytes"
            continue
        fi
        hex="$(od -An -tx1 -v "$bytes" | tr -d ' \n')"
        printf '%s\t%s\t%s\t%s->%s\t%s\t(bytes)\twrote\t%s\n' \
            "$CASE" "$encoding" "$endian" "$from" "$to" "$writer" "$hex"
        for reader in $run_impls; do
          # shellcheck disable=SC2046
          if ! "$(probe_for "$reader")" --idl "$DIR/$to.idl" --type "$TYPE" \
                  --in "$bytes" --encoding "$encoding" --endian "$endian" \
                  $(dialect_for "$reader") --values-out "$work/read.json" > /dev/null 2>"$work/err"; then
              printf '%s\t%s\t%s\t%s->%s\t%s\t%s\tunread\t%s\n' \
                  "$CASE" "$encoding" "$endian" "$from" "$to" "$writer" "$reader" \
                  "$(tr '\n' ' ' < "$work/err" | cut -c1-160)"
              continue
          fi
          printf '%s\t%s\t%s\t%s->%s\t%s\t%s\tread\t%s\n' \
              "$CASE" "$encoding" "$endian" "$from" "$to" "$writer" "$reader" \
              "$(tr -d '\n' < "$work/read.json")"
        done
      done
    done
  done
done

vendor_report || true
