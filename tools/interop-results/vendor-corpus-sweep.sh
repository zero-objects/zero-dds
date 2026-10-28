#!/bin/bash
# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 ZeroDDS Contributors
#
# vendor-corpus-sweep.sh — the interop corpus, through the DDS vendors'
# own serialisers, one row per (case, vendor, encoding version, byte
# order).
#
#   tools/interop-results/vendor-corpus-sweep.sh out.tsv
#   tools/interop-results/vendor-corpus-sweep.sh --vendors cyclone,fastdds out.tsv
#
#   --vendors <list>  default cyclone,rti,fastdds,opendds,hdds
#   --jobs <n>        cases in parallel; default the core count
#   --corpus <dir>    default tests/interop-corpus in this checkout
#   --cases <list>    only these cases, comma-separated, or @file for one
#                     name per line. The rows of every other case are
#                     kept from the output file as they are, so a
#                     fourteen-case re-record costs fourteen cases and
#                     not three hundred and forty-one.
#
# ## Why a subset merges rather than overwrites
#
# The output is one row per (case, vendor, encoding, order). A partial
# sweep that wrote only what it measured would delete every row it did
# not — the whole corpus minus the subset — and the tree would still
# verify, because it verifies the tree against the sources and both
# would have shrunk together. So a run reads the existing file, drops
# exactly the rows for the swept cases *and the swept vendors*, and puts
# the new rows in their place. A case named here that the corpus does
# not have is an error and not an empty sweep: a typo must not read as
# "nothing to record".
#
# **A vendor subset is a subset the same way.** The merge used to be
# gated on `--cases`, so `--vendors rti` alone overwrote the file and
# took the other three vendors' rows with it — 4148 of 5536, reported as
# "1388 rows into …", exit 0. See the merge itself, below.
#
# ## Why two encoding versions and not five encodings
#
# `crates/wire-probe/src/main.rs` takes five encodings because this
# project can be told which framing to write. A vendor cannot: its
# `--encoding` selects the encoding **version** and the framing follows
# the type's own extensibility, which is what XTypes 1.3 §7.3.1.2.1.8
# says it must. `wireprobe-cyclone` and `wireprobe-fastdds` both fold
# `plain-cdr1`/`pl-cdr1` onto version 1 and
# `plain-cdr2`/`delimited-cdr`/`pl-cdr2` onto version 2 for exactly that
# reason.
#
# So this asks each vendor the two questions it can answer, and the
# table generator records the other three encoding rows as pointing at
# the same run rather than inventing three identical binaries and three
# identical measurements out of one.
#
# ## One build per system, not one per cell
#
# It was one build per cell: 1,340 runs of the vendor's code generator
# and 1,340 compiles per vendor, because the encoding version and the
# byte order were `sed` substitutions in the probe's template and so
# part of the binary's identity. Making them arguments took that to one
# build per case — 335 — and `wireprobe-batch` takes it to one.
#
# The corpus is one IDL, each case in a module of its own, and the
# vendor's generator runs once over all of it; the driver holds one
# function per case and is compiled and linked once; one process walks
# every case. A case the front end or the compiler refuses is excluded
# by name, with its own diagnostic, and the batch is rebuilt without it,
# so the builds a vendor actually costs are `1 + refusals` — measured,
# and printed at the end beside the cell count.
#
# A vendor with no batch driver falls back to the per-case probe with
# `--matrix`, which is one build per case. That is stated in the output
# rather than left to be inferred from the ratio.
#
# `WIREPROBE_CACHE` is a second, independent saving for the per-case
# path — it keys a finished binary by the content of everything that
# went into it. The batch path does not use it and does not need it.
#
# ## The output
#
# One TSV, tab separated, no header, written atomically at the end:
#
#   <case>  <vendor>  <version>  <endian>  <status>  <bytes-or-reason>
#
# `version` is `xcdr1` or `xcdr2`, `status` is `wrote` or `refused`, and
# a refusal carries the probe's first line of stderr rather than an
# empty cell. A vendor whose toolchain is not installed produces no rows
# at all and is named on stderr with the row count it did not produce —
# never a silent skip, and never a failure merely for being absent.
#
# That is `vendor_probe`'s answer, taken before the sweep starts. The
# probe has one of its own: exit 9, `toolchain-absent`, which
# `docs/specs/zerodds-verdicts-1.0.md` calls "not a measurement" and
# stops the collection on — as it does exit 2, the command line. Neither
# writes a cell. A probe that answers either leaves `$out` exactly as it
# was and the run exits 9 naming the vendor and the tool, because a run
# that reported success over a column it never measured is the failure
# this file's merge already exists to prevent.
set -uo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$HERE/../.." && pwd)"
# shellcheck source=../interop-vendors.sh
. "$ROOT/tools/interop-vendors.sh"

