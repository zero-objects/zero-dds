#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 ZeroDDS Contributors
#
# scan.sh — what OMG IDL 4.2 allows, measured against this tree.
#
# The coverage pages under `docs/spec-coverage/` are positive lists: they
# name what was looked at and say how far it got. They cannot say what was
# never looked at, because nothing in them is derived from the specification's
# own inventory of constructs. This scan is the other direction. Its
# denominator is Annex A of OMG IDL 4.2 — all 227 grammar productions — plus
# the 24 standardized annotations of clause 8 and the lexical, preprocessing
# and scoping clauses of 7.2, 7.3 and 7.5. Every one of them is fed to the
# front end as IDL, and what comes back is recorded.
#
# `cases.tsv` holds one probe per line: the production or clause it stands
# for, what the specification says a conforming front end must do with it
# (`accept` or `reject`), and the IDL itself. `zerodds-idlc` has six
# subcommands, a construct can be missing from any one of them
# independently, and every one of them is measured:
#
#   check    `zerodds-idlc check`, vendor dialects allowed
#   strict   `zerodds-idlc check --strict`, OMG IDL 4.2 and nothing else
#   lint     `zerodds-idlc lint` — the rules a file that parses can break
#   format   `zerodds-idlc format` — is there a canonical form
#   trip     format, reparse, and compare: does the round trip hold
#   dump     `zerodds-idlc dump-ast` — does the model show the construct
#   deps     `zerodds-idlc print-deps` — does the dependency walk carry it
#   rust     `zerodds-idlc generate --lang rust` — is there an emission
#   c        `zerodds-idlc generate --lang c` — the second backend
#
# `trip` is the column that catches a silent loss. A construct the parser
# knows and the formatter does not disappears when a file is reformatted:
# no error, just a file with something missing from it. Comparing the two
# formatted texts does not catch that — the second format of a text that
# already lost the construct reproduces the loss. So the comparison is over
# the *model*: `dump-ast` and `print-deps` of the source against `dump-ast`
# and `print-deps` of the formatted output, plus idempotence of the format
# itself. `-` where `format` refused the file, because a refusal leaves no
# output to round-trip.
#
# and one more that is not about the tool:
#
#   fixtures how many committed `*.idl` files in this tree contain the
#            construct at all, by the probe's `witness` pattern. It counts
#            occurrences of a text pattern, which is weaker than "there is a
#            test for this" — a zero is nevertheless a fact: no IDL in the
#            tree carries the construct, so nothing in the tree can have
#            exercised it.
#
# The results are compared against `expected.tsv`, which is the recorded
# state of the gap. The point of a recorded baseline is that the gap has a
# size, and the size can change in either direction without anyone noticing.
#
# Usage:
#   tools/idl-4.2-gap-scan/scan.sh              measure and compare
#   tools/idl-4.2-gap-scan/scan.sh --record     rewrite expected.tsv
#   tools/idl-4.2-gap-scan/scan.sh --report     print the full measurement
#   IDLC=/path/to/zerodds-idlc tools/idl-4.2-gap-scan/scan.sh
#
# Exit status:
#   0  measured; the gap is exactly what expected.tsv records
#   1  a probe that used to get through no longer does — the gap grew
#   2  nothing was measured: no `zerodds-idlc` to run. Never a pass.
#   3  a probe now gets through that expected.tsv records as failing — the
#      gap shrank, and `docs/spec-coverage/idl-4.2.gaps.md` is behind the
#      tree. Rerun with --record and rewrite the affected rows.
#   4  cases.tsv and expected.tsv disagree on which probes exist
set -u

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$HERE/../.." && pwd)"
CASES="$HERE/cases.tsv"
EXPECTED="$HERE/expected.tsv"

MODE=compare
case "${1:-}" in
    --record) MODE=record ;;
    --report) MODE=report ;;
    "")       ;;
    *) echo "scan.sh: unknown argument $1" >&2; exit 2 ;;
esac

IDLC="${IDLC:-}"
if [ -z "$IDLC" ]; then
    for candidate in "$ROOT/target/release/zerodds-idlc" "$ROOT/target/debug/zerodds-idlc"; do
        [ -x "$candidate" ] && IDLC="$candidate" && break
    done
fi
if [ -z "$IDLC" ] || [ ! -x "$IDLC" ]; then
    echo "idl-4.2-gap-scan: no zerodds-idlc to run." >&2
    echo "  cargo build -p zerodds-idlc --release, or set IDLC=." >&2
    echo "  Nothing was measured, so nothing is confirmed." >&2
    exit 2
fi
[ -r "$CASES" ] || { echo "idl-4.2-gap-scan: $CASES unreadable" >&2; exit 2; }

