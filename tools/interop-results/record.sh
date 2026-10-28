#!/bin/bash
# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 ZeroDDS Contributors
#
# record.sh — run the systems, and write down what they produced.
#
#   tools/interop-results/record.sh                 every system this machine can reach
#   tools/interop-results/record.sh --vendors cyclone,opendds
#   tools/interop-results/record.sh --vendors cyclone,rti --cases a,b,c
#   tools/interop-results/record.sh --orbs          the three ORBs only
#   tools/interop-results/record.sh --ros           the two ROS 2 type supports only
#   tools/interop-results/record.sh --read          the read direction of what is recorded
#   tools/interop-results/record.sh --read --cases a,b   those cases' read direction, merged
#   tools/interop-results/record.sh --orbs --ros    both of them; the flags add up
#   tools/interop-results/record.sh --warnings      what the front ends said, and nothing else
#   tools/interop-results/record.sh --own           the own-language columns only
#   tools/interop-results/record.sh --own cpp,java  two of them
#
# ## The own-language columns run inside this and not beside it
#
# `cpp-record.sh`, `csharp-record.sh`, `java-record.sh` and
# `typescript-record.sh` were four copies of one loop that this file did
# not call, so a full record left four of the ledger's columns to
# whoever remembered to run them. Two of the four wrote no
# `sources/*.tsv` at all, which is the shape `csharp-record.sh`'s own
# header called out: "a re-run has nothing to compare against and the
# count in `systems.rs` is a number nobody can reproduce."
#
# They are now one `own-column-record.sh`, called from here with the
# vendor sweeps. The C column is not among them: it comes out of
# `tools/wireprobe/three-column-gate` below, so that this and
# `crates/wire-probe/tests/three_column_gate.rs` cannot report two
# different C columns.
#
# ## Warnings are a measurement of their own
#
# A vendor front end's warning is a statement, and this project has
# already turned two of them into results: Cyclone's "No default
# extensibility provided … this may change to 'appendable'", which is
# the root cause of issues #41 and #42, and `tao_idl`'s "error in
# lookup of symbol", which is how an annotation that was parsed and
# dropped is told apart from one that was honoured. Every sweep below
# throws them away — one keeps the first line of stderr and only on a
# failure, and `wireprobe-batch` filters them out of its blame so that
# a warning about one case cannot exclude it.
#
# `warning-sweep.sh` therefore runs alongside and not inside them. It
# runs each front end and stops: no fill, no compile, no link, no run.
# It produces no bytes and changes none, so it costs one code
# generation per case per system and is not a re-record of anything.
#
# ## Recording a subset
#
# `--cases` takes a comma-separated list, or `@file` for one name per
# line, and records only those. The rows of every other case are kept
# from the existing source file as they are — the sweep merges rather
# than overwrites, which is the whole point of the option: a fix that
# touches fourteen cases must not cost three hundred and forty-one.
#
# It is refused for the sweeps that have not learned it rather than
# silently ignored, because a filter that is accepted and does nothing
# is the same class of defect as an overwrite: the run reports success
# and the tree is not what it says.
#
# ## Recording is not checking
#
# This is the half that needs the vendor toolchains: Cyclone's `idlc`,
# `fastddsgen`, `rtiddsgen`, `opendds_idl`, `hddsgen`, `omniidl`,
# `tao_idl`, the JacORB IDL compiler and a ROS 2 installation. It runs when a vendor
# version changes or a case is added, and it writes one TSV per system
# under `tests/interop-results/sources/`.
#
# The other half — `interop-results --verify`, which is what `cargo
# test` and every CI job run — reads those TSVs and needs none of the
# above. It is seconds over the whole matrix. Keeping the two apart is
# the point of this file existing separately from
# `tools/interop-results/regenerate.sh`, which renders the tree and the
# tables out of what is already recorded.
#
# ## One build per case
#
# Every probe is asked once per case with `--matrix` and answers all
# four combinations from one build. The encoding version and the byte
# order are arguments to the probe binary, not substitutions made before
# its vendor's code generator runs, so they are not part of its
# identity. Each sweep prints its build count beside its cell count;
# `tools/interop-results/tests/record_shape.rs` fails if a setting moves
# back into a template or a cache key.
#
# ## What an absent toolchain does
#
# It is named, with the number of rows it did not produce, and the run
# continues. It is never a silent skip and never a failure merely for
# being absent — but a system that *ran* and produced nothing is a
# failure, and `corba-sweep.sh` exits non-zero on it for exactly the
# reason its own header gives.
#
# That is a toolchain the sweep knew about before it started, from
# `vendor_probe`. A toolchain that is absent when the **probe** runs
# answers exit 9, `toolchain-absent`, and
# `docs/specs/zerodds-verdicts-1.0.md` says of it: "the host has not got
# the column's compiler; **not a measurement**, the collection stops".
# Every sweep called below writes no cell for it, leaves its output file
# exactly as it found it, and exits 9 naming the column and the tool;
# this file's `status` carries that out. Exit 2, the command line, stops
# the same way and for the same reason.
#
# The unit of "nothing is written" is the output file, because that is
# the unit a sweep replaces. `own-column-record.sh` writes one file per
# column, so a missing `dotnet` costs the C# column and leaves C++, Java
# and TypeScript recorded; the vendor, ROS and read sweeps write one file
# for several columns, so an absence there stops that file whole rather
# than deleting the absent column's rows out of it. Either way no
# unmeasured cell appears and no measured cell disappears.
set -uo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$HERE/../.." && pwd)"
SOURCES="$ROOT/tests/interop-results/sources"