TARGET="${WIREPROBE_TARGET:-$ROOT/target/debug}"
export WIREPROBE_FILL="${WIREPROBE_FILL:-$TARGET/wireprobe-fill}"
export WIREPROBE_DUMP="${WIREPROBE_DUMP:-$TARGET/wireprobe-dump}"
export WIREPROBE_CACHE="${WIREPROBE_CACHE:-${TMPDIR:-/tmp}/wireprobe-cache}"
# Overridable so that the merge in this script can be tested without a
# vendor toolchain: a run that finds no probe measures nothing, which is
# exactly the case a subset re-record must survive without emptying the
# file it merges into.
PROBES="${WIREPROBE_PROBES:-$ROOT/tools/wireprobe}"
CORPUS="${INTEROP_CORPUS:-$ROOT/tests/interop-corpus}"

vendors="cyclone,rti,fastdds,opendds,hdds"
jobs="$(nproc 2>/dev/null || sysctl -n hw.ncpu 2>/dev/null || echo 4)"
out=""
only=""

while [ $# -gt 0 ]; do
    case "$1" in
        --vendors) vendors="$2"; shift 2 ;;
        --jobs) jobs="$2"; shift 2 ;;
        --corpus) CORPUS="$2"; shift 2 ;;
        --cases) only="$2"; shift 2 ;;
        --help|-h) sed -n '5,45p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'; exit 0 ;;
        -*) echo "vendor-corpus-sweep: \`$1\` is not an option" >&2; exit 2 ;;
        *) out="$1"; shift ;;
    esac
done
[ -n "$out" ] || { echo "vendor-corpus-sweep: an output path is required" >&2; exit 2; }

mkdir -p "$WIREPROBE_CACHE"

cases=$(cd "$CORPUS" && for d in */; do [ -f "$d/type.idl" ] && basename "$d"; done)

# The subset, if one was asked for. Every name is checked against the
# corpus before anything is built: a misspelt case that silently swept
# nothing would leave the rows it was meant to replace in place and
# report success.
if [ -n "$only" ]; then
    if [ "${only#@}" != "$only" ]; then
        wanted=$(grep -v '^[[:space:]]*$' "${only#@}") \
            || { echo "vendor-corpus-sweep: cannot read ${only#@}" >&2; exit 2; }
    else
        wanted=$(printf '%s\n' "${only//,/$'\n'}" | grep -v '^[[:space:]]*$')
    fi
    missing=""
    for c in $wanted; do
        [ -f "$CORPUS/$c/type.idl" ] || missing="$missing $c"
    done
    if [ -n "$missing" ]; then
        echo "vendor-corpus-sweep: no such case(s) in $CORPUS:$missing" >&2
        exit 2
    fi
    cases=$(printf '%s\n' $wanted)
fi
case_count=$(printf '%s\n' "$cases" | grep -c . || true)

