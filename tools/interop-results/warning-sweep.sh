#!/bin/bash
# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 ZeroDDS Contributors
#
# warning-sweep.sh — what each front end *said* about each case, as
# opposed to what it did with it.
#
#   tools/interop-results/warning-sweep.sh out.tsv
#   tools/interop-results/warning-sweep.sh --systems cyclone,opendds out.tsv
#
#   --systems <list>  default cyclone,rti,fastdds,opendds,omniorb,tao,jacorb
#   --jobs <n>        cases in parallel; default the core count
#   --corpus <dir>    default tests/interop-corpus in this checkout
#
# ## Why this exists
#
# A vendor front end's warning is a statement, and two of this
# project's most valuable results came out of one. Cyclone's `idlc`
# says *"No default extensibility provided … Currently the default
# extensibility for these types is 'final', but this may change to
# 'appendable' in a future release"* — which is the root cause of
# issues #41 and #42, and is in no clause and in no byte. TAO says
# `Warning … error in lookup of symbol` for an annotation it does not
# know, and without that line TAO reads as a conforming implementation
# where in fact it parsed the annotation and dropped it.
#
# Every sweep in this directory throws that away. `vendor-corpus-sweep`
# keeps the first line of stderr and only when the probe failed;
# `wireprobe-batch` filters warnings out of the blame so that a warning
# about one case cannot exclude it, which is right and which also means
# no warning survives the run. So a system whose bytes agree while its
# front end warned about the type was recorded as agreeing silently,
# and those are not the same thing.
#
# ## Why it is cheap
#
# A warning belongs to the **front end**, so this runs the front end
# and stops: no `wireprobe-fill`, no compile, no link, no execution. A
# re-record of the byte columns is 91 batch builds for Cyclone, 62 for
# Fast DDS and 107 for OpenDDS, each one a code generation plus a
# compile plus a link. This is one code generation per case per system
# and nothing after it — 335 × 7 generator runs, no compiler invoked at
# all. It is a separate measurement and not a re-record: it produces no
# bytes and changes none.
#
# ## What it records, and what decides a severity
#
# Every line every front end printed, with the exit status of the run
# it came from:
#
#   <case>  <system>  <program>  <exit>  <severity>  <text>
#
# `program` because OpenDDS runs two — `tao_idl` writes the C++ mapping
# and `opendds_idl` the type support — and they disagree about which
# annotations they know, which is only visible if the two are kept
# apart.
#
# `severity` is what the diagnostic itself says, never what the exit
# status implies, because on this corpus the two contradict each other:
# `tao_idl` prints `Error - …: fixed data types are not supported` and
# **exits 0**, on all ten `fixed` cases. A classifier that read the
# exit status would file 294 lines saying `Error` as warnings.
#
# A line matching none of a system's patterns is recorded as
# `unclassified` and counted on stderr. It is never dropped: an
# unrecognised diagnostic is a front end that changed under this
# script, which is the one thing here worth being told about.
#
# The text is normalised for the two things that are properties of the
# run and not of the front end — the temporary directory and the line
# and column numbers — and for nothing else. `java.util.logging`'s
# timestamp goes the same way, for the same reason.
set -uo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$HERE/../.." && pwd)"
# shellcheck source=../interop-vendors.sh
. "$ROOT/tools/interop-vendors.sh"

TARGET="${WIREPROBE_TARGET:-$ROOT/target/debug}"
CORBA_IDL="${WIREPROBE_CORBA_IDL:-$TARGET/wireprobe-corba-idl}"
OPENDDS_IDL="${OPENDDS_IDL:-$OPENDDS_HOME/bin/opendds_idl}"
CORPUS="${INTEROP_CORPUS:-$ROOT/tests/interop-corpus}"

systems="cyclone,rti,fastdds,opendds,hdds,omniorb,tao,jacorb"
jobs="$(nproc 2>/dev/null || sysctl -n hw.ncpu 2>/dev/null || echo 4)"
out=""

