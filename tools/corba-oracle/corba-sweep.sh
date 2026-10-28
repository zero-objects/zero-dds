#!/bin/bash
# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 ZeroDDS Contributors
#
# corba-sweep.sh — the corpus, through three ORBs and through ZeroDDS.
#
#   tools/corba-oracle/corba-sweep.sh sweep.tsv
#
# One row per (case, probe, byte order), write direction. The report is
# a TSV the Rust side never reads, so a disagreement is between two
# measurements and never between a measurement and a claim.
#
# XCDR1 *is* CORBA CDR — CORBA 3.3 part 2 §9.3 — which is why an ORB is
# a reference for the whole shared base and not only for `fixed<P,S>`.
# `--encoding` therefore takes the XCDR1 forms only: XCDR2 is XTypes
# §7.4.2 and `pl-cdr1` is rule (23), and no ORB has either.
#
# TAO writes little endian only (`ACE_ENABLE_SWAP_ON_WRITE` is not set
# in this build) and JacORB big endian only (`CDROutputStream` has no
# `setLittleEndian`), so each is asked for the order it can produce and
# omniORB covers both. That asymmetry is in the report shape, not a
# result: two of the three ORB columns are half-width by construction.
#
# ## What this replaces
#
# `/root/corba-oracle/corba-sweep.sh`, which `cd`-ed into the harness
# directory on the test host and read a corpus copy beside it.
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

out="${1:-corba-sweep.tsv}"
mkdir -p "$WIREPROBE_CACHE"
# The run writes here and merges into `$out` at the end, on the
# (case, system) key.
#
# It used to truncate `$out` and append to it as it went, so an ORB this
# machine does not have took its rows with it: the loop skipped it, the
# file was already empty, and the run exited 0. That is the shape
# `vendor-corpus-sweep.sh`'s merge exists to prevent — measured there on
# 2026-08-13, where `--vendors rti` deleted 4148 rows of three other
# vendors — and it was in four sweeps, not one. A machine with all three
# ORBs sweeps every key and the merge is the identity.
fresh="$(mktemp)"
trap 'rm -f "$fresh"' EXIT

corpus_cases=$(cd "$CORPUS" && for d in */; do [ -f "$d/type.idl" ] && basename "$d"; done | wc -l)
for orb in omniorb tao jacorb; do
    vendor_probe "$orb" || vendor_skip "$orb" "$corpus_cases"
done

err="$(mktemp)"
rows="$(mktemp)"
builds=0

# The complete tool output of every refusal, one file per (orb, case);
# the one-line reason in the record is for the table.
TOOL_LOGS="${TOOL_LOGS:-$ROOT/tests/interop-results/sources/toolchain-logs}"
archive_err() { # <orb> <case>
    [ -s "$err" ] || return 0
    mkdir -p "$TOOL_LOGS/$1"
    tr -d '\r' < "$err" | sed "s|$ROOT/||g" >> "$TOOL_LOGS/$1/$2.log"
}
# The first line that carries the tool's own error beats the first line
# of the file, which is often a probe summary saying only that a tool
# failed.
error_line() { # <file>
    local line
    line="$(grep -m1 -iE '(^|[^a-z])error' "$1" | tr '\t' ' ' | cut -c1-160 | iconv -f utf-8 -t utf-8 -c)"
    [ -n "$line" ] || line="$(head -1 "$1" | tr '\t' ' ' | cut -c1-160 | iconv -f utf-8 -t utf-8 -c)"
    printf '%s' "$line"
}

# What the three ORBs' front ends and C++/Java compilers said, kept.
# shellcheck source=../wireprobe/diagnostics.sh
. "$ROOT/tools/wireprobe/diagnostics.sh"
WIREPROBE_DIAGNOSTICS="$(mktemp -d)"
export WIREPROBE_DIAGNOSTICS

# One invocation per case per ORB, and one build behind it. It used to be
# one per byte order, because the order was spliced into the probe's
# template before the ORB's IDL compiler ran and so was part of the
# binary's identity: two runs of `omniidl`, `tao_idl` or the JacORB
# compiler over identical generated code. `--matrix` asks the one build
# for every row the ORB can answer.
sweep() { # <orb> <probe...> — fills "$rows" with `<version> <endian> <cell>`
    local orb=$1
    shift
    : > "$rows"
    if ! "$@" --matrix > "$rows" 2>"$err"; then
        : > "$rows"
        archive_err "$orb" "$c"
    fi
    builds=$((builds + 1))
    if [ ! -s "$rows" ]; then
        # A probe that produced nothing produced nothing for both
        # orders, and both are written. An empty file left as an empty
        # column is exactly the failure this sweep once shipped: the
        # three ORBs ran, exited 0 and wrote no rows, and the run that
        # consumed them read three columns of agreement.
        local reason
        reason="$(error_line "$err")"
        [ -n "$reason" ] || reason="the probe produced no rows"
        printf 'xcdr1\tle\t!%s\nxcdr1\tbe\t!%s\n' "$reason" "$reason" > "$rows"
    fi
}

cell_of() { # <endian> -> the hex, or an error token
    local want=$1 cell
    cell="$(awk -F'\t' -v b="$want" '$2 == b {print $3; exit}' "$rows")"
    case "$cell" in
        "")   printf 'ERR1:the probe printed no xcdr1/%s row' "$want" ;;
        '!'*) printf 'ERR1:%s' "${cell#!}" ;;
        *)    printf '%s' "$cell" ;;
    esac
}