# Named before the run and not discovered as a hole in the report: four
# rows per case per vendor, and a matrix short a vendor has fewer rows,
# which reads as a smaller matrix rather than as a missing column.
asked=""
for v in ${vendors//,/ }; do
    # Two separate absences, and both are absences: the toolchain, which
    # `vendor_probe` answers for, and the probe that drives it. Asking a
    # vendor with no probe produced four rows per case carrying a
    # shell's "No such file or directory", which is a harness gap
    # wearing a vendor's name, so the two are named apart.
    if [ ! -x "$PROBES/wireprobe-$v" ]; then
        echo "$v: NO PROBE ($PROBES/wireprobe-$v) — $((case_count * 4)) rows not measured in this run" >&2
    elif vendor_probe "$v" >/dev/null 2>&1; then
        asked="$asked $v"
    else
        vendor_skip "$v" $((case_count * 4))
    fi
done

work="$(mktemp -d)"
# What the vendors' own front ends and compilers said, kept rather than
# deleted with the temporary directory each probe builds in. A separate
# source from `warnings.tsv`, which `warning-sweep.sh` writes whole; see
# `tools/wireprobe/diagnostics.sh`.
# shellcheck source=../wireprobe/diagnostics.sh
. "$ROOT/tools/wireprobe/diagnostics.sh"
WIREPROBE_DIAGNOSTICS="$work/diagnostics"
export WIREPROBE_DIAGNOSTICS
trap 'rm -rf "$work"' EXIT
: > "$work/invocations"
: > "$work/batch-builds"

# The sentence a refusal carries, cleaned of everything that is not a
# property of the run.
#
# Bash reports a signalled child as
#
#   /root/harness-port/tree/tools/wireprobe/wireprobe-cyclone: line 150: \
#   529802 Segmentation fault      "$binary" > "$work/bytes.bin"
#
# and eight rows of the committed record carried exactly that: an
# absolute path on one machine, a process id from one run and a line
# number from one revision of the script. Re-recording on another
# machine changes all three and the diff reports eight rows moved when
# nothing moved. What is a property of the vendor is "it was killed by
# SIGSEGV", so that is what is kept.
reason_of() { # <stderr file> <fallback>
    local line
    # The first line that carries the tool's own error beats the probe's
    # summary: "rtiddsgen failed" says a tool said no, "Only 32 bit
    # bounds are accepted for enumerations" says why — and the second is
    # the measurement. The probe's full stderr is archived beside the
    # record by the caller, so nothing is lost to this selection.
    line="$(grep -m1 -iE '(^|[^a-z])error' "$1" | tr '\t' ' ' | cut -c1-200 | iconv -f utf-8 -t utf-8 -c)"
    [ -n "$line" ] || line="$(head -1 "$1" | tr '\t' ' ' | cut -c1-200 | iconv -f utf-8 -t utf-8 -c)"
    case "$line" in
        */wireprobe-*:\ line\ *)
            # `<path>: line <n>: <pid> <signal name>  <command>`
            line="the probe was killed: $(printf '%s' "$line" \
                | sed -e 's/^.*: line [0-9]*: [0-9]* //' -e 's/  .*$//')" ;;
    esac
    [ -n "$line" ] || line="$2"
    # A tool that names the case's file names it by the absolute path it
    # was given, which is a property of the checkout and not of the tool:
    # kept relative, as `archive_stderr` keeps the archive.
    printf '%s' "${line//$ROOT\//}"
}

# The complete tool output of a refusal, kept verbatim. The one-line
# reason in the record is for the table; the archive is for reading why
# a front end said no without re-running its toolchain.
TOOL_LOGS="${TOOL_LOGS:-$ROOT/tests/interop-results/sources/toolchain-logs}"
archive_stderr() { # <vendor> <case> <stderr file> [<stdout file>] [<exit>]
    mkdir -p "$TOOL_LOGS/$1"
    {
        printf '## wireprobe-%s, exit %s\n' "$1" "${5:-?}"
        if [ -n "${4:-}" ] && [ -s "$4" ]; then
            printf '### stdout\n'
            cat "$4"
        fi
        printf '### stderr\n'
        cat "$3"
    } | tr -d '\r' | sed "s|$ROOT/||g" > "$TOOL_LOGS/$1/$2.log"
}

