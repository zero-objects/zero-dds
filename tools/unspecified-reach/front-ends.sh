#!/bin/bash
# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 ZeroDDS Contributors
#
# front-ends.sh — which IDL front ends accept a construct DDS-XTypes 1.3
# leaves unspecified, and what each one says when it does not.
#
#   tools/unspecified-reach/front-ends.sh
#   tools/unspecified-reach/front-ends.sh --cases bitset,fixed
#   tools/unspecified-reach/front-ends.sh --front-ends cyclone,rti
#
#   --cases <list|all>       default all; names are the .idl files in cases/
#   --front-ends <list>      default cyclone,rti,opendds,tao,omniorb,fastdds
#   --work <dir>             where each front end is run
#   --out <file>             the report, as TSV; default stdout
#   --require                a front end that is not installed fails the run
#
# One row per (case, front end):
#
#   case  front-end  verdict  diagnostic
#
# `verdict` is `accept` or `reject`. A rejection is a finding and not a
# failure: five of six front ends reject `bitset`, and that is the
# measurement — it is why rows P1, P2 and P3 of
# `docs/wire-feature-inventory.md` have one implementation and can have
# no second one. The diagnostic is kept because the two front ends that
# reject `@try_construct` on an alias name *different* permitted sets,
# which is the substance of `docs/omg-xtypes-issues.md` §B4.
#
# Accepting is not the same as implementing. A front end that parses an
# annotation and drops it writes the unannotated bytes; `bitset-bytes.sh`
# beside this script is what tells the two apart.
set -u

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=../interop-vendors.sh
. "$HERE/../interop-vendors.sh"

CASES=all
FRONT_ENDS=cyclone,rti,opendds,tao,omniorb,fastdds
WORK=""
OUT=""
REQUIRE=0
while [ $# -gt 0 ]; do
    case "$1" in
        --cases)       CASES="$2"; shift 2 ;;
        --front-ends)  FRONT_ENDS="$2"; shift 2 ;;
        --work)        WORK="$2"; shift 2 ;;
        --out)         OUT="$2"; shift 2 ;;
        --require)     REQUIRE=1; shift ;;
        *) echo "front-ends.sh: unknown argument $1" >&2; exit 2 ;;
    esac
done

WORK="${WORK:-$(mktemp -d "${TMPDIR:-/tmp}/unspec-reach.XXXXXX")}"
mkdir -p "$WORK"

if [ "$CASES" = all ]; then
    CASE_LIST=$(cd "$HERE/cases" && ls ./*.idl 2>/dev/null | sed 's#^\./##; s#\.idl$##' | sort)
else
    CASE_LIST=$(printf '%s\n' "${CASES//,/ }")
fi

# The number of rows a front end that is absent does not produce.
case_count=$(printf '%s\n' $CASE_LIST | grep -c .)

emit() { if [ -n "$OUT" ]; then printf '%s\n' "$*" >> "$OUT"; else printf '%s\n' "$*"; fi; }
[ -n "$OUT" ] && : > "$OUT"

# run_front_end <name> <case-dir> -> sets VERDICT and DIAGNOSTIC
run_front_end() {
    local fe="$1" dir="$2" rc
    ( cd "$dir" || exit 3
      case "$fe" in
          cyclone) "$CYCLONE_HOME/bin/idlc" t.idl ;;
          rti)     "$NDDSHOME/bin/rtiddsgen" -language C -replace -d . t.idl ;;
          opendds) "$OPENDDS_HOME/bin/opendds_idl" t.idl ;;
          tao)     "$TAO_IDL" --idl-version 4 -as -g /bin/true t.idl ;;
          omniorb) "$OMNIIDL" -bcxx t.idl ;;
          fastdds) "$FASTDDSGEN" -replace t.idl ;;
      esac ) > "$dir/out.log" 2>&1
    rc=$?
    if [ $rc -eq 0 ]; then VERDICT=accept; else VERDICT=reject; fi
    # The first line that looks like a diagnostic, with ANSI colour and
    # the absolute path stripped — `fastddsgen` colours its errors and
    # every front end prints the path it was handed, and neither belongs
    # in a report that is diffed between machines.
    DIAGNOSTIC=$(grep -im1 'error\|syntax\|not supported\|unsupported\|mismatched\|not applicable\|invalid\|can only be applied' "$dir/out.log" \
        | sed 's/\x1b\[[0-9;]*m//g; s#[^ ]*/t\.idl#t.idl#g' | tr -d '\r' | cut -c1-160)
    [ -z "$DIAGNOSTIC" ] && DIAGNOSTIC='(no diagnostic)'
}

for fe in ${FRONT_ENDS//,/ }; do
    # `tao` and `opendds` share one install; probe the binary each one
    # actually runs rather than the prefix, because a prefix with a
    # half-built TAO passes a directory test and fails the run.
    probe_name=$fe
    case "$fe" in tao) probe_name=tao ;; esac
    if ! vendor_probe "$probe_name"; then
        # Counted, named, and carried to the exit status by
        # vendor_report below. Never dropped from the run in silence.
        vendor_skip "$probe_name" "$case_count"
        continue
    fi
    for case in $CASE_LIST; do
        src="$HERE/cases/$case.idl"
        if [ ! -f "$src" ]; then
            echo "front-ends.sh: no case $case" >&2; exit 2
        fi
        dir="$WORK/$case/$fe"
        mkdir -p "$dir"; cp "$src" "$dir/t.idl"
        run_front_end "$fe" "$dir"
        emit "$(printf '%s\t%s\t%s\t%s' "$case" "$fe" "$VERDICT" "$DIAGNOSTIC")"
    done
done

vendor_report
report_rc=$?
if [ $REQUIRE -eq 1 ] && [ $report_rc -ne 0 ]; then exit 1; fi
exit 0
