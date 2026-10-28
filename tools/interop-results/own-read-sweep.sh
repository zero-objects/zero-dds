#!/bin/bash
# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 ZeroDDS Contributors
#
# own-read-sweep.sh — the read direction of the four ZeroDDS language
# columns that record their octets through a recorder of their own.
#
#   tools/interop-results/own-read-sweep.sh
#   tools/interop-results/own-read-sweep.sh --columns cpp,java
#   tools/interop-results/own-read-sweep.sh --jobs 8 --cases primitive-mixed
#
# ## What it measures
#
# Each column's own octets, back through the same binding, held against
# **the case's own sample** — not against the Rust column's reading of
# its own bytes. Two decoders wrong in the same way agree with each
# other and disagree with the sample;
# `docs/specs/zerodds-interop-suite-1.0.md` §4, P2.
#
# The comparison is `wireprobe-compare`, the one every other read sweep
# in this tree uses, so a `same` here means what a `same` means in
# `read-corpus.tsv`.
#
# ## Why it is a sweep and not four
#
# `read-corpus-sweep.sh` already takes a write sweep in the six-column
# shape and answers `same`, `different`, `refused` or `unwritten` for
# every row of it, and it finds a system's probe by name —
# `wireprobe-<system>`, which is `wireprobe-zerodds-cpp` for
# `zerodds-cpp`. So the whole of this file is: hand it each column's
# committed write source and put the answer beside it under the name
# `assemble.rs`'s `read_own` looks for, `zerodds-<column>-read.tsv`.
#
# The C and the Python columns are not here. Their two directions come
# out of `tools/wireprobe/three-column-gate --full` in one run, which is
# where `record.sh` gets them, and measuring them a second way would be
# one column with two answers.
#
# ## The generators are built once, up front
#
# Not by each of the parallel probes: `cargo build` takes a lock, and
# `--jobs 8` racing for it is eight serialised builds per case. Each
# probe reads `WIREPROBE_<LANG>_GEN` when it is set and skips its own
# staleness check, which is why they are exported below.
set -uo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$HERE/../.." && pwd)"
SOURCES="${INTEROP_SOURCES:-$ROOT/tests/interop-results/sources}"
TARGET="${WIREPROBE_TARGET:-$ROOT/target/debug}"

columns="cpp,java,csharp,typescript"
jobs="$(nproc 2>/dev/null || sysctl -n hw.ncpu 2>/dev/null || echo 4)"
only=""

while [ $# -gt 0 ]; do
    case "$1" in
        --columns) columns="$2"; shift 2 ;;
        --jobs) jobs="$2"; shift 2 ;;
        --cases) only="$2"; shift 2 ;;
        --help|-h) sed -n '5,44p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'; exit 0 ;;
        *) echo "own-read-sweep.sh: \`$1\` is not an option" >&2; exit 2 ;;
    esac
done

# The crate each column's generator is, so a missing binary is built
# once here and named where it is missing rather than at the bottom of
# 3,760 identical failures.
gen_crate() {
    case "$1" in
        cpp) echo zerodds-wireprobe-cpp-gen ;;
        java) echo zerodds-wireprobe-java-gen ;;
        csharp) echo zerodds-wireprobe-csharp-gen ;;
        typescript) echo zerodds-wireprobe-typescript-gen ;;
    esac
}