sweep_one() { # <case> — every asked vendor, both versions, both orders
    local c=$1 d="$CORPUS/$1" t v status probe err rows e o cell
    t=$(cat "$d/type" 2>/dev/null || echo Sample)
    err="$work/err.$c.$$"
    rows="$work/rows.$c.$$"
    for v in $ASKED; do
        probe="$PROBES/wireprobe-$v"
        # One invocation, one build, four rows. `--matrix` is what makes
        # that a property of the probe rather than of the cache.
        "$probe" --idl "$d/type.idl" --type "$t" --values "$d/values.json" \
                 --matrix > "$rows" 2>"$err"
        status=$?
        # A build counted here and not inside the probe: one line per
        # (case, vendor) invocation, which is exactly the number of
        # builds a cold cache does.
        printf '%s\t%s\n' "$c" "$v" >> "$work/invocations"
        case "$status" in
            # Not a measurement, so not four rows.
            # `docs/specs/zerodds-verdicts-1.0.md` gives 9 as
            # `toolchain-absent` and 2 as the command line, and says of
            # both that "the collection stops". `vendor_probe` above
            # keeps a wholly absent vendor out of `$ASKED`, so what
            # reaches here is a toolchain that went missing between that
            # test and the probe — and it must not become four cells per
            # case saying so. A child of `xargs` cannot stop its parent,
            # so it names itself here and the parent reads the file after
            # the pipeline.
            2|9)
                printf '%s\t%s\t%s\n' "$v" "$status" \
                    "$(reason_of "$err" "the probe exited $status")" >> "$ABSENT"
                continue
                ;;
        esac
        if [ $status -ne 0 ] || [ ! -s "$rows" ]; then
            # Exit 0 and nothing on stdout is not a result. It is what a
            # probe does when a step it did not check failed, and a run
            # that recorded it as bytes would put an empty payload in a
            # table beside a vendor's real one. Measured on 2026-08-10:
            # 648 such rows from one missing helper on `$PATH`. The
            # refusal is written for all four combinations, because all
            # four are what this invocation was asked for.
            local reason
            reason="$(reason_of "$err" "the probe exited $status and produced no rows")"
            archive_stderr "$v" "$c" "$err" "$rows" "$status"
            for e in xcdr1 xcdr2; do
                for o in le be; do
                    printf '%s\t%s\t%s\t%s\trefused\t%s\n' "$c" "$v" "$e" "$o" "$reason"
                done
            done
            continue
        fi
        # Read back what the probe printed, and answer for every
        # combination it did not print. A combination with no row is not
        # dropped: it becomes a refusal that says the probe did not
        # answer it, so the matrix keeps its shape.
        for e in xcdr1 xcdr2; do
            for o in le be; do
                cell="$(awk -F'\t' -v a="$e" -v b="$o" '$1 == a && $2 == b {print $3; exit}' "$rows")"
                if [ -z "$cell" ]; then
                    # A combination the matrix run did not print. The
                    # usual cause is the one that cost the old
                    # arrangement nothing and costs this one a row: a
                    # probe that aborts — `terminate called after
                    # throwing BadParamException`, sixteen Fast DDS rows
                    # in the committed record — takes the rest of the
                    # loop down with it, where four separate processes
                    # would each have reached their own combination.
                    #
                    # So the missing one is asked for on its own. The
                    # binary is already built and the cache hands the
                    # same one back, so this costs a process launch and
                    # never a compile: the one-build-per-case property
                    # is not what is being traded away here.
                    cell="$("$probe" --idl "$d/type.idl" --type "$t" \
                                     --values "$d/values.json" \
                                     --encoding "$e" --endian "$o" 2>"$err")"
                    if [ $? -ne 0 ] || [ -z "$cell" ]; then
                        cell="!$(reason_of "$err" "the probe printed no $e/$o row")"
                        archive_stderr "$v" "$c" "$err"
                    fi
                fi
                case "$cell" in
                    '!'*) printf '%s\t%s\t%s\t%s\trefused\t%s\n' "$c" "$v" "$e" "$o" \
                              "${cell#!}" ;;
                    *)    printf '%s\t%s\t%s\t%s\twrote\t%s\n' "$c" "$v" "$e" "$o" "$cell" ;;
                esac
            done
        done
    done
    rm -f "$err" "$rows"
}

ASKED="$(echo "$asked" | tr -s ' ' | sed 's/^ //;s/ $//')"
# Where a cell that answered exit 2 or 9 says so, for the parent to read
# after the parallel run.
ABSENT="$work/toolchain-absent"
: > "$ABSENT"
export ASKED CORPUS PROBES work ROOT TOOL_LOGS ABSENT
export -f reason_of
export -f archive_stderr
export -f sweep_one