while [ $# -gt 0 ]; do
    case "$1" in
        --systems) systems="$2"; shift 2 ;;
        --jobs) jobs="$2"; shift 2 ;;
        --corpus) CORPUS="$2"; shift 2 ;;
        --help|-h) sed -n '5,20p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'; exit 0 ;;
        -*) echo "warning-sweep: \`$1\` is not an option" >&2; exit 2 ;;
        *) out="$1"; shift ;;
    esac
done
[ -n "$out" ] || { echo "warning-sweep: an output path is required" >&2; exit 2; }

cases=$(cd "$CORPUS" && for d in */; do [ -f "$d/type.idl" ] && basename "$d"; done)
case_count=$(printf '%s\n' "$cases" | grep -c . || true)

asked=""
for s in ${systems//,/ }; do
    if vendor_probe "$s" >/dev/null 2>&1; then
        asked="$asked $s"
    else
        vendor_skip "$s" "$case_count"
    fi
done
ASKED="$(echo "$asked" | tr -s ' ' | sed 's/^ //;s/ $//')"
if [ -z "$ASKED" ]; then
    echo "warning-sweep: no front end was present; nothing was measured" >&2
    # The file stays. Truncating it here made "nothing was measured"
    # into "the record is now empty" on any machine without the front
    # ends, and reported success — see `vendor-corpus-sweep.sh`'s merge
    # for the same shape and the run that measured it.
    if [ -f "$out" ]; then
        echo "warning-sweep: $(wc -l < "$out") row(s) kept in $out from a run that had them" >&2
    fi
    exit 0
fi

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

# ---------------------------------------------------------------------
# The severity tables. One per front end, written from what the corpus
# measured, and every one of them ends in `unclassified` rather than in
# a default.
# ---------------------------------------------------------------------

# classify <system> <line> -> `noise`, `warning`, `error` or `unclassified`
classify() {
    local s=$1 line=$2
    case "$s" in
        cyclone)
            case "$line" in
                *"Warning:"*) echo warning ;;
                # Three advisories `idlc` prints with no severity word
                # in front of them. The first is issues #41 and #42's
                # root cause and the reason this file exists.
                "No default extensibility provided"*) echo warning ;;
                *"Explicit defaults are not supported yet"*) echo warning ;;
                *"Inheriting from appendable structs is unsafe"*) echo warning ;;
                "Failed to compile"*) echo noise ;;
                *) echo error ;;
            esac ;;
        rti)
            case "$line" in
                "INFO "*) echo noise ;;
                "WARN"*) echo warning ;;
                "ERROR "*) echo error ;;
                *) echo unclassified ;;
            esac ;;
        fastdds)
            case "$line" in
                "openjdk version"*|"OpenJDK "*) echo noise ;;
                "Processing the file"*|"Generating "*|"Adding project"*) echo noise ;;
                # A Java stack trace under the diagnostic that caused it.
                "	at "*|"        at "*|" at "*|"Caused by:"*) echo noise ;;
                *"warning:"*|*"WARNING:"*) echo warning ;;
                *"error:"*|*"ERROR:"*) echo error ;;
                # `fastddsgen` re-prints a `NullPointerException`'s
                # message on its own line under the trace.
                "java.lang."*|"Cannot invoke"*) echo error ;;
                *) echo unclassified ;;
            esac ;;
        opendds|tao)
            case "$line" in
                # `tao_idl` says this on every run, whatever the IDL is.
                "TAO_IDL: warning, GPERF"*) echo noise ;;
                "Perfect Hashing"*|"Now, using Dynamic Hashing"*) echo noise ;;
                "To use Perfect Hashing"*|"-Build gperf"*) echo noise ;;
                "-Set the environment"*|"-Refer to Operation Lookup"*) echo noise ;;
                "Warning - "*) echo warning ;;
                "Error - "*|"Fatal Error"*) echo error ;;
                # The tally line both generators print after a failure.
                *" found "*" error"*) echo noise ;;
                *"syntax error"*) echo error ;;
                *) echo unclassified ;;
            esac ;;
        omniorb)
            case "$line" in
                *"Warning:"*) echo warning ;;
                "omniidl: "*" error"*) echo noise ;;
                *) echo error ;;
            esac ;;
        jacorb)
            case "$line" in
                # `java.util.logging` writes a header line naming the
                # logger and then the message; the header is not a
                # diagnostic, and neither is the source line the parser
                # echoes under a syntax error.
                *"org.jacorb.idl.lexer emit_error") echo noise ;;
                "SEVERE: "*) echo error ;;
                "WARNING: "*) echo warning ;;
                "Can't recover from previous error"*) echo noise ;;
                *) echo noise ;;
            esac ;;
        hdds)
            case "$line" in
                *"warning"*|*"Warning"*) echo warning ;;
                *"error"*|*"Error"*) echo error ;;
                *) echo unclassified ;;
            esac ;;
        *) echo unclassified ;;
    esac
}