WORK="$(mktemp -d "${TMPDIR:-/tmp}/idl-gap-scan.XXXXXX")"
trap 'rm -rf "$WORK"' EXIT
mkdir -p "$WORK/gen"
# The round trip writes both sides to one fixed path, so that the file name
# the diagnostics carry is the same on both.
RT="$WORK/rt"
mkdir -p "$RT"
# `lex-7.3-include` needs something to include — in both directories the
# probes are run from, because the round trip runs the formatted output
# from `$RT` and an include that resolves on one side only would read as a
# round-trip failure.
printf 'struct Included { long m; };\n' > "$WORK/other.idl"
printf 'struct Included { long m; };\n' > "$RT/other.idl"

# Every committed .idl in the tree, minus the release mirror and this corpus.
CORPUS="$WORK/corpus.list"
( cd "$ROOT" && git ls-files -- '*.idl' ) \
    | grep -v '^github/' \
    | grep -v '^tools/idl-4.2-gap-scan/' > "$CORPUS" || true
CORPUS_N=$(grep -c '' < "$CORPUS" || true)

MEASURED="$WORK/measured.tsv"
: > "$MEASURED"

# `axis`, `refs` and `expect` are read to place the columns; what they say
# belongs in docs/spec-coverage/idl-4.2.gaps.md, not in a comparison here.
# The scan compares against the recorded state and nothing else — deciding
# here whether a result is right would put the specification's answer in two
# places, and the two would drift.
# shellcheck disable=SC2034
while IFS=$'\t' read -r id axis refs expect witness idl; do
    case "$id" in \#*|"") continue ;; esac
    printf '%s\n' "${idl//<NL>/$'\n'}" > "$WORK/$id.idl"

    "$IDLC" check          "$WORK/$id.idl" >"$WORK/$id.check"  2>&1 && ck=ok || ck=err
    "$IDLC" check --strict "$WORK/$id.idl" >"$WORK/$id.strict" 2>&1 && st=ok || st=err
    "$IDLC" lint           "$WORK/$id.idl" >"$WORK/$id.lint"   2>&1 && lt=ok || lt=err
    "$IDLC" dump-ast       "$WORK/$id.idl" >"$WORK/$id.dump"   2>/dev/null && du=ok || du=err
    "$IDLC" print-deps     "$WORK/$id.idl" >"$WORK/$id.deps"   2>/dev/null && dp=ok || dp=err
    "$IDLC" format         "$WORK/$id.idl" >"$WORK/$id.fmt"    2>"$WORK/$id.fmt.err" && fm=ok || fm=err
    "$IDLC" generate --lang rust --out "$WORK/gen" "$WORK/$id.idl" >"$WORK/$id.rust" 2>&1 && rs=ok || rs=err
    "$IDLC" generate --lang c    --out "$WORK/gen" "$WORK/$id.idl" >"$WORK/$id.c"    2>&1 && cc=ok || cc=err

    # The round trip. `-` where there is nothing to measure: `format`
    # refused the file, or `check` did — comparing the model of a file that
    # does not lower cleanly compares two error recoveries and says nothing
    # about a loss. The failure this column is for is a `format` that
    # succeeds on a good file and drops something out of it.
    #
    # Both sides are written to the same path, `rt/probe.idl`, because the
    # diagnostics `dump-ast` and `print-deps` print ahead of their output
    # carry the file name; and the diagnostic lines themselves are dropped,
    # because a warning whose column moved is a formatting difference and
    # not a model difference.
    if [ "$fm" != ok ] || [ "$ck" != ok ]; then
        tp="-"
    else
        tp=ok
        cp "$WORK/$id.idl" "$RT/probe.idl"
        "$IDLC" dump-ast   "$RT/probe.idl" 2>/dev/null | grep -v "^$RT/probe.idl:" > "$WORK/$id.dump1"
        "$IDLC" print-deps "$RT/probe.idl" 2>/dev/null | grep -v "^$RT/probe.idl:" > "$WORK/$id.deps1"
        cp "$WORK/$id.fmt" "$RT/probe.idl"
        # (a) the formatted output still parses and lowers
        "$IDLC" check "$RT/probe.idl" >/dev/null 2>&1 || tp=err
        # (b) the model is the one the source had — this is the column's point
        "$IDLC" dump-ast   "$RT/probe.idl" 2>/dev/null | grep -v "^$RT/probe.idl:" > "$WORK/$id.dump2"
        "$IDLC" print-deps "$RT/probe.idl" 2>/dev/null | grep -v "^$RT/probe.idl:" > "$WORK/$id.deps2"
        cmp -s "$WORK/$id.dump1" "$WORK/$id.dump2" || tp=err
        cmp -s "$WORK/$id.deps1" "$WORK/$id.deps2" || tp=err
        # (c) a second format changes nothing
        if "$IDLC" format "$RT/probe.idl" >"$WORK/$id.fmt2" 2>/dev/null; then
            cmp -s "$WORK/$id.fmt" "$WORK/$id.fmt2" || tp=err
        else
            tp=err
        fi
    fi

    if [ "$witness" = "-" ]; then
        fx="-"
    elif [ "$CORPUS_N" -eq 0 ]; then
        fx="-"
    else
        fx=$( cd "$ROOT" && tr '\n' '\0' < "$CORPUS" \
              | xargs -0 grep -lE -- "$witness" 2>/dev/null | grep -c '' || true )
    fi

    printf '%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\n' \
        "$id" "$ck" "$st" "$lt" "$fm" "$tp" "$du" "$dp" "$rs" "$cc" "$fx" >> "$MEASURED"
