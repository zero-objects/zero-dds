#!/bin/bash
# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 ZeroDDS Contributors
#
# own-column-record.sh — one own-language column's octets, into the
# ledger. One script for all of them.
#
#   tools/interop-results/own-column-record.sh cpp
#   tools/interop-results/own-column-record.sh csharp --cases primitive-mixed
#   tools/interop-results/own-column-record.sh --all
#
# ## Why one and not four
#
# `cpp-record.sh`, `csharp-record.sh`, `java-record.sh` and
# `typescript-record.sh` were four copies of one loop, and where two of
# them had drifted nothing held them together to notice. What the drift
# cost is recorded rather than asserted:
#
# - `cpp-record.sh` and `java-record.sh` wrote no `sources/*.tsv` at all.
#   `csharp-record.sh`'s own header names the consequence — "a re-run has
#   nothing to compare against and the count in `systems.rs` is a number
#   nobody can reproduce".
# - `cpp-record.sh` and `java-record.sh` sent the probe's stderr to
#   `/dev/null` and then ran the probe a second time to find out why it
#   had failed, so the reason in the log came from a different process
#   than the failure.
# - only `typescript-record.sh` printed the exit code on a skip, which is
#   the one thing that says *which* of the four refusals it was —
#   `docs/specs/zerodds-verdicts-1.0.md`.
# - `cpp-record.sh`'s header called itself "the Java column's octets".
#
# Every column here gets all four behaviours: one run per slot, the
# reason from that run, the exit code beside it, and a six-column TSV
# written as it goes so the ledger's source and the octet tree come out
# of one run and cannot drift apart.
#
# ## What it records and what it does not
#
# The write direction, over the five encodings and both byte orders —
# the same grid every other column in the ledger carries.
# `tools/interop-results/ledger-diff.py` compares against it, which is
# what a vendor or runtime change has to be measured with.
#
# The read direction is a round trip and belongs to
# `tools/wireprobe/wireprobe-pairwise`, which compares a decoded sample
# against the one that was written; a ledger holds octets.
#
# A case the probe refuses is skipped and named, with the probe's own
# first line of stderr and the exit code it answered in. A skip is
# printed rather than swallowed, so a new refusal cannot pass for a case
# that has no octets. `docs/language-limits.md` is the standing table of
# what a column cannot express.
#
# ## Exit 9 is not a refusal and not a row
#
# `docs/specs/zerodds-verdicts-1.0.md` gives 9 as `toolchain-absent` —
# "the host has not got the column's compiler; **not a measurement**, the
# collection stops" — and 2 as the command line, which stops for the same
# reason. Neither writes a cell here. Ten rows of `zerodds-csharp.tsv`
# say what the alternative looks like: `recursive-array`, all five
# encodings and both orders, verdict `refused`, reason ``(9)
# wireprobe-zerodds-csharp: no `dotnet` on PATH`` — a sentence about the
# machine, filed as the column's answer about a case.
#
# So a column that answers 9 stops at that cell, writes nothing, and
# leaves its source file exactly as the last run that had the toolchain
# left it. The run ends with a non-zero status naming the column and the
# tool. The other columns on the command line have files of their own and
# are recorded as usual.
set -uo pipefail

cd "$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"

# Every own-language column that records octets. `c` is not here: it
# comes out of `tools/wireprobe/three-column-gate`, so that this and
# `crates/wire-probe/tests/three_column_gate.rs` cannot report two
# different C columns. `record.sh` says the same where it calls the gate.
ALL="cpp csharp java typescript"

columns=""
only=""
while [ $# -gt 0 ]; do
    case "$1" in
        --all) columns="$ALL"; shift ;;
        --cases) only="$2"; shift 2 ;;
        --help|-h) sed -n '5,69p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'; exit 0 ;;
        -*) echo "own-column-record.sh: \`$1\` is not an option" >&2; exit 2 ;;
        *) columns="$columns $1"; shift ;;
    esac
done
if [ -z "${columns// /}" ]; then
    echo "own-column-record.sh: name a column ($ALL) or --all" >&2
    exit 2
fi

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