# The one thing a front end says that belongs to the run and not to the
# front end: where the file was, which line it was on, and what time it
# is. Everything else is kept verbatim.
normalise() {
    sed -e "s#$work/[A-Za-z0-9._-]*/##g" \
        -e 's#/tmp/[A-Za-z0-9._/-]*/gen\.idl#gen.idl#g' \
        -e 's#/[A-Za-z0-9._/-]*/bin/\(tao_idl\|opendds_idl\|idlc\|omniidl\|rtiddsgen\)#\1#g' \
        -e 's#gen\.idl:[0-9][0-9]*:[0-9][0-9]*#gen.idl:L:C#g' \
        -e 's#gen\.idl:[0-9][0-9]*#gen.idl:L#g' \
        -e 's#"gen\.idl", line [0-9][0-9]*#"gen.idl", line L#g' \
        -e 's#gen\.idl, line: *[0-9][0-9]*(\([0-9][0-9]*\))#gen.idl, line: L#g' \
        -e 's#gen\.idl line [0-9][0-9]*:[0-9][0-9]*#gen.idl line L:C#g' \
        -e 's#^[A-Z][a-z][a-z] [0-9][0-9]*, [0-9][0-9][0-9][0-9] [0-9:]* [AP]M ##' \
        -e 's#@[0-9a-f]\{6,\}#@OBJECT#g' \
        -e 's#\x1b\[[0-9;]*m##g' \
        -e 's#\r##g'
}

