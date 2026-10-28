#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 ZeroDDS Contributors
#
# scan.sh — what OMG DDS-XTypes 1.3 defines, measured against this tree.
#
# The coverage pages under `docs/spec-coverage/` are positive lists: they name
# what was looked at and say how far it got. They cannot say what was never
# looked at, because nothing in them is derived from the specification's own
# inventory. This scan is the other direction.
#
# The denominator is not a grammar. OMG IDL 4.2 has Annex A — 227 numbered
# productions and a hole check — and XTypes 1.3 has nothing of the kind. Its
# inventory is a set of normative tables and IDL blocks, and `roster.py`
# extracts them one at a time with a completeness check per axis:
#
#   typekind    the 28 TypeKind constants of Annex B, cross-checked between
#               the two listings that declare them
#   typeid      the discriminators of `union TypeIdentifier`, checked against
#               every TI_*/EK_* constant Annex B declares, plus Table 29
#   annotation  the sixteen subclauses of §7.3.1.2.1 (hole check 1..16) and
#               the annotations Table 21 restricts
#   assign      §7.2.4.4 — eight subclauses (hole check 1..8), Tables 14..19,
#               and the 55 rules of `anchors.tsv`, each a literal fragment of
#               the PDF that `roster.py` fails without
#   typeobject  the case labels of MinimalTypeObject and CompleteTypeObject,
#               checked equal to each other
#   typelookup  §7.6.3.3 — four subclauses, two operations, ten IDL types
#   datarep     Table 59's four cells and Table 60's twelve rows
#
# `cases.tsv` holds one probe per line. The probes here are octets rather than
# text: a `TypeIdentifier` or `TypeObject` written out from Annex B by hand,
# or an IDL text whose type is built and encoded. Three things are measured
# per probe, and their meaning is per-op — `probe/src/main.rs` carries the
# table:
#
#   decode   the probe's input is taken in
#   encode   the corresponding output is produced, byte for byte
#   rule     the specification's own answer for this probe is produced —
#            the assignability verdict, the negotiation, the model
#
# and one more that is not about the code:
#
#   corpus   how many committed sources of this tree carry the construct at
#            all, by the probe's `witness` string. It counts files containing
#            a fixed string, which is weaker than "there is a test for this"
#            — a zero is nevertheless a fact: nothing in the tree names the
#            construct, so nothing in the tree can have exercised it.
#
# The results are compared against `expected.tsv`, the recorded state of the
# gap. The point of a recorded baseline is that the gap has a size and the
# size can change in either direction without anyone noticing.
#
# Usage:
#   tools/xtypes-1.3-gap-scan/scan.sh              measure and compare
#   tools/xtypes-1.3-gap-scan/scan.sh --record     rewrite expected.tsv
#   tools/xtypes-1.3-gap-scan/scan.sh --report     print the full measurement
#   tools/xtypes-1.3-gap-scan/scan.sh --explain    print it with the first
#                                                  diagnostic per probe
#   PROBE=/path/to/xtypes-gap-probe tools/xtypes-1.3-gap-scan/scan.sh
#
# Exit status:
#   0  measured; the gap is exactly what expected.tsv records
#   1  a probe that used to get through no longer does — the gap grew
#   2  nothing was measured: no probe binary to run. Never a pass.
#   3  a probe now gets through that expected.tsv records as failing — the
#      gap shrank, and docs/spec-coverage/dds-xtypes-1.3.gaps.md is behind
#      the tree. Rerun with --record and rewrite the affected rows.
#   4  cases.tsv and expected.tsv disagree on which probes exist
set -u

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$HERE/../.." && pwd)"
CASES="$HERE/cases.tsv"
EXPECTED="$HERE/expected.tsv"

MODE=compare
case "${1:-}" in
    --record)  MODE=record ;;
    --report)  MODE=report ;;
    --explain) MODE=explain ;;
    "")        ;;
    *) echo "scan.sh: unknown argument $1" >&2; exit 2 ;;
esac

PROBE="${PROBE:-}"
if [ -z "$PROBE" ]; then
    for candidate in "$HERE/probe/target/release/xtypes-gap-probe" \
                     "$HERE/probe/target/debug/xtypes-gap-probe"; do
        [ -x "$candidate" ] && PROBE="$candidate" && break
    done