done < "$CASES"

n=$(grep -c '' < "$MEASURED" || true)

if [ "$MODE" = record ]; then
    {
        echo "# Recorded by tools/idl-4.2-gap-scan/scan.sh --record."
        echo "# ok/err per column; fixtures is a count, - where not measurable."
        echo "#id	check	strict	lint	format	trip	dump	deps	rust	c	fixtures"
        cat "$MEASURED"
    } > "$EXPECTED"
    echo "idl-4.2-gap-scan: recorded $n probe(s) into $EXPECTED"
    exit 0
fi

if [ "$MODE" = report ]; then
    printf '%s\n' "#id	check	strict	lint	format	trip	dump	deps	rust	c	fixtures"
    cat "$MEASURED"
    exit 0
fi

[ -r "$EXPECTED" ] || {
    echo "idl-4.2-gap-scan: no $EXPECTED. Run --record once." >&2
    exit 2
}

grep -v '^#' "$EXPECTED" | sort > "$WORK/exp.sorted"
sort "$MEASURED" > "$WORK/got.sorted"

roster_bad=0
cut -f1 "$WORK/exp.sorted" > "$WORK/exp.ids"
cut -f1 "$WORK/got.sorted" > "$WORK/got.ids"
if ! diff -q "$WORK/exp.ids" "$WORK/got.ids" >/dev/null; then
    echo "idl-4.2-gap-scan: cases.tsv and expected.tsv name different probes."
    diff "$WORK/exp.ids" "$WORK/got.ids" | sed 's/^/  /'
    roster_bad=1
fi

grew=0
shrank=0
if [ "$roster_bad" -eq 0 ]; then
    while IFS=$'\t' read -r id eck est elt efm etp edu edp ers ec efx; do
        line=$(grep -F "$(printf '%s\t' "$id")" "$WORK/got.sorted" | head -1)
        IFS=$'\t' read -r _ gck gst glt gfm gtp gdu gdp grs gc gfx <<< "$line"
        for col in check:$eck:$gck strict:$est:$gst lint:$elt:$glt \
                   format:$efm:$gfm trip:$etp:$gtp dump:$edu:$gdu \
                   deps:$edp:$gdp rust:$ers:$grs c:$ec:$gc; do
            name=${col%%:*}; rest=${col#*:}; want=${rest%%:*}; got=${rest#*:}
            # `-` is a real value in the `trip` column — format refused, so
            # there was nothing to round-trip. A move into or out of it is a
            # change like any other and is reported rather than passed over.
            if [ "$want" = "$got" ]; then
                continue
            fi
            if [ "$got" = err ] || [ "$want" = ok ]; then
                echo "  gap grew:   $id $name  $want -> $got"
                grew=1
            else
                echo "  gap shrank: $id $name  $want -> $got"
                shrank=1
            fi
        done
        if [ "$efx" != "-" ] && [ "$gfx" != "-" ] && [ "$gfx" -lt "$efx" ]; then
            echo "  gap grew:   $id fixtures $efx -> $gfx"
            grew=1
        fi
    done < "$WORK/exp.sorted"
fi

echo "idl-4.2-gap-scan: $n probe(s) over $CORPUS_N committed .idl file(s), via $IDLC"

if [ "$roster_bad" -eq 1 ]; then exit 4; fi
if [ "$grew" -eq 1 ]; then
    echo "The gap grew. docs/spec-coverage/idl-4.2.gaps.md names what each probe stands for."
    exit 1
fi
if [ "$shrank" -eq 1 ]; then
    echo "The gap shrank. Rerun with --record and rewrite the affected rows of"
    echo "docs/spec-coverage/idl-4.2.gaps.md — a stale negative list is worse than none."
    exit 3
fi
echo "The gap is exactly what expected.tsv records."
exit 0