# run_front_end <system> <case dir> <build dir> — leaves one log per
# program in the build directory and prints `<program> <exit>` lines.
run_front_end() {
    local s=$1 d=$2 b=$3 t st
    t=$(cat "$d/type" 2>/dev/null || echo Sample)
    case "$s" in
        cyclone|rti|fastdds|opendds|hdds) cp "$d/type.idl" "$b/gen.idl" ;;
        omniorb|tao|jacorb)
            # A case with no CORBA form never reaches an ORB's front
            # end, so it has no diagnostic and gets no row. That hole is
            # already `no form` in both tables and saying it twice would
            # put a front end's name on a refusal it never made.
            "$CORBA_IDL" --idl "$d/type.idl" --type "$t" --out "$b/gen.idl" 2>/dev/null || return 1 ;;
    esac
    case "$s" in
        cyclone)
            ( cd "$b" && "$CYCLONE_HOME/bin/idlc" gen.idl ) > "$b/idlc.log" 2>&1
            echo "idlc $?" ;;
        rti)
            ( cd "$b" && "$NDDSHOME/bin/rtiddsgen" -language C -platform "$RTI_TARGET" \
                  -replace gen.idl ) > "$b/rtiddsgen.log" 2>&1
            echo "rtiddsgen $?" ;;
        fastdds)
            ( cd "$b" && "$FASTDDSGEN" -t . -replace gen.idl ) > "$b/fastddsgen.log" 2>&1
            echo "fastddsgen $?" ;;
        opendds)
            # Both, and kept apart: `tao_idl -Sa -St` writes the C++
            # mapping and `opendds_idl` the type support, and the two
            # know different annotations.
            ( cd "$b" && "$TAO_IDL" -I"$OPENDDS_HOME/include" -I"$TAO_ROOT" -Sa -St \
                  --idl-version 4 gen.idl ) > "$b/tao_idl.log" 2>&1
            echo "tao_idl $?"
            ( cd "$b" && "$OPENDDS_IDL" -I"$OPENDDS_HOME/include" --idl-version 4 gen.idl ) \
                > "$b/opendds_idl.log" 2>&1
            echo "opendds_idl $?" ;;
        omniorb)
            ( cd "$b" && "$OMNIIDL" -bcxx -Wbh=.hh -Wbs=SK.cc gen.idl ) > "$b/omniidl.log" 2>&1
            echo "omniidl $?" ;;
        tao)
            ( cd "$b" && "$TAO_IDL" -as -I"$TAO_HOME/include" -o . gen.idl ) > "$b/tao_idl.log" 2>&1
            echo "tao_idl $?" ;;
        jacorb)
            ( cd "$b" && "$JACORB_HOME/bin/idl" -d . gen.idl ) > "$b/jacorb-idl.log" 2>&1
            echo "jacorb-idl $?" ;;
        hdds)
            # `hddsgen` writes the generated source to standard output
            # and its diagnostics to standard error; only the latter is
            # this sweep's measurement.
            ( cd "$b" && "$HDDSGEN" gen rust gen.idl > /dev/null ) > "$b/hddsgen.log" 2>&1
            echo "hddsgen $?" ;;
    esac
    return 0
}

sweep_one() { # <case> — every asked front end, one generator run each
    local c=$1 d="$CORPUS/$1" s b program st
    for s in $ASKED; do
        b="$work/$s.$c.$$"
        rm -rf "$b"; mkdir -p "$b"
        run_front_end "$s" "$d" "$b" > "$b/programs" || { rm -rf "$b"; continue; }
        while read -r program st; do
            [ -f "$b/$program.log" ] || continue
            normalise < "$b/$program.log" | while IFS= read -r line; do
                [ -n "${line// /}" ] || continue
                printf '%s\t%s\t%s\t%s\t%s\t%s\n' \
                    "$c" "$s" "$program" "$st" "$(classify "$s" "$line")" \
                    "$(printf '%s' "$line" | tr '\t' ' ')"
            done
        done < "$b/programs"
        rm -rf "$b"
    done
}

export ASKED CORPUS CORBA_IDL OPENDDS_IDL work
export -f classify normalise run_front_end sweep_one

printf '%s\n' "$cases" \
    | xargs -P "$jobs" -I{} bash -c 'sweep_one "$@"' _ {} \
    | LC_ALL=C sort -u > "$work/rows.tsv"
mv "$work/rows.tsv" "$out"

lines=$(wc -l < "$out")
echo "warning-sweep: $lines distinct diagnostic line(s) into $out" >&2
echo "warning-sweep: $((case_count * $(echo "$ASKED" | wc -w))) front-end runs, no compile" >&2
for s in $ASKED; do
    w=$(awk -F'\t' -v s="$s" '$2 == s && $5 == "warning"' "$out" | wc -l)
    n=$(awk -F'\t' -v s="$s" '$2 == s && $5 == "warning" {print $1}' "$out" | sort -u | wc -l)
    echo "  $s: $w warning line(s) over $n case(s)" >&2
done
# An unrecognised diagnostic is a front end that changed under this
# script's severity table. It is named, it is in the file, and it is
# never quietly a warning or quietly an error.
u=$(awk -F'\t' '$5 == "unclassified"' "$out" | wc -l)
if [ "$u" -gt 0 ]; then
    echo "warning-sweep: $u line(s) match no severity pattern; they are in the file as \`unclassified\`" >&2
    awk -F'\t' '$5 == "unclassified" {print "  " $2 ": " $6}' "$out" | sort -u | head -20 >&2
fi
vendor_report || true
