#!/bin/bash
# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 ZeroDDS Contributors
#
# fe.sh — which positions each IDL front end accepts `@try_construct` in.
#
#   tools/try-construct-gate/fe/fe.sh
#
# A behaviour gate needs a type that compiles. This asks the question
# one step earlier: DDS-XTypes 1.3 §7.3.1.2.1.12 lists the positions the
# annotation may appear in, the list is incomplete, and the four front
# ends disagree about the rest. Two refusals against two silences is not
# a reading — §4.3 of `docs/interop-evidence.md` — and this is what
# produced that sentence.
#
# The eight files vary exactly one thing each: on a member, bare with no
# argument, on a sequence element, on an alias, on a union
# discriminator, on both halves of a map, on a `long` that cannot be
# trimmed at all, and on an array element.
#
# One row per (file, front end):
#
#   file  frontend  verdict  detail
#
# `accepted` means the generator produced code, not that it produced
# code that does anything: Fast DDS and OpenDDS accept positions they
# then say nothing about, and that silence is the finding.
set -uo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$HERE/../../.." && pwd)"
# shellcheck source=../../interop-vendors.sh
. "$ROOT/tools/interop-vendors.sh"

work="${TC_FE_WORK:-${TMPDIR:-/tmp}/tc-gate-fe}"
frontends="${*:-cyclone rti fastdds opendds}"

files=$(cd "$HERE" && ls ./*.idl | sed 's|^\./||' | sort)
file_count=$(printf '%s\n' "$files" | grep -c .)

present=""
for fe in $frontends; do
    if vendor_probe "$fe"; then present="$present $fe"
    else vendor_skip "$fe" "$file_count"; fi
done

generate() { # <frontend> <dir> -> 0 accepted, 1 refused; log in $dir/fe.log
    local fe=$1 d=$2
    case "$fe" in
        cyclone) ( cd "$d" && "$CYCLONE_HOME/bin/idlc" gen.idl ) > "$d/fe.log" 2>&1 ;;
        rti)     ( cd "$d" && "$NDDSHOME/bin/rtiddsgen" -language C -platform "$RTI_TARGET" \
                       -replace gen.idl ) > "$d/fe.log" 2>&1 ;;
        fastdds) ( cd "$d" && "$FASTDDSGEN" -replace gen.idl ) > "$d/fe.log" 2>&1 ;;
        opendds) ( cd "$d" && "$TAO_IDL" -I"$OPENDDS_HOME/include" -I"$TAO_ROOT" \
                       -Sa -St --idl-version 4 gen.idl \
                   && "$OPENDDS_HOME/bin/opendds_idl" -I"$DDS_ROOT" --idl-version 4 gen.idl ) \
                   > "$d/fe.log" 2>&1 ;;
        *) return 1 ;;
    esac
}

printf 'file\tfrontend\tverdict\tdetail\n'
for file in $files; do
    for fe in $present; do
        d="$work/${file%.idl}/$fe"
        rm -rf "$d"; mkdir -p "$d"
        cp "$HERE/$file" "$d/gen.idl"
        if generate "$fe" "$d"; then
            printf '%s\t%s\taccepted\t\n' "$file" "$fe"
        else
            # Colour escapes out (two of the four generators colour their
            # diagnostics, and a TSV cell is not a terminal), and the first
            # non-empty line when no line carries an error word: `idlc`
            # refuses some of these by printing nothing but the position.
            reason="$(sed 's/\x1b\[[0-9;]*m//g' "$d/fe.log" \
                      | grep -m1 -i 'error\|unsupported\|not supported')"
            [ -n "$reason" ] || reason="$(sed 's/\x1b\[[0-9;]*m//g' "$d/fe.log" \
                                          | grep -m1 .)"
            printf '%s\t%s\trefused\t%s\n' "$file" "$fe" \
                "$(printf '%s' "$reason" | tr '\n\t' '  ' | cut -c1-160)"
        fi
    done
done

vendor_report || true