status=0
for column in $columns; do
    probe="tools/wireprobe/wireprobe-zerodds-$column"
    if [ ! -x "$probe" ]; then
        # Named, with what it did not produce, and the run carries on:
        # an absent column is never a silent skip.
        echo "zerodds-$column: no $probe; 0 case(s) recorded" >&2
        status=1
        continue
    fi

    # The ledger's source, written by the run that produced the octets.
    # `sources::read_six_column` reads `case, system, encoding, endian,
    # wrote|refused, hex-or-reason`.
    var="ZERODDS_$(echo "$column" | tr '[:lower:]' '[:upper:]')_SOURCE"
    source_tsv="${!var:-tests/interop-results/sources/zerodds-$column.tsv}"
    mkdir -p "$(dirname "$source_tsv")"

    # The run writes here and the file is installed at the end. It used
    # to write into `$source_tsv` itself, which meant the committed
    # source was emptied — `: > "$source_tsv"` — or filtered before the
    # first probe had run once. A column whose toolchain is not on this
    # host therefore destroyed its own record on the way to measuring
    # nothing, and there is no arrangement of the loop below that can put
    # that back.
    fresh="$work/zerodds-$column.tsv"
    : > "$fresh"
    absent=""

    wrote=0; skipped=0
    for d in tests/interop-corpus/*/; do
        case=$(basename "$d")
        if [ -n "$only" ] && ! echo ",$only," | grep -q ",$case,"; then
            continue
        fi
        [ -f "$d/type.idl" ] && [ -f "$d/values.json" ] && [ -f "$d/type" ] || continue
        root=$(cat "$d/type")
        out="tests/interop-results/$case/zerodds-$column"
        made=0
        last_code=0
        last_reason=""
        for encoding in plain-cdr1 pl-cdr1 plain-cdr2 delimited-cdr pl-cdr2; do
            for endian in le be; do
                mkdir -p "$out"
                code=0
                reason=$("$probe" --idl "$d/type.idl" --type "$root" \
                    --values "$d/values.json" --encoding "$encoding" \
                    --endian "$endian" --out "$out/$encoding.$endian.bin" 2>&1) || code=$?
                case "$code" in
                    # Not a measurement, and so not a cell.
                    # `docs/specs/zerodds-verdicts-1.0.md` says of 9 —
                    # `toolchain-absent` — and of 2 that "the collection
                    # stops". This is where ten rows of
                    # `zerodds-csharp.tsv` came from, each reading
                    # ``(9) wireprobe-zerodds-csharp: no `dotnet` on
                    # PATH``: a sentence about the host, in the column's
                    # cell, under the verdict `refused`.
                    #
                    # The tool is missing for every case, not for this
                    # one, so the column stops here rather than asking
                    # 3 759 more times.
                    2|9)
                        rm -f "$out/$encoding.$endian.bin"
                        absent="($code) $(printf '%s' "$reason" | head -1 | tr '\t' ' ')"
                        break 2
                        ;;
                esac
                if [ "$code" = 0 ]; then
                    made=$((made + 1))
                    hex=$(od -An -tx1 -v "$out/$encoding.$endian.bin" | tr -d ' \n')
                    printf '%s\t%s\t%s\t%s\twrote\t%s\n' \
                        "$case" "zerodds-$column" "$encoding" "$endian" "$hex" \
                        >> "$fresh"
                else
                    rm -f "$out/$encoding.$endian.bin"
                    # The probe's own first line, from the run that
                    # failed and not from a second one: a refusal names
                    # its reason or it is not a verdict, and the reason
                    # has to belong to the failure it is filed under.
                    last_code=$code
                    last_reason=$(printf '%s' "$reason" | head -1 | tr '\t' ' ')
                    printf '%s\t%s\t%s\t%s\trefused\t(%s) %s\n' \
                        "$case" "zerodds-$column" "$encoding" "$endian" \
                        "$code" "$last_reason" >> "$fresh"
                fi
            done
        done
        if [ -n "$absent" ]; then
            rmdir "$out" 2>/dev/null
            break
        fi
        if [ "$made" -eq 0 ]; then
            rmdir "$out" 2>/dev/null
            skipped=$((skipped + 1))
            # The exit code with the reason: it is the code and not the
            # message that says which of the four refusals this was.
            printf 'skipped %-46s (%s) %s\n' "$case" "$last_code" "$last_reason"
        else
            wrote=$((wrote + 1))
        fi
    done
    # Nothing is installed for a column whose toolchain this host has not
    # got. The file it would have been written into keeps every row of
    # the run that had the toolchain: a cell nobody measured must not
    # appear as a measurement and must not vanish either.
    #
    # The unit is the column, because the unit is the file. The other
    # columns named on the command line have files of their own and are
    # recorded as usual; the run ends non-zero all the same.
    if [ -n "$absent" ]; then
        echo "zerodds-$column: TOOLCHAIN ABSENT $absent" >&2
        echo "zerodds-$column: 0 row(s) recorded; $source_tsv is left as it stands\
$([ -f "$source_tsv" ] && printf ' (%s row(s))' "$(wc -l < "$source_tsv" | tr -d ' ')")" >&2
        status=9
        continue
    fi

    # Sorted, so two runs over one tree produce one file: the corpus walk
    # is already ordered and this is what keeps a `--cases` run
    # appendable.
    if [ -n "$only" ] && [ -f "$source_tsv" ]; then
        # A `--cases` run replaces its own cases' rows and keeps every
        # other one. Appending without this left the old refusal beside
        # the new bytes — twenty rows for a ten-slot case, and the
        # assembler reading whichever came last.
        awk -F'\t' -v only=",$only," 'index(only, "," $1 ",") == 0' \
            "$source_tsv" > "$work/kept.tsv"
        cat "$work/kept.tsv" "$fresh" | LC_ALL=C sort > "$work/merged.tsv"
        mv "$work/merged.tsv" "$source_tsv"
    else
        LC_ALL=C sort "$fresh" > "$work/merged.tsv"
        mv "$work/merged.tsv" "$source_tsv"
    fi
    echo "zerodds-$column: $wrote case(s) recorded, $skipped skipped"
    echo "zerodds-$column: $(wc -l < "$source_tsv") row(s) into $source_tsv"
done
exit "$status"