fi
if [ -z "$PROBE" ] || [ ! -x "$PROBE" ]; then
    echo "xtypes-1.3-gap-scan: no probe binary to run." >&2
    echo "  cargo build --release --manifest-path $HERE/probe/Cargo.toml" >&2
    echo "  or set PROBE=. Nothing was measured, so nothing is confirmed." >&2
    exit 2
fi
[ -r "$CASES" ] || { echo "xtypes-1.3-gap-scan: $CASES unreadable" >&2; exit 2; }

WORK="$(mktemp -d "${TMPDIR:-/tmp}/xtypes-gap-scan.XXXXXX")"
trap 'rm -rf "$WORK"' EXIT

# ---------------------------------------------------------------- the probes
grep -v '^#' "$CASES" | awk -F'\t' 'NF>=7 {print $1"\t"$5"\t"$7}' > "$WORK/in.tsv"
if [ "$MODE" = explain ]; then
    "$PROBE" --explain < "$WORK/in.tsv" > "$WORK/probe.tsv"
else
    "$PROBE" < "$WORK/in.tsv" > "$WORK/probe.tsv"
fi

# ---------------------------------------------------------------- the corpus
# Every committed source of this tree, minus the release mirror and this
# tool's own directory. `*.md` is deliberately not in it: a coverage page
# that names a construct is not evidence that anything exercises it, and
# this page would otherwise be counting itself.
CORPUS="$WORK/corpus.list"
( cd "$ROOT" && git ls-files -- '*.rs' '*.idl' '*.tsv' '*.xml' '*.py' '*.sh' '*.toml' ) \
    | grep -v '^github/' \
    | grep -v '^tools/xtypes-1.3-gap-scan/' > "$CORPUS" || true
CORPUS_N=$(grep -c '' < "$CORPUS" || true)