vendors="cyclone,rti,fastdds,opendds,hdds"
own="cpp csharp java typescript"
do_vendors=1; do_orbs=1; do_ros=1; do_read=1; do_warnings=1; do_own=1
only=""
jobs="$(nproc 2>/dev/null || sysctl -n hw.ncpu 2>/dev/null || echo 4)"

# The selecting flags add up; they do not cancel each other.
#
# Each of them used to switch the other five off, so naming two of them
# switched **both** off and the run swept nothing and exited 0 —
# `record.sh --orbs --ros --cases <list>` measured not one row and said
# so nowhere. A sweep that silently measures nothing is worse than one
# that fails, because its emptiness reaches the ledger as a result.
#
# So: no selecting flag means all six sweeps, and the first one named
# switches the rest off once, after which each named flag switches its
# own back on.
chose_a_sweep=0
choose() {
    if [ "$chose_a_sweep" = 0 ]; then
        do_vendors=0; do_orbs=0; do_ros=0; do_read=0; do_warnings=0; do_own=0
        chose_a_sweep=1
    fi
}

while [ $# -gt 0 ]; do
    case "$1" in
        --vendors) choose; do_vendors=1; vendors="$2"; shift 2 ;;
        --orbs) choose; do_orbs=1; shift ;;
        --ros) choose; do_ros=1; shift ;;
        --read) choose; do_read=1; shift ;;
        --warnings) choose; do_warnings=1; shift ;;
        # A list is optional; without one it is all four of them.
        --own)
            choose; do_own=1
            shift
            if [ $# -gt 0 ] && [ "${1#-}" = "$1" ]; then own="${1//,/ }"; shift; fi
            ;;
        --jobs) jobs="$2"; shift 2 ;;
        --cases) only="$2"; shift 2 ;;
        --help|-h) sed -n '5,75p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'; exit 0 ;;
        *) echo "record.sh: \`$1\` is not an option" >&2; exit 2 ;;
    esac
done

# Loud rather than ignored: the vendor sweep, the own-language columns
# and the read direction merge a subset; the other two overwrite their
# whole file, and accepting the flag for them would delete every case
# the run did not name.
if [ -n "$only" ] && { [ "$do_orbs" = 1 ] || [ "$do_ros" = 1 ] || \
                       [ "$do_warnings" = 1 ]; }; then
    echo "record.sh: --cases is implemented for the vendor sweep, the own-language" >&2
    echo "           columns and the read direction; pass --vendors <list>, --own" >&2
    echo "           or --read with it" >&2
    exit 2
fi

mkdir -p "$SOURCES"
# The probes the sweeps call — `wireprobe-fill`, `wireprobe-dump` and the
# rest of `zerodds-wire-probe`'s binaries — built from this tree before
# anything is measured. The sweeps take them from `target/debug` and
# never built them: a re-record of thirteen Fast DDS cases ran a
# `wireprobe-fill` two hours older than the fix it was re-recording for,
# and recorded the old answer. `WIREPROBE_TARGET` names another
# directory and skips this, for a caller that built on purpose.
if [ -z "${WIREPROBE_TARGET:-}" ]; then
    cargo build -q -p zerodds-wire-probe --bins \
        --manifest-path "$ROOT/Cargo.toml" --target-dir "$ROOT/target" || {
        echo "record.sh: the probes do not build from this tree; nothing was measured" >&2
        exit 1
    }
fi
# A warm cache across the whole run, so a case swept in the write
# direction and then in the read direction does not rebuild what it
# already built. Independent of the `--matrix` shape: one build per case
# holds with this unset.
export WIREPROBE_CACHE="${WIREPROBE_CACHE:-$ROOT/target/wireprobe-cache}"
mkdir -p "$WIREPROBE_CACHE"

status=0

if [ "$do_vendors" = 1 ]; then
    echo "=== the DDS vendors: $vendors ===" >&2
    "$HERE/vendor-corpus-sweep.sh" --vendors "$vendors" --jobs "$jobs" \
        ${only:+--cases "$only"} \
        "$SOURCES/vendor-corpus.tsv" || status=1
fi
if [ "$do_orbs" = 1 ]; then
    echo "=== the three ORBs ===" >&2
    "$ROOT/tools/corba-oracle/corba-sweep.sh" "$SOURCES/corba-corpus.tsv" || status=1
fi
if [ "$do_ros" = 1 ]; then
    echo "=== the two ROS 2 type supports ===" >&2
    "$HERE/ros-corpus-sweep.sh" --jobs "$jobs" "$SOURCES/ros-corpus.tsv" || status=1
fi