run() { # <probe> <idl> <type> <values> <endian> -> hex, or an error token
    local probe=$1 idl=$2 type=$3 values=$4 endian=$5 hex status
    hex=$("$probe" --idl "$idl" --type "$type" --values "$values" \
                   --encoding xcdr1 --endian "$endian" 2>"$err")
    status=$?
    if [ $status -ne 0 ]; then
        archive_err zerodds "$c"
        printf 'ERR%d:%s' "$status" "$(error_line "$err")"
    else
        printf '%s' "$hex"
    fi
}

for d in "$CORPUS"/*/; do
    c=$(basename "$d")
    [ -f "$d/type.idl" ] || continue
    t=$(cat "$d/type" 2>/dev/null || echo Sample)
    # A case the CORBA rewrite cannot carry is said once and not four
    # times: the front end never saw it, so no ORB refused it. Which
    # kind of "cannot" it is comes from the exit code and not from the
    # message — `docs/specs/zerodds-verdicts-1.0.md` has the vocabulary
    # and the reason the two must not share a word.
    #
    # `|| status=$?` and not a bare call: this script runs under
    # `set -e` and would otherwise end here, before the marker it is
    # about to write.
    status=0
    "$CORBA_IDL" --idl "$d/type.idl" --type "$t" --out /dev/null >/dev/null 2>"$err" || status=$?
    if [ "$status" -ne 0 ]; then
        case "$status" in
            # CORBA IDL has no such construct.
            3) marker=NOFORM ;;
            # It has one, and the octets differ by a documented mapping
            # rule: `omniidl` and `tao_idl` both take a `long double`
            # and CORBA's C++ mapping makes it x87 extended rather than
            # §9.3.1.3's binary128.
            8) marker=MAPSDIFFERENTLY ;;
            # Ours, not CORBA's.
            6|7) marker=HARNESSGAP ;;
            *) marker=NOFORM ;;
        esac
        printf '%s\t%s\t-\t-\t%s\n' "$c" "$marker" \
            "$(head -1 "$err" | sed 's/^wireprobe-corba-idl: //' | cut -c1-160 | iconv -f utf-8 -t utf-8 -c)" >> "$fresh"
        continue
    fi
    # This project's own column stays one call per order: it is a Rust
    # binary with no code generator behind it, so there is nothing to
    # build twice.
    for e in le be; do
        printf '%s\tzerodds\t%s\twrite\t%s\n' "$c" "$e" \
            "$(run "$ZERODDS" "$d/type.idl" "$t" "$d/values.json" "$e")" >> "$fresh"
    done
    if vendor_probe omniorb; then
        sweep omniorb "$PROBES/wireprobe-omniorb" --idl "$d/type.idl" --type "$t" \
              --values "$d/values.json"
        for e in le be; do
            printf '%s\tomniorb\t%s\twrite\t%s\n' "$c" "$e" "$(cell_of "$e")" >> "$fresh"
        done
    fi
    if vendor_probe tao; then
        sweep tao "$PROBES/wireprobe-tao" --idl "$d/type.idl" --type "$t" \
              --values "$d/values.json"
        printf '%s\ttao\tle\twrite\t%s\n' "$c" "$(cell_of le)" >> "$fresh"
    fi
    if vendor_probe jacorb; then
        sweep jacorb "$PROBES/wireprobe-jacorb" --idl "$d/type.idl" --type "$t" \
              --values "$d/values.json"
        printf '%s\tjacorb\tbe\twrite\t%s\n' "$c" "$(cell_of be)" >> "$fresh"
    fi
done
rm -f "$err" "$rows"

# The rows this run produced replace theirs and no others. Keyed on what
# came out rather than on what was asked for, the same rule and the same
# reason as `vendor-corpus-sweep.sh`.
if [ -f "$out" ]; then
    keys="$(mktemp)"; keep="$(mktemp)"
    awk -F'\t' '{print $1"\t"$2}' "$fresh" | LC_ALL=C sort -u > "$keys"
    awk -F'\t' -v keyfile="$keys" '
        BEGIN { while ((getline line < keyfile) > 0) if (line != "") swept[line] = 1 }
        !swept[$1 "\t" $2]
    ' "$out" > "$keep"
    kept=$(wc -l < "$keep")
    if [ "$kept" -gt 0 ]; then
        echo "corba-sweep: $kept row(s) kept from the previous run" >&2
        # Sorted only when there is something to interleave, so a run
        # that swept every key leaves the file in the order the sweep
        # writes it and a re-record is a diff of the octets alone.
        cat "$keep" "$fresh" | LC_ALL=C sort > "$out.new"
    else
        cp "$fresh" "$out.new"
    fi
    mv "$out.new" "$out"
    rm -f "$keys" "$keep"
else
    cp "$fresh" "$out"
fi

probe_diagnostics_merge "$WIREPROBE_DIAGNOSTICS" \
    "$(dirname "$out")/toolchain-warnings.tsv" omniorb tao jacorb
rm -rf "$WIREPROBE_DIAGNOSTICS"

cells=$(wc -l < "$out")
echo "done: $cells rows" >&2
echo "corba-sweep: $builds probe builds for $cells cells" >&2
# A column that ran and produced nothing is named here and not left to a
# reader of the file. Three of these once exited 0 with no rows at all.
for orb in omniorb tao jacorb; do
    vendor_probe "$orb" || continue
    n=$(awk -F'\t' -v o="$orb" '$2 == o && $5 !~ /^ERR/ && $5 != ""' "$out" | wc -l)
    if [ "$n" -eq 0 ]; then
        echo "corba-sweep: $orb produced no bytes in any cell; that is a harness failure, not a result" >&2
        vendor_report || true
        exit 1
    fi
    echo "  $orb: $n cell(s) carry bytes" >&2
done
vendor_report || true