if [ -z "$ASKED" ]; then
    # Not `: > "$out"`, which is what this did. A run on a host with no
    # vendor toolchain measured nothing and then emptied the file every
    # recorded byte in the tree comes from — and exited 0, so the sweep
    # reported success and `interop-results --verify` still passed,
    # because it holds the tree against the sources and both had shrunk
    # together. Deleting a measurement is not a way of reporting that
    # one could not be taken.
    echo "vendor-corpus-sweep: no vendor was present; nothing was measured" >&2
    if [ -f "$out" ]; then
        echo "vendor-corpus-sweep: $out is left as it was ($(wc -l < "$out") row(s))" >&2
    else
        : > "$out"
    fi
    exit 1
fi

# A vendor with a batch driver is swept whole, in one build. The rest
# go through the per-case loop below.
: > "$work/batched.tsv"
# Which vendors have a batch driver is read from the driver itself, so
# one place names them and a vendor added there is swept whole without
# this file changing.
batched=""
per_case=""
for v in $ASKED; do
    if grep -q "^  $v)\$" "$PROBES/wireprobe-batch" 2>/dev/null; then
        batched="$batched $v"
    else
        per_case="$per_case $v"
    fi
done

# The batch driver has a `--cases` of its own and sweeps the whole
# corpus without one. Not passing it made a subset run measure every
# case for the batched vendors — the merge below then kept the old rows
# for the cases it had not been asked about *and* took the fresh ones,
# and the file gained 1104 duplicate rows.
printf '%s\n' "$cases" > "$work/cases"
for v in $batched; do
    if "$PROBES/wireprobe-batch" --vendor "$v" --corpus "$CORPUS" \
           --cases "$work/cases" \
           "$work/batch.$v.tsv" 2>"$work/batch.$v.log"
    then
        awk -F'\t' -v v="$v" '{print $1"\t"v"\t"$2"\t"$3"\t"$4"\t"$5}' \
            "$work/batch.$v.tsv" >> "$work/batched.tsv"
        # The batch driver reports batch builds PLUS per-case retries. The old
        # regexp no longer matched and silently substituted one build for all
        # of them. Missing accounting is a harness failure, never a default 1.
        b=$(python3 "$HERE/verify_record.py" --batch-count "$work/batch.$v.log") || exit 125
        printf '%s\t%s\n' "$v" "$b" >> "$work/batch-builds"
    else
        # A batch that failed produced nothing, and nothing is not a
        # result. The vendor is named with the rows it did not produce
        # and the run continues; a green sweep standing for a column it
        # never measured is the failure this whole tree is about.
        echo "$v: THE BATCH FAILED — $((case_count * 4)) rows not measured in this run" >&2
        tail -3 "$work/batch.$v.log" >&2
    fi
done
ASKED="$(echo "$per_case" | tr -s ' ' | sed 's/^ //;s/ $//')"
export ASKED
if [ -n "$ASKED" ]; then
    printf '%s\n' "$cases" \
        | xargs -P "$jobs" -I{} bash -c 'sweep_one "$@"' _ {} \
        > "$work/per-case.tsv"
fi
cat "$work/batched.tsv" "$work/per-case.tsv" 2>/dev/null \
    | LC_ALL=C sort > "$work/rows.tsv"

# A vendor whose toolchain went absent under the run measured nothing,
# and the merge below is not the place to sort that out: it keys on the
# rows this run produced, so a vendor with no rows keeps its old ones —
# which is right — but the run would then report success over a column it
# never measured, which is the failure this whole file is about. So
# nothing is installed and the run ends non-zero, naming the vendor and
# the tool. `$out` keeps every row of the run that had the toolchain.
if [ -s "$ABSENT" ]; then
    echo "vendor-corpus-sweep: a toolchain is absent; $out is left as it stands" >&2
    LC_ALL=C sort -u "$ABSENT" | awk -F'\t' '!seen[$1]++ {print "  " $1 ": exit " $2 " :: " $3}' >&2
    if [ -f "$out" ]; then
        echo "vendor-corpus-sweep: $(wc -l < "$out") row(s) kept in $out from a run that had it" >&2
    fi
    exit 9
fi