if [ "$do_own" = 1 ]; then
    echo "=== the own-language columns: $own ===" >&2
    # shellcheck disable=SC2086
    "$HERE/own-column-record.sh" ${only:+--cases "$only"} $own || status=1
fi

if [ "$do_warnings" = 1 ]; then
    echo "=== what the front ends said ===" >&2
    "$HERE/warning-sweep.sh" --jobs "$jobs" "$SOURCES/warnings.tsv" || status=1
fi
if [ "$do_read" = 1 ]; then
    echo "=== the read direction ===" >&2
    # Every recorded payload, back through the system that wrote it, and
    # what comes out held against the case's own sample. The ORB source
    # has five columns rather than six — it predates the shape the other
    # sweeps write — so it is put into that shape here rather than the
    # read sweep learning two formats.
    work="$(mktemp -d)"
    trap 'rm -rf "$work"' EXIT
    : > "$work/write-all.tsv"
    for file in vendor-corpus ros-corpus; do
        [ -f "$SOURCES/$file.tsv" ] && cat "$SOURCES/$file.tsv" >> "$work/write-all.tsv"
    done
    if [ -f "$SOURCES/corba-corpus.tsv" ]; then
        # The shared refusal markers are the whole verdict vocabulary,
        # not only NOFORM: `sources.rs` fans each of them out to the
        # three ORBs, and none of them is a system a read could ask.
        awk -F'\t' '$2 != "zerodds" && $2 !~ /^(NOFORM|MAPSDIFFERENTLY|HARNESSGAP)$/ \
            && $5 != "" && $5 !~ /^ERR/ \
            {print $1"\t"$2"\txcdr1\t"$3"\twrote\t"$5}' \
            "$SOURCES/corba-corpus.tsv" >> "$work/write-all.tsv"
    fi
    read_out="$SOURCES/read-corpus.tsv"
    if [ -n "$only" ]; then
        # A subset merges rather than overwrites, for the vendor sweep's
        # reason: re-reading seven cases must not delete the other 411.
        subset="$work/write-subset.tsv"
        : > "$subset"
        for case in ${only//,/ }; do
            awk -F'\t' -v c="$case" '$1 == c' "$work/write-all.tsv" >> "$subset"
        done
        if [ ! -s "$subset" ]; then
            echo "record.sh: none of the cases in --cases has a recorded write row; \
$read_out is left as it stands" >&2
            status=1
        else
            fresh="$work/read-fresh.tsv"
            "$HERE/read-corpus-sweep.sh" --jobs "$jobs" "$subset" "$fresh" || status=1
            # A sweep that measured nothing must not be written anywhere:
            # the merge below reads `$read_out` **as** its first file when
            # the first file has no lines, marks every committed cell seen
            # and emits none of them. See `own-read-sweep.sh` for the run
            # where that happened.
            if [ ! -s "$fresh" ]; then
                echo "record.sh: the read sweep produced no rows; \
$read_out is left as it stands" >&2
                status=1
            else
                # Keyed on the first four columns, which is the cell — a
                # row whose verdict changed replaces the old one rather
                # than sitting beside it.
                awk -F'\t' 'NR == FNR { seen[$1 FS $2 FS $3 FS $4]; next }
                            !(($1 FS $2 FS $3 FS $4) in seen)' \
                    "$fresh" "$read_out" > "$work/read.kept"
                cat "$fresh" "$work/read.kept" | LC_ALL=C sort > "$read_out.new"
                mv "$read_out.new" "$read_out"
                echo "record.sh: $(wc -l < "$fresh") row(s) merged, \
$(wc -l < "$read_out") row(s) in $read_out" >&2
            fi
        fi
    else
        "$HERE/read-corpus-sweep.sh" --jobs "$jobs" \
            "$work/write-all.tsv" "$read_out" || status=1
    fi
fi

# The C and Python columns come out of the one gate that measures them,
# so that this and `crates/wire-probe/tests/three_column_gate.rs` cannot
# report two different C columns.
# Missing is one reason to run it; stale is the other, and stale is the
# one that used to go unnoticed. The JSON is keyed by case, so a corpus
# that has grown since the run leaves cases with no cell in it, and each
# of them lands in the tree as `not measured` — the right word reached
# from a file rather than from a run. Six cases arrived that way.
gate_needed=1
if [ -f "$ROOT/target/three-column-c-full.json" ]; then
    gate_covers=$(grep -o '"[^"]*": *{' "$ROOT/target/three-column-c-full.json" | wc -l)
    corpus_has=$(find "$ROOT/tests/interop-corpus" -mindepth 2 -maxdepth 2 -name type.idl | wc -l)
    if [ "$gate_covers" -ge "$corpus_has" ]; then
        gate_needed=0
    else
        echo "three-column gate: $ROOT/target/three-column-c-full.json covers \
$gate_covers case(s) and the corpus has $corpus_has; re-running it" >&2
    fi
fi
if [ "$gate_needed" -eq 1 ]; then
    echo "=== the three-column gate, full tier ===" >&2
    "$ROOT/tools/wireprobe/three-column-gate" --full || status=1
fi

"$HERE/regenerate.sh" || status=1
exit "$status"
