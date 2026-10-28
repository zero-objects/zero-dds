#!/bin/bash
# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 ZeroDDS Contributors
#
# read-corpus-sweep.sh — the read direction of the same matrix: every
# system reads back the payload it wrote, and what comes out is held
# against the case's own sample.
#
#   tools/interop-results/read-corpus-sweep.sh write.tsv out.tsv
#
#   --jobs <n>      cases in parallel; default the core count
#   --corpus <dir>  default tests/interop-corpus in this checkout
#
# ## Its own bytes, against the case's own sample
#
# Not another column's bytes, and not another column's reading of them.
# A round trip through one implementation passes a decoder that is wrong
# in the same way its own encoder is wrong; comparing against the sample
# is what catches that. Two decoders wrong in the same way agree with
# each other and disagree with the sample.
#
# ## The output
#
#   <case>  <system>  <version>  <endian>  <verdict>  <detail>
#
# `verdict` is one word of `docs/specs/zerodds-verdicts-1.0.md`, and it
# is the probe's **exit code** that says which — never a guess made from
# its message. `same`, `different` and `unwritten` are this sweep's own;
# the rest come out of the exit table:
#
#   2    no cell            the probe was called wrong — not a
#                           measurement; the sweep stops
#   3    no-form            the target has no form for the construct
#   6    harness-gap        a tool of *ours* has not written it
#   7    generator-defect   its front end took the case and its own
#                           generated code does not build
#   8    mapping-divergence a documented mapping rule makes it differ
#   9    no cell            `toolchain-absent` — this host has not got
#                           the column's compiler. Not a measurement;
#                           the sweep stops and `$out` is left alone
#   any other non-zero      refused, with the probe's first line
#
# 2 and 9 had no arm here for as long as this file existed, and the
# table above did not name them either. They fell into `*) refused`, so
# a host without a column's compiler wrote one `refused` cell per case
# saying so — 60 of them in `zerodds-csharp-read.tsv`, each reading
# ``wireprobe-zerodds-csharp: no `dotnet` on PATH``. A code the spec
# carries and the comment omits is where that starts.
#
# 124 is `timed-out` in that table and is **not** here: this sweep runs
# no clock, so nothing it calls can exit 124 for that reason, and an arm
# for a code nothing produces would be a verdict with no measurement
# behind it.
#
# Every one of them used to be `refused`, which says the toolchain was
# asked and said no. `identifier-target-keyword-typename` is the
# measurement that made the distinction matter: `idlc` accepts
# `struct static`, emits a `gen.h` that does not compile, and the sweep
# recorded "Cyclone refused" for a case Cyclone never got to see.
#
# The same reader takes every source in `tests/interop-results/sources/`,
# so `own-read-sweep.sh` answers in the same vocabulary.
set -uo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$HERE/../.." && pwd)"
# shellcheck source=../interop-vendors.sh
. "$ROOT/tools/interop-vendors.sh"

TARGET="${WIREPROBE_TARGET:-$ROOT/target/debug}"
export WIREPROBE_FILL="${WIREPROBE_FILL:-$TARGET/wireprobe-fill}"
export WIREPROBE_DUMP="${WIREPROBE_DUMP:-$TARGET/wireprobe-dump}"
export WIREPROBE_ROS_PACKAGE="${WIREPROBE_ROS_PACKAGE:-$TARGET/wireprobe-ros-package}"
export WIREPROBE_CORBA_IDL="${WIREPROBE_CORBA_IDL:-$TARGET/wireprobe-corba-idl}"
export WIREPROBE_CACHE="${WIREPROBE_CACHE:-${TMPDIR:-/tmp}/wireprobe-cache}"
PROBES="$ROOT/tools/wireprobe"
CORPUS="${INTEROP_CORPUS:-$ROOT/tests/interop-corpus}"
COMPARE="${WIREPROBE_COMPARE:-$TARGET/wireprobe-compare}"

jobs="$(nproc 2>/dev/null || sysctl -n hw.ncpu 2>/dev/null || echo 4)"
write=""
out=""

while [ $# -gt 0 ]; do
    case "$1" in
        --jobs) jobs="$2"; shift 2 ;;
        --corpus) CORPUS="$2"; shift 2 ;;
        --help|-h) sed -n '5,53p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'; exit 0 ;;
        -*) echo "read-corpus-sweep: \`$1\` is not an option" >&2; exit 2 ;;
        *) if [ -z "$write" ]; then write="$1"; else out="$1"; fi; shift ;;
    esac