# A subset keeps every row it did not measure. The rows it replaces are
# those of the swept cases *for the swept vendors only*, so
# `--vendors cyclone --cases primitive-mixed` does not take Fast DDS's
# `primitive-mixed` row with it.
#
# ## Why the merge is not conditional on `--cases`
#
# It was, and a vendor subset is a subset for the same reason a case
# subset is. `record.sh --vendors rti` — the command
# `docs/OPEN-ITEMS.md` names for re-recording RTI after a probe change —
# took `vendor-corpus.tsv` from 5536 rows to 1388 and deleted Cyclone's,
# Fast DDS's and OpenDDS's 4148 in one `mv`. It reported "1388 rows
# into …" and exited 0, and the tree verifies afterwards, because the
# tree is verified against the sources and both had shrunk together —
# word for word the failure this file's own header says the merge exists
# to prevent. Measured 2026-08-13.
#
# So the merge runs whenever there is a file to merge into. The keys are
# derived from the rows this run produced, so a full sweep sweeps every
# key and the merge is the identity; nothing about a complete run
# changes. What changes is that a run which measured three vendors of
# four leaves the fourth's rows where they were — including the case
# where the fourth is absent from *this machine*, which is announced by
# `vendor_skip` and must not be a deletion.
if [ -f "$out" ]; then
    # Keyed on the rows this run actually produced and not on the cases
    # it was asked for. A sweep is allowed to measure more than it was
    # asked to — a batched vendor without a filter did exactly that —
    # and a merge that dropped by the request would then keep an old row
    # beside the new one for the same cell. Deriving the keys from the
    # output makes a duplicate impossible rather than unlikely.
    awk -F'\t' '{print $1"\t"$2}' "$work/rows.tsv" | LC_ALL=C sort -u > "$work/swept-keys"
    awk -F'\t' -v keyfile="$work/swept-keys" '
        BEGIN {
            while ((getline line < keyfile) > 0) if (line != "") swept[line] = 1
        }
        !swept[$1 "\t" $2]
    ' "$out" > "$work/kept.tsv"
    kept=$(wc -l < "$work/kept.tsv")
    cat "$work/kept.tsv" "$work/rows.tsv" | LC_ALL=C sort > "$work/merged.tsv"
    mv "$work/merged.tsv" "$out"
    echo "vendor-corpus-sweep: $kept row(s) kept from the previous run" >&2
else
    mv "$work/rows.tsv" "$out"
fi

# The rows this run measured replace theirs and no others, the same rule
# the file above follows — and on the same key. Under `--cases` the pair
# is (vendor, case) and not the vendor alone: keying on the vendor alone
# deleted 91 diagnostic rows of cases a subset run never touched, and
# reported success.
if [ -n "$only" ]; then
    printf '%s\n' "$wanted" > "$work/diagnostic-cases"
    export PROBE_DIAGNOSTICS_CASES="$work/diagnostic-cases"
fi
# shellcheck disable=SC2086
probe_diagnostics_merge "$WIREPROBE_DIAGNOSTICS" \
    "$(dirname "$out")/toolchain-warnings.tsv" $(echo "$vendors" | tr ',' ' ')
# The ratio, measured. One build per case per vendor is the shape this
# sweep is built around, and printing it is how a regression to one
# build per cell would be noticed rather than merely felt in the clock.
cells=$(wc -l < "$out")
per_case_builds=$(wc -l < "$work/invocations" 2>/dev/null || echo 0)
batch_builds=$(awk -F'\t' '{n += $2} END {print n + 0}' "$work/batch-builds" 2>/dev/null || echo 0)
builds=$((per_case_builds + batch_builds))
echo "vendor-corpus-sweep: $cells rows into $out" >&2
echo "vendor-corpus-sweep: $builds probe builds for $cells cells" >&2
for v in $batched $ASKED; do
    c=$(awk -F'\t' -v v="$v" '$2 == v' "$out" | wc -l)
    b=$(awk -F'\t' -v v="$v" '$1 == v {print $2}' "$work/batch-builds" 2>/dev/null)
    if [ -n "$b" ]; then
        echo "  $v: $b build(s) for $c cell(s), batched" >&2
    else
        b=$(grep -c "	$v\$" "$work/invocations" 2>/dev/null || echo 0)
        echo "  $v: $b build(s) for $c cell(s), one per case" >&2
    fi
done
vendor_report || true