grep -v '^#' "$CASES" | awk -F'\t' 'NF>=7 && $6!="-" {print $6}' | sort -u > "$WORK/witness.list"
: > "$WORK/hits.tsv"
if [ "$CORPUS_N" -gt 0 ] && [ -s "$WORK/witness.list" ]; then
    # One `awk` over the corpus with every witness held in memory, rather
    # than one `grep` per witness. `xargs` may split the file list across
    # several `awk` runs, so each run emits `witness<TAB>file` pairs and the
    # counting happens outside: an END block per run would count each split
    # separately and the numbers would be wrong in a way nothing notices.
    ( cd "$ROOT" && tr '\n' '\0' < "$CORPUS" \
        | xargs -0 awk -v wl="$WORK/witness.list" '
            BEGIN {
                while ((getline p < wl) > 0) {
                    pat[++np] = p
                    gsub(/[][(){}.*+?^$|\\\/]/, "\\\\&", p)
                    re = re (re == "" ? "" : "|") p
                }
            }
            FNR == 1 { delete hit }
            # One regular expression over the line before the loop of index
            # calls: the loop then runs on the few lines that can match
            # rather than on all seventy megabytes of them.
            $0 ~ re { for (i = 1; i <= np; i++)
                  if (!(i in hit) && index($0, pat[i])) { hit[i]; print pat[i] "\t" FILENAME } }
          ' 2>/dev/null ) \
        | sort -u | cut -f1 | uniq -c \
        | awk '{ print $2"\t"$1 }' > "$WORK/hits.tsv" || true
fi

# ---------------------------------------------------------------- assemble
MEASURED="$WORK/measured.tsv"
awk -F'\t' -v hits="$WORK/hits.tsv" -v probe="$WORK/probe.tsv" -v cases="$CASES" '
BEGIN {
    while ((getline line < hits) > 0) { split(line, a, "\t"); n[a[1]] = a[2] }
    while ((getline line < probe) > 0) { split(line, b, "\t"); p[b[1]] = b[2]"\t"b[3]"\t"b[4]; note[b[1]] = b[5] }
    while ((getline line < cases) > 0) {
        if (line ~ /^#/ || line == "") continue
        split(line, c, "\t")
        w = c[6]
        fx = (w == "-") ? "-" : (w in n ? n[w] : 0)
        printf "%s\t%s\t%s\n", c[1], (c[1] in p ? p[c[1]] : "?\t?\t?"), fx
    }
}' > "$MEASURED"

n=$(grep -c '' < "$MEASURED" || true)

if [ "$MODE" = record ]; then
    {
        echo "# Recorded by tools/xtypes-1.3-gap-scan/scan.sh --record."
        echo "# ok/err per column, - where the column does not apply to the op;"
        echo "# corpus is a file count, - where no witness string isolates the construct."
        printf '#id\tdecode\tencode\trule\tcorpus\n'
        cat "$MEASURED"
    } > "$EXPECTED"
    echo "xtypes-1.3-gap-scan: recorded $n probe(s) into $EXPECTED"
    exit 0
fi

if [ "$MODE" = report ]; then
    printf '#id\tdecode\tencode\trule\tcorpus\n'
    cat "$MEASURED"
    exit 0
fi

if [ "$MODE" = explain ]; then
    printf '#id\tdecode\tencode\trule\tcorpus\tdiagnostic\n'
    awk -F'\t' -v probe="$WORK/probe.tsv" '
    BEGIN { while ((getline line < probe) > 0) { split(line, b, "\t"); note[b[1]] = b[5] } }
    { printf "%s\t%s\n", $0, (($1 in note) ? note[$1] : "") }' "$MEASURED"
    exit 0
fi

[ -r "$EXPECTED" ] || {
    echo "xtypes-1.3-gap-scan: no $EXPECTED. Run --record once." >&2
    exit 2
}

grep -v '^#' "$EXPECTED" | sort > "$WORK/exp.sorted"
sort "$MEASURED" > "$WORK/got.sorted"

roster_bad=0
cut -f1 "$WORK/exp.sorted" > "$WORK/exp.ids"
cut -f1 "$WORK/got.sorted" > "$WORK/got.ids"
if ! diff -q "$WORK/exp.ids" "$WORK/got.ids" >/dev/null; then
    echo "xtypes-1.3-gap-scan: cases.tsv and expected.tsv name different probes."
    diff "$WORK/exp.ids" "$WORK/got.ids" | sed 's/^/  /'
    roster_bad=1
fi

grew=0
shrank=0
if [ "$roster_bad" -eq 0 ]; then
    while IFS=$'\t' read -r id ed ee er ec; do
        line=$(grep -F "$(printf '%s\t' "$id")" "$WORK/got.sorted" | head -1)
        IFS=$'\t' read -r _ gd ge gr gc <<< "$line"
        for col in decode:$ed:$gd encode:$ee:$ge rule:$er:$gr; do
            name=${col%%:*}; rest=${col#*:}; want=${rest%%:*}; got=${rest#*:}
            if [ "$want" = ok ] && [ "$got" = err ]; then
                echo "  gap grew:   $id $name  ok -> err"
                grew=1
            elif [ "$want" = err ] && [ "$got" = ok ]; then
                echo "  gap shrank: $id $name  err -> ok"
                shrank=1
            elif [ "$want" != "$got" ]; then
                echo "  gap moved:  $id $name  $want -> $got"
                grew=1
            fi
        done
        if [ "$ec" != "-" ] && [ "$gc" != "-" ] && [ "$gc" -lt "$ec" ]; then
            echo "  gap grew:   $id corpus $ec -> $gc"
            grew=1
        fi
    done < "$WORK/exp.sorted"
fi

echo "xtypes-1.3-gap-scan: $n probe(s) over $CORPUS_N committed source(s), via $PROBE"

if [ "$roster_bad" -eq 1 ]; then exit 4; fi
if [ "$grew" -eq 1 ]; then
    echo "The gap grew. docs/spec-coverage/dds-xtypes-1.3.gaps.md names what each probe stands for."
    exit 1
fi
if [ "$shrank" -eq 1 ]; then
    echo "The gap shrank. Rerun with --record and rewrite the affected rows of"
    echo "docs/spec-coverage/dds-xtypes-1.3.gaps.md — a stale negative list is worse than none."
    exit 3
fi
echo "The gap is exactly what expected.tsv records."
exit 0