done
[ -n "$write" ] && [ -n "$out" ] \
    || { echo "read-corpus-sweep: a write sweep and an output path are required" >&2; exit 2; }

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

# One line of the write sweep is one read: the row says which system
# produced which octets, so nothing here decides what to read back.
read_one() { # <case> <system> <version> <endian> <status> <hex>
    local c=$1 s=$2 v=$3 o=$4 status=$5 hex=$6
    local d="$CORPUS/$c" t probe bin json err
    case "$s" in
        NOFORM|MAPSDIFFERENTLY|HARNESSGAP)
            # A shared ORB refusal marker, not a system — `sources.rs`
            # fans it out to the three ORBs and no octets exist behind
            # it. Skipped with a word, never read: treating the marker
            # as a system ran `wireprobe-MAPSDIFFERENTLY` and wrote the
            # shell's error, host path included, into the record.
            echo "read-corpus-sweep: $c: \`$s\` is a marker, nothing to read" >&2
            return
            ;;
    esac
    if [ "$status" != wrote ]; then
        printf '%s\t%s\t%s\t%s\tunwritten\t%s\n' "$c" "$s" "$v" "$o" "the write direction produced nothing"
        return
    fi
    t=$(cat "$d/type" 2>/dev/null || echo Sample)
    bin="$work/$c.$s.$v.$o.bin"
    # `perl` and not `xxd`: the test host has no `xxd`, and a missing
    # decoder turned every one of 3,197 read rows into "the recorded hex
    # is not hex" — a harness failure wearing eleven systems' names.
    #
    # An **empty** hex string is a zero-octet payload and not a failure.
    # `@final struct S {}` under XCDR1 is exactly that — `empty-struct`
    # in the corpus — and the emptiness check used to refuse all four of
    # its slots as "the recorded hex did not decode to octets" while the
    # write record said it had written nothing, which is what an empty
    # struct writes.
    if ! printf '%s' "$hex" | perl -ne 'print pack("H*", $_)' > "$bin" 2>/dev/null \
       || { [ -n "$hex" ] && [ ! -s "$bin" ]; }; then
        printf '%s\t%s\t%s\t%s\trefused\t%s\n' "$c" "$s" "$v" "$o" \
            "the recorded hex did not decode to octets"
        return
    fi
    case "$s" in
        ros-*) probe="$PROBES/wireprobe-ros --typesupport ${s#ros-}" ;;
        *) probe="$PROBES/wireprobe-$s" ;;
    esac
    err="$work/err.$c.$s.$v.$o"
    # The exit code in a variable of its own: `$?` after the assignment
    # is the probe's, and the classification is structural — the verdict
    # is never read out of the message.
    local code=0
    # shellcheck disable=SC2086
    json=$($probe --idl "$d/type.idl" --type "$t" --in "$bin" \
                  --encoding "$v" --endian "$o" 2>"$err") || code=$?
    if [ "$code" != 0 ] || [ -z "$json" ]; then
        local word
        case "$code" in
            # No cell, in either direction of the spec's table: 2 says
            # this harness called the probe wrong and 9 says the host has
            # not got the column's compiler. Neither is an answer about
            # the case. A child of `xargs` cannot stop its parent, so it
            # names itself here and the parent reads the file after the
            # pipeline.
            2|9)
                printf '%s\t%s\t%s\n' "$s" "$code" \
                    "$(head -1 "$err" | tr '\t' ' ' | cut -c1-200 | iconv -f utf-8 -t utf-8 -c)" \
                    >> "$ABSENT"
                rm -f "$bin" "$err"
                return
                ;;
            3) word=no-form ;;
            6) word=harness-gap ;;
            7) word=generator-defect ;;
            8) word=mapping-divergence ;;
            # Exit 0 with nothing on standard output is this harness's
            # gap and not the toolchain's answer: the probe ran, said it
            # had succeeded, and produced no document.
            0) word=harness-gap ;;
            *) word=refused ;;
        esac
        printf '%s\t%s\t%s\t%s\t%s\t%s\n' "$c" "$s" "$v" "$o" "$word" \
            "$(head -1 "$err" | tr '\t' ' ' | sed "s|$ROOT/||g" | cut -c1-200 | iconv -f utf-8 -t utf-8 -c)"
        rm -f "$bin" "$err"
        return
    fi
    printf '%s' "$json" > "$work/$c.$s.$v.$o.json"
    if "$COMPARE" --idl "$d/type.idl" --type "$t" \
                  --expected "$d/values.json" --found "$work/$c.$s.$v.$o.json" \
                  > "$work/cmp.$c.$s.$v.$o" 2>&1; then
        printf '%s\t%s\t%s\t%s\tsame\t\n' "$c" "$s" "$v" "$o"
    else
        printf '%s\t%s\t%s\t%s\tdifferent\t%s\n' "$c" "$s" "$v" "$o" \
            "$(head -1 "$work/cmp.$c.$s.$v.$o" | tr '\t' ' ' | cut -c1-200 | iconv -f utf-8 -t utf-8 -c)"
    fi
    rm -f "$bin" "$err" "$work/$c.$s.$v.$o.json" "$work/cmp.$c.$s.$v.$o"
}