status=0
for column in ${columns//,/ }; do
    crate="$(gen_crate "$column")"
    if [ -z "$crate" ]; then
        echo "own-read-sweep.sh: \`$column\` is not one of cpp, java, csharp, typescript" >&2
        exit 2
    fi
    binary="$TARGET/wireprobe-zerodds-$column-gen"
    if [ ! -x "$binary" ]; then
        echo "=== building $crate ===" >&2
        cargo build -q --manifest-path "$ROOT/Cargo.toml" -p "$crate" || { status=1; continue; }
    fi
done

export WIREPROBE_CPP_GEN="${WIREPROBE_CPP_GEN:-$TARGET/wireprobe-zerodds-cpp-gen}"
export WIREPROBE_JAVA_GEN="${WIREPROBE_JAVA_GEN:-$TARGET/wireprobe-zerodds-java-gen}"
export WIREPROBE_CSHARP_GEN="${WIREPROBE_CSHARP_GEN:-$TARGET/wireprobe-zerodds-csharp-gen}"
export WIREPROBE_TYPESCRIPT_GEN="${WIREPROBE_TYPESCRIPT_GEN:-$TARGET/wireprobe-zerodds-typescript-gen}"

if [ ! -x "$TARGET/wireprobe-compare" ]; then
    echo "=== building wireprobe-compare ===" >&2
    cargo build -q --manifest-path "$ROOT/Cargo.toml" \
        -p zerodds-wire-probe --bin wireprobe-compare || status=1
fi

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

for column in ${columns//,/ }; do
    write="$SOURCES/zerodds-$column.tsv"
    out="$SOURCES/zerodds-$column-read.tsv"
    if [ ! -f "$write" ]; then
        # Named, with the recorder that would produce it, and the run
        # goes on. A column whose write direction was never recorded
        # here has nothing to read back, and inventing an empty read
        # file for it would report a measurement nobody made.
        echo "own-read-sweep.sh: $write is not there; \
tools/interop-results/$column-record.sh writes it. \
0 row(s) measured for zerodds-$column" >&2
        status=1
        continue
    fi
    subset="$write"
    if [ -n "$only" ]; then
        # A subset merges rather than overwrites, for
        # `vendor-corpus-sweep.sh`'s reason: a fix that touches fourteen
        # cases must not delete the other 362.
        subset="$work/$column.write.tsv"
        : > "$subset"
        for case in ${only//,/ }; do
            awk -F'\t' -v c="$case" '$1 == c' "$write" >> "$subset"
        done
    fi
    echo "=== the read direction of zerodds-$column ($(wc -l < "$subset") row(s)) ===" >&2
    fresh="$work/$column.read.tsv"
    sweep_code=0
    "$HERE/read-corpus-sweep.sh" --jobs "$jobs" "$subset" "$fresh" || sweep_code=$?
    [ "$sweep_code" = 0 ] || status=1
    # 9 is `toolchain-absent`: this host has not got the column's
    # compiler, the sweep wrote no cell and no `$fresh`, and there is
    # nothing here to merge. `docs/specs/zerodds-verdicts-1.0.md` — "not
    # a measurement, the collection stops". `zerodds-csharp-read.tsv`
    # carried 60 rows of the alternative, one per case, each saying which
    # program was not on the path.
    if [ "$sweep_code" = 9 ]; then
        echo "own-read-sweep.sh: zerodds-$column's toolchain is absent; \
0 row(s) measured and $out is left as it stands" >&2
        continue
    fi
    # A sweep that measured nothing must not be written anywhere. Both
    # branches below destroy the committed source when `$fresh` is
    # empty: `mv` replaces it with the empty file, and the `NR == FNR`
    # merge reads `$out` **as** the first file when the first file has
    # no lines at all, marks every committed cell `seen`, and emits
    # none of them. It happened: all four columns' sources went to zero
    # rows in one run, because `read-corpus-sweep.sh` called GNU
    # `xargs -d` on a host whose xargs has no such option and produced
    # no rows at all. The sweep failing is one fault; the sweep erasing
    # the previous measurement on its way out is a second and a worse
    # one.
    if [ ! -s "$fresh" ]; then
        echo "own-read-sweep.sh: the sweep produced no rows for zerodds-$column; \
$out is left as it stands" >&2
        status=1
        continue
    fi
    if [ -n "$only" ] && [ -f "$out" ]; then
        # The cells this run measured replace their own; every other one
        # is kept as it stands. Keyed on the first four columns, which
        # is the cell — a row whose verdict changed has to replace the
        # old one and not sit beside it.
        awk -F'\t' 'NR == FNR { seen[$1 FS $2 FS $3 FS $4]; next }
                    !(($1 FS $2 FS $3 FS $4) in seen)' "$fresh" "$out" > "$work/$column.kept"
        cat "$fresh" "$work/$column.kept" | LC_ALL=C sort > "$out.new"
        mv "$out.new" "$out"
    else
        mv "$fresh" "$out"
    fi
    echo "own-read-sweep.sh: $(wc -l < "$out") row(s) into $out" >&2
done

exit "$status"