# Where a cell that answered exit 2 or 9 says so, for the parent to read
# after the parallel run.
ABSENT="$work/toolchain-absent"
: > "$ABSENT"

export CORPUS PROBES COMPARE work ABSENT
export -f read_one

if [ ! -x "$COMPARE" ]; then
    echo "read-corpus-sweep: $COMPARE is not there; nothing was measured" >&2
    # The file stays. Truncating it turned a missing binary into 7766
    # deleted rows and exit 0 — see `vendor-corpus-sweep.sh`'s merge for
    # the same shape and the run that measured it.
    if [ -f "$out" ]; then
        echo "read-corpus-sweep: $(wc -l < "$out") row(s) kept in $out from a run that had it" >&2
    fi
    exit 0
fi

# The row reaches the shell as an **argument** and not as text spliced
# into the script. It used to be `<<< "{}"`, which puts the line inside
# the program `bash -c` parses, so a row whose sixth column carried a
# backtick or a parenthesis was a syntax error and produced no row at
# all. Measured 2026-08-19 over the Java column, whose refusal reads
# `NoLanguageForm { construct: "a declaration named ` + "`common`" + …
# (JLS 8.1: …)" }` — 6 of its 3 760 cells vanished, silently, into
# `bash: -c: line 1: syntax error near unexpected token '('`.
# NUL-separated and `xargs -0`, not `xargs -d '\n'`. `-d` is GNU's
# alone: BSD and macOS xargs reject it outright, the pipeline produces
# **no rows at all**, and `own-read-sweep.sh` then wrote that emptiness
# over four committed sources. `-0` is in both, and a NUL cannot occur
# inside a field of this file.
awk -F'\t' '{printf "%s\t%s\t%s\t%s\t%s\t%s%c", $1, $2, $3, $4, $5, $6, 0}' "$write" \
    | xargs -0 -P "$jobs" -I{} bash -c 'IFS=$'"'"'\t'"'"' read -r a b c d e f <<< "$1"; read_one "$a" "$b" "$c" "$d" "$e" "$f"' _ {} \
    | LC_ALL=C sort > "$work/rows.tsv"

# A column whose compiler this host has not got measured nothing, and
# the cells it did not measure must neither appear as refusals nor
# disappear from the record. `$out` is replaced whole here — the caller
# is what merges, when it merges at all — so installing the rows the
# other columns produced would delete that column's previous rows. The
# file therefore stays exactly as it is and the run ends non-zero,
# naming the column and the tool it wanted.
#
# `docs/specs/zerodds-verdicts-1.0.md`, "Why 9 exists": "A run that
# cannot measure a column should stop and say which compiler is missing,
# not fill the matrix with rows about the host."
if [ -s "$ABSENT" ]; then
    echo "read-corpus-sweep: a column's toolchain is absent; $out is left as it stands" >&2
    LC_ALL=C sort -u "$ABSENT" | awk -F'\t' '!seen[$1]++ {print "  " $1 ": exit " $2 " :: " $3}' >&2
    if [ -f "$out" ]; then
        echo "read-corpus-sweep: $(wc -l < "$out") row(s) kept in $out from a run that had it" >&2
    fi
    exit 9
fi

# A write sweep with rows in it and a read sweep with none is a harness
# failure, not a measurement, and writing it out would replace a
# recorded answer with silence.
if [ -s "$write" ] && [ ! -s "$work/rows.tsv" ]; then
    echo "read-corpus-sweep: $(wc -l < "$write") row(s) in and none out; \
$out is left as it stands" >&2
    exit 6
fi
mv "$work/rows.tsv" "$out"
echo "read-corpus-sweep: $(wc -l < "$out") rows into $out" >&2
