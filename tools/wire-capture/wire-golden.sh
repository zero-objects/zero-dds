#!/bin/bash
# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 ZeroDDS Contributors
#
# wire-golden.sh — the octets a product puts on a socket, per corpus
# case, per implementation.
#
#   . tools/interop-env.sh
#   tools/wire-capture/wire-golden.sh --all --out /root/wire-goldens
#   tools/wire-capture/wire-golden.sh --impl cyclone --out /root/wire-goldens
#
#   --all             every implementation below, each on a domain
#                     offset of its own, then the frames, `run-log.tsv`
#                     and `tests/interop-results/wire-goldens.tsv` back
#                     into the tree. The whole measurement, one command.
#   --jobs <n>        implementations at once under --all, default 3
#   --assemble        the frames, `run-log.tsv` and the table again out
#                     of an --out directory, without running anything —
#                     for re-running one implementation on its own
#   --retry           with --impl: re-run and splice into the rows that
#                     are already in --out. On its own it takes the cases
#                     whose row says the run did not happen — nobody
#                     read, nobody wrote, the capture missed it — and no
#                     others: `!refused` is the product's answer and
#                     `!unstable` is a finding. With --cases it takes
#                     that list instead, which is what a fixed template
#                     needs.
#   --impl <name>     cyclone | rti | fastdds | opendds | opensplice
#                     | hdds | ros-fastrtps | ros-cyclone
#                     | omniorb | tao | jacorb
#   --cases <list>    comma separated; default every row of cases.tsv
#   --encodings <l>   1,2; default both. The ORBs have only 1.
#   --count <n>       samples per run, default 6. Must be even: the
#                     first is poison and they alternate.
#   --out <dir>       where the frames, the rows and the log go
#   --keep-pcap       keep the capture beside the frames
#
# ## What a run is
#
# One case, one encoding version, one domain of its own. A capture is
# started on the loopback filtered to that domain's UDP port range, a
# subscriber and a publisher are started, the publisher alternates a
# **poison** sample and the **corpus** sample through one writer, and
# the capture is decoded by `extract-rtps.py`.
#
# ## The four checks a run has to pass before it produces a row
#
# 1. the subscriber read as many samples as the publisher wrote — a
#    capture of a writer nobody was listening to is a capture of
#    nothing;
# 2. every sequence number 1..count is in the capture;
# 3. the three corpus samples are byte-identical to each other, and so
#    are the three poison samples — a serialiser that is not a function
#    of its input would make every column below meaningless;
# 4. the corpus payload differs from the poison payload — if it does
#    not, the capture is the same octets twice and the extractor is
#    reading something that is not the sample.
#
# A run that fails any of them writes `!` and the reason into the row
# and does not write a golden. A silent skip would read as agreement.
set -uo pipefail

TOOLS="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$TOOLS/../.." && pwd)"
CORPUS="$ROOT/tests/interop-corpus"
TARGET="${WIREPROBE_TARGET:-$ROOT/target/debug}"
FILL="${WIREPROBE_FILL:-$TARGET/wireprobe-fill}"
ROS_PACKAGE="${WIREPROBE_ROS_PACKAGE:-$TARGET/wireprobe-ros-package}"
ROS_PREFIX="${ROS_PREFIX:-/root/micromamba/envs/ros2}"

impl=""
cases=""
encodings="1,2"
count=6
out=""
keep_pcap=0
domain_offset=0
all=0
assemble_only=0
retry=0
jobs=3

# The eleven, in the order `compare-levels.py` writes them.
ALL_IMPLS="cyclone rti fastdds opendds opensplice hdds ros-fastrtps ros-cyclone omniorb tao jacorb"
# One offset per implementation of a transport, and the step is the
# number of domain slots `cases.tsv` cycles through (20 to 39). The
# eight RTPS implementations at 20 slots each reach domain 179; DDSI-RTPS
# 2.5 §9.6.1.1 puts domain 233's first port past 65535, and HDDS refuses
# a domain id above 232 (`InvalidDomainId(233)`, measured). The three
# ORBs speak GIOP on 12300 + domain and touch no RTPS port, so they are
# counted apart: one offset each among themselves, and an ORB and an
# RTPS implementation on the same offset do not meet. Counted together,
# the eleventh would start at domain 220.
DOMAIN_STEP=20

while [ $# -gt 0 ]; do
    case "$1" in
        --impl) impl="$2"; shift 2 ;;
        --all) all=1; shift ;;
        --assemble) assemble_only=1; shift ;;
        --retry) retry=1; shift ;;
        --jobs) jobs="$2"; shift 2 ;;
        --cases) cases="$2"; shift 2 ;;
        --encodings) encodings="$2"; shift 2 ;;
        --count) count="$2"; shift 2 ;;
        --out) out="$2"; shift 2 ;;
        --keep-pcap) keep_pcap=1; shift ;;
        # Added to every case's domain id. Two implementations running
        # the same case at the same time would otherwise share a domain,
        # a port range and a topic name, and each would find the other's
        # writer in its capture. Sequential runs need no offset; --all
        # gives every implementation one.
        --domain-offset) domain_offset="$2"; shift 2 ;;
        --help|-h) sed -n '5,37p' "${BASH_SOURCE[0]}"; exit 0 ;;
        *) echo "wire-golden: \`$1\` is not an option" >&2; exit 2 ;;
    esac
done
[ -n "$out" ] || { echo "wire-golden: --out is required" >&2; exit 2; }
[ $((count % 2)) -eq 0 ] || { echo "wire-golden: --count must be even" >&2; exit 2; }

# --------------------------------------------------------------- the nine
#
# `--all` is this script again, once per implementation, and then the
# three files the tree keeps. It is a mode rather than a second script
# because every argument it forwards — the case list, the sample count,
# the encodings — has exactly one meaning, and a wrapper would be a
# second place for that meaning to drift.
#
# The offsets are what makes the concurrency safe. `extract-rtps.py`
# attributes a packet by the destination port's domain, so two
# implementations on disjoint domain ranges cannot appear in each
# other's rows even though the capture filter also matches any datagram
# whose payload starts with `RTPS`.

# One finished job, not all of them.
#
# `wait -n` returns the *exit status of the job that finished*, and a
# non-zero one is ordinary here: an implementation whose last case was
# refused exits non-zero. Writing `wait -n || wait` reads as "fall back
# if `wait -n` is unsupported" and does the opposite — it waits for
# every remaining job on any ordinary failure, which is what serialised
# the 2026-08-19 sweep into two implementations at a time and left tao
# and jacorb unlaunched for half an hour. The version test is the check
# that was meant.
wait_one() {
    if [ "${BASH_VERSINFO[0]}" -gt 4 ] ||
       { [ "${BASH_VERSINFO[0]}" -eq 4 ] && [ "${BASH_VERSINFO[1]}" -ge 3 ]; }; then
        wait -n
    else
        wait
    fi
    return 0
}

run_all() {
    mkdir -p "$out"
    # The children get the signal too. Each of them has a trap of its
    # own for its `tshark` and its work directory, and a `TERM` is what
    # lets that trap run; without this line a `Ctrl-C` here kills the
    # parent, leaves three `--impl` shells running, and the sweep goes
    # on writing rows into a directory nobody is reading.
    trap 'kill $(jobs -p) 2>/dev/null; exit 130' INT TERM
    local rtps_index=0 giop_index=0 running=0 name offset
    for name in $ALL_IMPLS; do
        case "$name" in
            omniorb|tao|jacorb) offset=$((giop_index * DOMAIN_STEP)); giop_index=$((giop_index + 1)) ;;
            *) offset=$((rtps_index * DOMAIN_STEP)); rtps_index=$((rtps_index + 1)) ;;
        esac
        local forwarded=(--impl "$name" --out "$out"
                         --count "$count" --encodings "$encodings"
                         --domain-offset "$offset")
        [ -n "$cases" ] && forwarded+=(--cases "$cases")
        [ $keep_pcap -eq 1 ] && forwarded+=(--keep-pcap)
        "${BASH_SOURCE[0]}" "${forwarded[@]}" > "$out/$name.log" 2>&1 &
        running=$((running + 1))
        if [ "$running" -ge "$jobs" ]; then wait_one; running=$((running - 1)); fi
    done
    wait
    assemble
}

# ------------------------------------------------------------ the assembly
#
# The three files the tree keeps, out of a directory `--impl` runs
# filled. Separate from `run_all` so that an implementation re-run on its
# own — which is what a product that will not share a host needs —
# rebuilds the table without re-running the other eight.

assemble() {
    # The frames are the committed evidence and `recheck.sh` re-derives
    # every row from them, so they are replaced wholesale: a frames
    # directory holding one run's octets beside another's would let
    # `recheck.sh` agree with a table neither run produced.
    rm -rf "$TOOLS/frames"
    cp -a "$out/frames" "$TOOLS/frames"

    # A run that failed check 1 — nobody read the samples back — still
    # reached the extractor and still left a frames file. `recheck.sh`
    # has no subscriber log, would find nothing wrong with those octets
    # and would call the file `ok`, so the frames would answer a question
    # the table refuses to. Those are dropped.
    #
    # `!unstable` frames are **kept**: that check is a property of the
    # octets, `recheck.sh` re-derives it from them, and the three rows
    # the 2026-08-13 run recorded that way are a finding about RTI's send
    # buffers rather than a run that did not happen.
    local name label
    for name in $ALL_IMPLS; do
        [ -f "$out/rows-$name.tsv" ] || continue
        [ -d "$TOOLS/frames/$name" ] || continue
        for label in "$TOOLS/frames/$name"/*.frames; do
            [ -e "$label" ] || continue
            local base case_name version
            base="$(basename "$label" .frames)"
            case_name="${base%.*}"; version="${base##*.xcdr}"
            awk -F'\t' -v c="$case_name" -v v="xcdr$version" \
                '$2 == c && $3 == v && ($4 == "ok" || $4 == "!unstable") { found = 1 }
                 END { exit !found }' \
                "$out/rows-$name.tsv" || rm -f "$label"
        done
    done

    # The three run-level checks no frame can carry. Assembled from the
    # per-run logs here rather than transcribed later, because a claim
    # whose evidence lives only on the measurement host is a claim with
    # a deadline.
    {
        cat <<'HEAD'
# The run-level checks the committed frames cannot carry: how many
# samples the publisher wrote, how many endpoints it matched, and how
# many the subscriber read back. `recheck.sh` re-derives every octet in
# the table from `frames/` and can re-derive none of these three,
# because none of them is in the octets — a capture of a writer nobody
# was listening to looks exactly like a capture of one that was.
#
# One line per row of `tests/interop-results/wire-goldens.tsv` that
# produced octets. Written by `wire-golden.sh --all` out of the per-run
# logs, which stay on the measurement host; this is what they said.
#
# impl	case	encoding	wrote	matched	read
HEAD
        cat "$out"/run-log-*.tsv 2>/dev/null | sort
    } > "$out/run-log.tsv"
    cp "$out/run-log.tsv" "$TOOLS/run-log.tsv"

    python3 "$TOOLS/compare-levels.py" --rows "$out" --root "$ROOT" \
        --out "$ROOT/tests/interop-results/wire-goldens.tsv"

    local rows_total goldens cases_total
    rows_total=$(grep -vc '^#' "$ROOT/tests/interop-results/wire-goldens.tsv")
    goldens=$(grep -v '^#' "$ROOT/tests/interop-results/wire-goldens.tsv" \
              | awk -F'\t' '$11 == "same" || $11 == "wire-pads" || $11 == "differs"' \
              | wc -l)
    # Counted off the table and not off `cases.tsv`, so a run narrowed
    # by --cases reports what it measured rather than what the file
    # holds.
    cases_total=$(grep -v '^#' "$ROOT/tests/interop-results/wire-goldens.tsv" \
                  | cut -f1 | sort -u | wc -l)
    echo "wire-golden: $cases_total cases x $(echo $ALL_IMPLS | wc -w) implementations — $rows_total rows, $goldens goldens" >&2

    # The ledger, from the same command that produced the table.
    #
    # It was a second command until 2026-08-20, and the run of
    # 2026-08-19 is what that cost: the table it produced never reached
    # the tree, and `coverage.tsv` — the one file that says which of the
    # 146 behaviours a capture reached — was never written at all, so
    # nothing anywhere reported that the run had happened. A ledger
    # somebody has to remember to regenerate is a ledger that describes
    # the run before last.
    #
    # Its exit status is this function's: a behaviour with neither a
    # case nor a reason, or a case no run reached, is an unfinished
    # claim and a sweep that leaves one has not finished either.
    python3 "$TOOLS/coverage.py" --root "$ROOT" --out "$TOOLS/coverage.tsv"
}

if [ $all -eq 1 ]; then
    [ -z "$impl" ] || { echo "wire-golden: --all and --impl are exclusive" >&2; exit 2; }
    run_all
    exit 0
fi
if [ $assemble_only -eq 1 ]; then
    assemble
    exit 0
fi

[ -n "$impl" ] || { echo "wire-golden: --impl or --all is required" >&2; exit 2; }

mkdir -p "$out/frames/$impl" "$out/logs/$impl"
rows="$out/rows-$impl.tsv"
run_log="$out/run-log-$impl.tsv"

# ------------------------------------------------------- what a kill leaves
#
# `run_dds` and `run_giop` start `tshark` in the background and neither
# returns before stopping it, so an ordinary run leaves nothing. A run
# that is *killed* leaves both: the capture, and the `mktemp -d` the
# build wrote into.
#
# Measured, not supposed. On 2026-08-20 this host still carried three
# `tshark`/`dumpcap` pairs from the 2026-08-19 sweep, PIDs 3208548,
# 3426376 and 3728753, at eleven hours of run time, each writing to a
# `/tmp/tmp.*` whose owning shell had been gone since the evening
# before. Their filter is the harness's own —
# `(udp portrange L-H) or (udp[8:4] = 0x52545053)` — and the second half
# of it matches *every* RTPS datagram any other run on this host sends,
# so a leaked capture is not idle: it writes for as long as anything
# else on the machine speaks RTPS. Three of them had reached 31 MB.
#
# The trap is on EXIT as well as on the two signals because `set -uo
# pipefail` leaves an unset variable fatal, and an unset variable half
# way through a run is the same leak with no signal in front of it.
capture_pid=""
current_work=""
cleanup_run() {
    [ -n "$capture_pid" ] && kill -INT "$capture_pid" 2>/dev/null
    [ -n "$current_work" ] && rm -rf "$current_work"
    return 0
}
trap cleanup_run EXIT INT TERM

# --------------------------------------------------------------- the retry
#
# A row can fail for two different reasons and only one of them is a
# measurement.
#
#   `!refused`   the product's front end would not take the type, or its
#                `create_datawriter` would not take the representation.
#                That is the product's answer and re-running it produces
#                the same answer. **Never retried.**
#   `!unstable`  the octets are not a function of the sample even in
#                steady state — RTI's send-buffer pool, three rows of the
#                2026-08-13 run. Also an answer. **Never retried.**
#   everything else — `!unread`, `!publisher`, `!nothing-captured`,
#                `!no-server` — is a run that did not happen: nobody
#                read, nobody wrote, or the capture missed it. On a host
#                carrying four sweeps at load 24 those appear by the
#                dozen and they are the *host's* answer, not the
#                product's. **Retried, on request.**
#
# `--retry` re-runs exactly those cases and splices the new rows over the
# old ones. It is not "run until green": a case that fails the same way
# twice keeps its failure, and `!refused` and `!unstable` are never in
# the list.
#
# `--retry --cases a,b,c` names the set instead of deriving it, and
# splices the same way. That is what a fixed *template* needs — a
# missing `<stdbool.h>`, an IDL compiler at the wrong language version —
# because those rows say `!refused`, which is never in the derived set
# and must not be: a refusal is the product's answer until something in
# this directory changes, and then it is named by hand.
retry_cases=""
if [ $retry -eq 1 ]; then
    [ -s "$rows" ] || { echo "wire-golden: $rows has no rows to retry" >&2; exit 2; }
    retry_cases=$cases
    [ -n "$retry_cases" ] || retry_cases=$(awk -F'\t' \
        '$4 == "!unread" || $4 == "!publisher" || $4 == "!nothing-captured" || $4 == "!no-server" {print $2}' \
        "$rows" | sort -u | paste -sd, -)
    if [ -z "$retry_cases" ]; then
        echo "wire-golden: $impl — nothing to retry" >&2
        exit 0
    fi
    mv "$rows" "$rows.previous"
    [ -f "$run_log" ] && mv "$run_log" "$run_log.previous"
    cases="$retry_cases"
fi

: > "$rows"
: > "$run_log"

# ---------------------------------------------------------------- cases

case_rows() {
    grep -v '^#' "$TOOLS/cases.tsv" | tail -n +2 | awk -F'\t' 'NF >= 5'
}

selected() {
    if [ -z "$cases" ]; then case_rows; return; fi
    local wanted
    wanted=",$cases,"
    case_rows | while IFS=$'\t' read -r name domain control rows_ why; do
        case "$wanted" in *,"$name",*) printf '%s\t%s\t%s\t%s\t%s\n' \
            "$name" "$domain" "$control" "$rows_" "$why" ;; esac
    done
}

# ------------------------------------------------------------- builders
#
# One build per case. The batched form under `tools/wireprobe/` exists
# because 335 cases times four generators is hours; 23 cases times one
# is minutes, and a build per case keeps one topic in each capture,
# which is the property the whole attribution rests on.

build_cyclone() {
    local case_name=$1 work=$2 type_name=$3
    cp "$CORPUS/$case_name/type.idl" "$work/gen.idl" || return 1
    ( cd "$work" && "${CYCLONE_HOME:-/opt/cyclone}/bin/idlc" gen.idl ) \
        > "$work/idlc.log" 2>&1 || { sed -n 1,6p "$work/idlc.log" >&2; return 1; }
    local c_name
    c_name=$("$FILL" --vendor cyclone --idl "$CORPUS/$case_name/type.idl" \
                     --type "$type_name" --print-c-name) || return 1
    "$FILL" --vendor cyclone --idl "$CORPUS/$case_name/type.idl" \
            --type "$type_name" --values "$CORPUS/$case_name/values.json" \
            --out "$work/fill-sample.inc" || return 1
    "$FILL" --vendor cyclone --idl "$CORPUS/$case_name/type.idl" \
            --type "$type_name" --values "$TOOLS/poison/$case_name.json" \
            --out "$work/fill-poison.inc" || return 1
    sed -e "s/@TYPE@/$c_name/g" "$TOOLS/cyclone-wire-template.c" > "$work/probe.c"
    ( cd "$work" && gcc -o probe probe.c gen.c -I. \
        -I"${CYCLONE_HOME:-/opt/cyclone}/include" \
        -L"${CYCLONE_HOME:-/opt/cyclone}/lib" -lddsc \
        -Wl,-rpath,"${CYCLONE_HOME:-/opt/cyclone}/lib" ) \
        > "$work/cc.log" 2>&1 || { sed -n 1,12p "$work/cc.log" >&2; return 1; }
}

build_fastdds() {
    local case_name=$1 work=$2 type_name=$3
    cp "$CORPUS/$case_name/type.idl" "$work/gen.idl" || return 1
    ( cd "$work" && "${FASTDDSGEN:?}" -t . -replace gen.idl ) \
        > "$work/gen.log" 2>&1 || { sed -n 1,8p "$work/gen.log" >&2; return 1; }
    local cpp_name
    cpp_name=$("$FILL" --vendor fastdds --idl "$CORPUS/$case_name/type.idl" \
                       --type "$type_name" --print-c-name) || return 1
    "$FILL" --vendor fastdds --idl "$CORPUS/$case_name/type.idl" \
            --type "$type_name" --values "$CORPUS/$case_name/values.json" \
            --out "$work/fill-sample.inc" || return 1
    "$FILL" --vendor fastdds --idl "$CORPUS/$case_name/type.idl" \
            --type "$type_name" --values "$TOOLS/poison/$case_name.json" \
            --out "$work/fill-poison.inc" || return 1
    sed -e "s/@TYPE@/$cpp_name/g" "$TOOLS/fastdds-wire-template.cpp" > "$work/probe.cpp"
    ( cd "$work" && g++ -std=c++17 -o probe probe.cpp genPubSubTypes.cxx \
        genTypeObjectSupport.cxx -I. -I"${FASTDDS_HOME:-/opt/fastdds}/include" \
        -L"${FASTDDS_HOME:-/opt/fastdds}/lib" -lfastcdr -lfastdds \
        -Wl,-rpath,"${FASTDDS_HOME:-/opt/fastdds}/lib" ) \
        > "$work/cc.log" 2>&1 || { sed -n 1,12p "$work/cc.log" >&2; return 1; }
}

build_rti() {
    local case_name=$1 work=$2 type_name=$3
    cp "$CORPUS/$case_name/type.idl" "$work/gen.idl" || return 1
    ( cd "$work" && NDDSHOME="${NDDSHOME:?}" "$NDDSHOME/bin/rtiddsgen" -language C \
        -platform "${RTI_TARGET:-x64Linux4gcc7.3.0}" -replace gen.idl ) \
        > "$work/gen.log" 2>&1 || { sed -n 1,8p "$work/gen.log" >&2; return 1; }
    local cpp_name
    cpp_name=$("$FILL" --vendor rti --idl "$CORPUS/$case_name/type.idl" \
                       --type "$type_name" --print-c-name) || return 1
    "$FILL" --vendor rti --idl "$CORPUS/$case_name/type.idl" \
            --type "$type_name" --values "$CORPUS/$case_name/values.json" \
            --out "$work/fill-sample.inc" || return 1
    "$FILL" --vendor rti --idl "$CORPUS/$case_name/type.idl" \
            --type "$type_name" --values "$TOOLS/poison/$case_name.json" \
            --out "$work/fill-poison.inc" || return 1
    sed -e "s/@TYPE@/$cpp_name/g" "$TOOLS/rti-wire-template.c" > "$work/probe.c"
    ( cd "$work" && gcc -o probe probe.c gen.c genPlugin.c genSupport.c \
        -DRTI_UNIX -DRTI_LINUX -DRTI_64BIT -I. -I"$NDDSHOME/include" \
        -I"$NDDSHOME/include/ndds" \
        -L"${RTI_LIB:-$NDDSHOME/lib/${NDDSARCH:?}}" -lnddsc -lnddscore -ldl -lm -lpthread -lrt \
        -Wl,-rpath,"${RTI_LIB:-$NDDSHOME/lib/$NDDSARCH}" ) \
        > "$work/cc.log" 2>&1 || { sed -n 1,12p "$work/cc.log" >&2; return 1; }
}

build_opendds() {
    local case_name=$1 work=$2 type_name=$3
    local home="${OPENDDS_HOME:-/opt/opendds}"
    cp "$CORPUS/$case_name/type.idl" "$work/gen.idl" || return 1
    # `tao_idl -Sa -St` writes the C++ mapping, `opendds_idl` writes the
    # `Serializer` operators *and* `genTypeSupport.idl`, and that file
    # has to go through `tao_idl` in turn for the `<T>DataWriter`
    # interface a real writer narrows to. The in-memory probe needs
    # neither of the last two and compiles two files where this needs
    # four.
    # `--no-default-nested` because OpenDDS 3.x treats a type as nested
    # unless it carries `@topic`, and a nested type gets no
    # `<T>TypeSupport` interface at all — the corpus IDL is annotated
    # for XTypes and not for OpenDDS's topic marker, and without this
    # every case would refuse with a missing symbol rather than with
    # anything about the type.
    ( cd "$work" \
        && DDS_ROOT="${DDS_ROOT:-$home/share/dds}" TAO_ROOT="${TAO_ROOT:-$home}" \
           "$home/bin/tao_idl" -I"$home/include" -Sa -St --idl-version 4 gen.idl \
        && DDS_ROOT="${DDS_ROOT:-$home/share/dds}" TAO_ROOT="${TAO_ROOT:-$home}" \
           "$home/bin/opendds_idl" --no-default-nested -I"$home/include" \
           --idl-version 4 gen.idl \
        && DDS_ROOT="${DDS_ROOT:-$home/share/dds}" TAO_ROOT="${TAO_ROOT:-$home}" \
           "$home/bin/tao_idl" -I"$home/include" -Sa -St --idl-version 4 \
           genTypeSupport.idl ) \
        > "$work/gen.log" 2>&1 || { grep -m5 -i error "$work/gen.log" >&2 || sed -n 1,8p "$work/gen.log" >&2; return 1; }
    local cpp_name
    cpp_name=$("$FILL" --vendor omniorb --idl "$CORPUS/$case_name/type.idl" \
                       --type "$type_name" --print-c-name) || return 1
    "$FILL" --vendor omniorb --idl "$CORPUS/$case_name/type.idl" \
            --type "$type_name" --values "$CORPUS/$case_name/values.json" \
            --out "$work/fill-sample.inc" || return 1
    "$FILL" --vendor omniorb --idl "$CORPUS/$case_name/type.idl" \
            --type "$type_name" --values "$TOOLS/poison/$case_name.json" \
            --out "$work/fill-poison.inc" || return 1
    sed -e "s/@TYPE@/$cpp_name/g" "$TOOLS/opendds-wire-template.cpp" > "$work/probe.cpp"
    ( cd "$work" && g++ -std=c++17 -o probe probe.cpp genC.cpp genTypeSupportC.cpp \
        genTypeSupportImpl.cpp -I. -I"$home/include" -L"$home/lib" \
        -lOpenDDS_Rtps_Udp -lOpenDDS_Rtps -lOpenDDS_Dcps \
        -lTAO_AnyTypeCode -lTAO_Valuetype -lTAO -lACE \
        -Wl,-rpath,"$home/lib" ) \
        > "$work/cc.log" 2>&1 || { grep -m10 "error:" "$work/cc.log" >&2; return 1; }
}

# OpenSplice Community Edition 6.9, SACPP over DDSI2. The IDL goes through
# the same lowering `wireprobe-opensplice` applies (`opensplice-idl.py`:
# the annotations `idlpp` does not read, as the pragmas it does), and the
# fill is the in-memory probe's CORBA C++ one, made from the corpus IDL.
build_opensplice() {
    local case_name=$1 work=$2 type_name=$3
    local home="${OSPL_HOME:-/opt/opensplice-6.9}"
    python3 "$ROOT/tools/wireprobe/opensplice-idl.py" "$CORPUS/$case_name/type.idl" \
        "$work/gen.idl" "$type_name" > "$work/lower.log" 2>&1 \
        || { sed -n 1,8p "$work/lower.log" >&2; return 1; }
    ( cd "$work" && OSPL_HOME="$home" PATH="$home/bin:$PATH" \
        LD_LIBRARY_PATH="$home/lib:${LD_LIBRARY_PATH:-}" "$home/bin/idlpp" -S -l cpp gen.idl ) \
        > "$work/idl.log" 2>&1 \
        || { grep -m5 -i error "$work/idl.log" >&2 || sed -n 1,8p "$work/idl.log" >&2; return 1; }
    local cpp_name
    cpp_name=$("$FILL" --vendor omniorb --idl "$CORPUS/$case_name/type.idl" \
                       --type "$type_name" --print-c-name) || return 1
    "$FILL" --vendor omniorb --idl "$CORPUS/$case_name/type.idl" \
            --type "$type_name" --values "$CORPUS/$case_name/values.json" \
            --out "$work/fill-sample.inc" || return 1
    "$FILL" --vendor omniorb --idl "$CORPUS/$case_name/type.idl" \
            --type "$type_name" --values "$TOOLS/poison/$case_name.json" \
            --out "$work/fill-poison.inc" || return 1
    sed -e "s/@TYPE@/$cpp_name/g" "$TOOLS/opensplice-wire-template.cpp" > "$work/probe.cpp"
    ( cd "$work" && g++ -std=c++11 -o probe probe.cpp gen.cpp genDcps.cpp genDcps_impl.cpp \
        genSplDcps.cpp -I. -I"$home/include" -I"$home/include/sys" \
        -I"$home/include/dcps/C++/SACPP" -L"$home/lib" -ldcpssacpp -lddskernel \
        -Wl,-rpath,"$home/lib" ) \
        > "$work/cc.log" 2>&1 || { grep -m10 "error:" "$work/cc.log" >&2; return 1; }
}

# HDDS: hddsgen 1.2.0's generated type, the one `wireprobe-hdds` measures
# in memory, through the published runtime `hdds` 1.1.2. The two are
# versioned apart, so the build pulls the declaration shim the in-memory
# probe compiles against (as `hdds`) beside the runtime (as `hdds_rt`),
# and `hdds-wire-template.rs` forwards the runtime's encode and decode to
# the generated ones; see that file. One shared target directory, as
# `wireprobe-hdds` keeps one: a cold `hdds` build is the whole cost.
build_hdds() {
    local case_name=$1 work=$2 type_name=$3
    local hddsgen="${HDDSGEN:-/root/hdds-probe/hdds_gen_src/target/release/hddsgen}"
    cp "$CORPUS/$case_name/type.idl" "$work/gen.idl" || return 1
    "$hddsgen" gen rust "$work/gen.idl" > "$work/generated.rs" 2> "$work/gen.log" \
        || { echo "hddsgen refused the file" >&2; head -5 "$work/gen.log" >&2; return 1; }
    # `hddsgen` reports some rejections on stderr and still exits zero.
    grep -q "pub struct\|pub enum" "$work/generated.rs" \
        || { echo "hddsgen produced no type" >&2; head -5 "$work/gen.log" >&2; return 1; }
    local rust_name
    rust_name=$("$FILL" --vendor hdds --idl "$CORPUS/$case_name/type.idl" \
                        --type "$type_name" --print-c-name) || return 1
    "$FILL" --vendor hdds --idl "$CORPUS/$case_name/type.idl" \
            --type "$type_name" --values "$CORPUS/$case_name/values.json" \
            --out "$work/fill-sample.inc" || return 1
    "$FILL" --vendor hdds --idl "$CORPUS/$case_name/type.idl" \
            --type "$type_name" --values "$TOOLS/poison/$case_name.json" \
            --out "$work/fill-poison.inc" || return 1
    sed -e "s/@TYPE@/$rust_name/g" "$TOOLS/hdds-wire-template.rs" > "$work/main.rs"
    # The binary's name is this build's own: two cases sharing the target
    # directory under one name would let one run the other's program.
    local bin="zdw-hdds-$$-$RANDOM"
    cat > "$work/Cargo.toml" <<TOML
[package]
name = "$bin"
version = "0.0.0"
edition = "2021"

[dependencies]
hdds = { path = "$ROOT/tools/wireprobe/hdds-shim" }
hdds_rt = { package = "hdds", version = "${HDDS_VERSION:-1.1.2}" }

[[bin]]
name = "$bin"
path = "main.rs"

[workspace]
TOML
    local target="${WIRE_HDDS_TARGET:-${TMPDIR:-/tmp}/wire-golden-hdds-target}"
    ( cd "$work" && CARGO_TARGET_DIR="$target" cargo build --release --quiet ) \
        > "$work/cc.log" 2>&1 || { grep -m6 -A4 "^error" "$work/cc.log" >&2; return 1; }
    mv "$target/release/$bin" "$work/probe"
}

build_ros() {
    local case_name=$1 work=$2 type_name=$3 rmw=$4
    local package=zdros
    "$ROS_PACKAGE" --idl "$CORPUS/$case_name/type.idl" --type "$type_name" \
                   --out "$work/idl" > "$work/root" 2> "$work/pkg.log" \
        || { sed -n 1,6p "$work/pkg.log" >&2; return 1; }
    local names=()
    mapfile -t names < <("$ROS_PACKAGE" --idl "$CORPUS/$case_name/type.idl" \
                          --type "$type_name" --print-files \
                          | sed -e 's|^msg/||' -e 's|\.idl$||')
    local message_args=()
    local name
    for name in "${names[@]}"; do message_args+=(--message "$name"); done
    "$ROS_PREFIX/bin/python" "$ROOT/tools/wireprobe/rosidl-generate.py" \
        --package "$package" --idl-dir "$work/idl" --out "$work/gen" \
        --prefix "$ROS_PREFIX" "${message_args[@]}" \
        > "$work/names.sh" 2> "$work/gen.log" \
        || { tail -n 8 "$work/gen.log" >&2; return 1; }
    # shellcheck source=/dev/null
    . "$work/names.sh"

    "$FILL" --vendor ros --idl "$CORPUS/$case_name/type.idl" \
            --type "$type_name" --values "$CORPUS/$case_name/values.json" \
            --out "$work/fill-sample.inc" || return 1
    "$FILL" --vendor ros --idl "$CORPUS/$case_name/type.idl" \
            --type "$type_name" --values "$TOOLS/poison/$case_name.json" \
            --out "$work/fill-poison.inc" || return 1

    printf '%s\n' "$ROS_MESSAGE_INCLUDES" > "$work/messages.h"
    sed -e "s/@PACKAGE@/$package/g" -e "s/@MESSAGE@/$ROS_MESSAGE/g" \
        -e "s/@SNAKE@/$ROS_SNAKE/g" -e "s/@CTYPE@/$ROS_CTYPE/g" \
        -e 's|@MESSAGE_INCLUDES@|#include "messages.h"|' \
        "$TOOLS/ros-wire-template.cpp" > "$work/probe.cpp"

    # The env's headers after the system's: a conda-style prefix ships a
    # sysroot whose `stdio.h` does not match the host's libstdc++.
    local includes=(-I"$ROS_PREFIX/include/rosidl_runtime_c"
                    -I"$ROS_PREFIX/include/rosidl_typesupport_interface"
                    -I"$ROS_PREFIX/include/rosidl_typesupport_fastrtps_c"
                    -I"$ROS_PREFIX/include/rosidl_typesupport_fastrtps_cpp"
                    -I"$ROS_PREFIX/include/rosidl_typesupport_introspection_c"
                    -I"$ROS_PREFIX/include/rmw" -I"$ROS_PREFIX/include/rcpputils"
                    -I"$ROS_PREFIX/include/rcutils"
                    -idirafter "$ROS_PREFIX/include")
    local objects=() index=0 relative object
    # Both type supports go into the binary: which one the run uses is
    # the RMW's identifier at run time, and building two binaries for
    # one message would be two chances for them to differ.
    for relative in $ROS_C_SOURCES $ROS_INTROSPECTION_SOURCES; do
        index=$((index + 1)); object="$work/zd_c_$index.o"
        gcc -std=c11 -O1 -c -o "$object" "$work/gen/$relative" \
            -I"$work" -I"$work/gen" "${includes[@]}" >> "$work/cc.log" 2>&1 \
            || { grep -m8 "error:" "$work/cc.log" >&2; return 1; }
        objects+=("$object")
    done
    for relative in $ROS_FASTRTPS_SOURCES; do
        index=$((index + 1)); object="$work/zd_x_$index.o"
        g++ -std=c++17 -O1 -c -o "$object" "$work/gen/$relative" \
            -I"$work" -I"$work/gen" "${includes[@]}" >> "$work/cc.log" 2>&1 \
            || { grep -m8 "error:" "$work/cc.log" >&2; return 1; }
        objects+=("$object")
    done
    g++ -std=c++17 -O1 -o "$work/probe" "$work/probe.cpp" "${objects[@]}" \
        -I"$work" -I"$work/gen" "${includes[@]}" \
        -L"$ROS_PREFIX/lib" "-l${rmw}" -lrmw -lfastcdr \
        -lrosidl_runtime_c -lrosidl_typesupport_fastrtps_c \
        -lrosidl_typesupport_introspection_c -lrcutils \
        -Wl,-rpath,"$ROS_PREFIX/lib" >> "$work/cc.log" 2>&1 \
        || { grep -m10 "error:\|undefined" "$work/cc.log" >&2; return 1; }
}

# The three ORBs. GIOP over TCP, one port per case, and one source for
# omniORB and TAO because both implement the OMG IDL-to-C++ mapping that
# `wireprobe-fill --vendor omniorb` writes against.
#
# The IDL is the corpus type as CORBA IDL — `wireprobe-corba-idl` prints
# it from the model, so a construct with no CORBA form is refused by
# name — plus an interface to carry it. `done()` is how the server knows
# the run is over; a fixed sleep would either cut a run short or pad
# every one of them.

corba_idl() {
    local case_name=$1 work=$2 type_name=$3
    "$ROOT/target/debug/wireprobe-corba-idl" --idl "$CORPUS/$case_name/type.idl" \
        --type "$type_name" --out "$work/gen.idl" || return 1
    {
        printf '\ninterface ZdwSink {\n'
        printf '  void put(in %s v);\n' "$type_name"
        printf '  void done();\n};\n'
    } >> "$work/gen.idl"
}

corba_fill() {
    local case_name=$1 work=$2 type_name=$3 vendor=$4
    "$FILL" --vendor "$vendor" --idl "$CORPUS/$case_name/type.idl" \
            --type "$type_name" --values "$CORPUS/$case_name/values.json" \
            --out "$work/fill-sample.inc" || return 1
    "$FILL" --vendor "$vendor" --idl "$CORPUS/$case_name/type.idl" \
            --type "$type_name" --values "$TOOLS/poison/$case_name.json" \
            --out "$work/fill-poison.inc" || return 1
}

build_omniorb() {
    local case_name=$1 work=$2 type_name=$3
    corba_idl "$case_name" "$work" "$type_name" || return 1
    ( cd "$work" && "${OMNIIDL:-omniidl}" -bcxx gen.idl ) > "$work/idl.log" 2>&1 \
        || { sed -n 1,8p "$work/idl.log" >&2; return 1; }
    corba_fill "$case_name" "$work" "$type_name" omniorb || return 1
    local cpp_name
    cpp_name=$("$FILL" --vendor omniorb --idl "$CORPUS/$case_name/type.idl" \
                       --type "$type_name" --print-c-name) || return 1
    sed -e "s/@TYPE@/${cpp_name//\//\\/}/g" -e "s/@HEADER@/gen.hh/g" \
        "$TOOLS/corba-wire-template.cpp" > "$work/probe.cpp"
    ( cd "$work" && g++ -std=c++17 -o probe probe.cpp genSK.cc -I. \
        -lomniORB4 -lomniDynamic4 -lomnithread ) \
        > "$work/cc.log" 2>&1 || { grep -m10 "error:" "$work/cc.log" >&2; return 1; }
}

build_tao() {
    local case_name=$1 work=$2 type_name=$3
    local home="${OPENDDS_HOME:-/opt/opendds}"
    corba_idl "$case_name" "$work" "$type_name" || return 1
    # `--idl-version 4`, as `build_opendds` already passes to the same
    # binary. Without it `tao_idl` defaults to IDL 3 and refuses an
    # anonymous `sequence<...>` in a member declaration — "anonymous
    # types require the IDL version to be 4 or later" — which is the
    # corpus's own shape and not a limit of TAO. Four cases of the
    # 2026-08-19 sweep recorded that as TAO refusing a type its ORB
    # takes; `refusal-check.py` is what found them.
    ( cd "$work" && "$home/bin/tao_idl" -I"$home/include" --idl-version 4 gen.idl ) \
        > "$work/idl.log" 2>&1 || { grep -m6 -i error "$work/idl.log" >&2; return 1; }
    corba_fill "$case_name" "$work" "$type_name" tao || return 1
    local cpp_name
    cpp_name=$("$FILL" --vendor tao --idl "$CORPUS/$case_name/type.idl" \
                       --type "$type_name" --print-c-name) || return 1
    sed -e "s/@TYPE@/${cpp_name//\//\\/}/g" -e "s/@HEADER@/genS.h/g" \
        "$TOOLS/corba-wire-template.cpp" > "$work/probe.cpp"
    ( cd "$work" && g++ -std=c++17 -o probe probe.cpp genC.cpp genS.cpp \
        -I. -I"$home/include" -L"$home/lib" \
        -lTAO_PortableServer -lTAO_AnyTypeCode -lTAO_Valuetype -lTAO -lACE \
        -Wl,-rpath,"$home/lib" ) \
        > "$work/cc.log" 2>&1 || { grep -m10 "error:" "$work/cc.log" >&2; return 1; }
}

build_jacorb() {
    local case_name=$1 work=$2 type_name=$3
    local home="${JACORB_HOME:-/opt/jacorb}"
    corba_idl "$case_name" "$work" "$type_name" || return 1
    local jars
    jars=$(find "$home/lib" -name '*.jar' | tr '\n' ':')
    ( cd "$work" && java -cp "$jars" org.jacorb.idl.parser -d . gen.idl ) \
        > "$work/idl.log" 2>&1 || { sed -n 1,8p "$work/idl.log" >&2; return 1; }
    corba_fill "$case_name" "$work" "$type_name" jacorb || return 1
    local java_name
    java_name=$("$FILL" --vendor jacorb --idl "$CORPUS/$case_name/type.idl" \
                        --type "$type_name" --print-c-name) || return 1
    sed -e "s/@TYPE@/$java_name/g" \
        -e "/ZDW__FILL__SAMPLE/r $work/fill-sample.inc" -e "/ZDW__FILL__SAMPLE/d" \
        -e "/ZDW__FILL__POISON/r $work/fill-poison.inc" -e "/ZDW__FILL__POISON/d" \
        "$TOOLS/CorbaWire.java" > "$work/CorbaWire.java"
    # JDK 21 dropped `javax.rmi.CORBA`, and JacORB 3.9's
    # `org.jacorb.orb.Reference` extends `javax.rmi.CORBA.Stub`, so the
    # class has to exist for anything to compile. Nothing on the IIOP
    # path calls into it.
    mkdir -p "$work/javax/rmi/CORBA"
    cp "$TOOLS/JavaxRmiCorbaStub.java" "$work/javax/rmi/CORBA/Stub.java"
    # `-encoding UTF-8`: the sources are UTF-8 and `javac` without it
    # reads the platform encoding, which is US-ASCII in a CI container.
    ( cd "$work" && mkdir -p classes && javac -encoding UTF-8 -cp "$jars" -d classes \
        $(find . -name '*.java') ) \
        > "$work/javac.log" 2>&1 || { grep -m10 "error:" "$work/javac.log" >&2; return 1; }
    printf '%s' "$jars" > "$work/jars"
}

# run_giop <case> <domain> <work> <log prefix>
run_giop() {
    local case_name=$1 domain=$2 work=$3 log=$4
    local port=$((12300 + domain))
    local pcap="$work/cap.pcap"
    tshark -i lo -f "tcp port $port" -s 0 -F pcap -w "$pcap" -q \
        > "$work/tshark.log" 2>&1 &
    local capture=$!
    capture_pid=$capture
    for _ in $(seq 1 60); do [ -s "$pcap" ] && break; sleep 0.2; done
    sleep 1

    local server_args=() client_args=()
    case "$impl" in
        omniorb) server_args=(-ORBendPoint "giop:tcp:127.0.0.1:$port")
                 client_args=(-ORBendPoint "giop:tcp:127.0.0.1:0") ;;
        tao)     server_args=(-ORBListenEndpoints "iiop://127.0.0.1:$port") ;;
        jacorb)  server_args=(-DOAPort="$port" -DOAIAddr=127.0.0.1)
                 client_args=(-DOAIAddr=127.0.0.1) ;;
    esac

    rm -f "$work/ior.txt"
    if [ "$impl" = jacorb ]; then
        ( cd "$work" && java -cp "$(cat jars):classes" "${server_args[@]}" \
            CorbaWire server "$count" ) > "$log.sub" 2>&1 &
    else
        ( cd "$work" && ./probe server "$count" "" "${server_args[@]}" ) \
            > "$log.sub" 2>&1 &
    fi
    local server=$!
    for _ in $(seq 1 100); do [ -s "$work/ior.txt" ] && break; sleep 0.2; done
    if [ ! -s "$work/ior.txt" ]; then
        kill $server 2>/dev/null; kill -INT $capture 2>/dev/null
        capture_pid=""
        return 1
    fi
    if [ "$impl" = jacorb ]; then
        ( cd "$work" && java -cp "$(cat jars):classes" "${client_args[@]}" \
            CorbaWire client "$count" "$(cat ior.txt)" ) > "$log.pub" 2>&1
    else
        ( cd "$work" && ./probe client "$count" "$(cat ior.txt)" "${client_args[@]}" ) \
            > "$log.pub" 2>&1
    fi
    wait $server 2>/dev/null
    sleep 1
    kill -INT $capture 2>/dev/null; wait $capture 2>/dev/null
    capture_pid=""
    return 0
}

# --------------------------------------------------------- the transport
#
# Every product is pinned to the loopback and to the RTPS port scheme,
# and both halves are needed.
#
# **The loopback**, because the measurement host has a real network
# interface and the corpus is not something to put on it, and because a
# capture on `lo` then holds the whole exchange.
#
# **The port scheme**, because that is what attributes a packet to a
# case: one domain per case, and DDSI-RTPS 2.5 §9.6.1.1 makes the ports
# a function of the domain id. Cyclone does not use it by default —
# with `ParticipantIndex` unset it binds an ephemeral port and
# advertises it in SPDP, and a capture filtered on the domain's ports
# is then empty. An empty capture is a false green of the loudest kind
# available here, which is why `wire-golden.sh` sets the index rather
# than widening the filter until something appears.

transport_env() {
    local work=$1 domain=$2
    case "$impl" in
        cyclone|ros-cyclone)
            cat > "$work/cyclone.xml" <<'XML'
<CycloneDDS><Domain id="any">
 <General><Interfaces><NetworkInterface name="lo" multicast="true"/></Interfaces></General>
 <Discovery><ParticipantIndex>auto</ParticipantIndex><Peers><Peer address="127.0.0.1"/></Peers></Discovery>
</Domain></CycloneDDS>
XML
            export CYCLONEDDS_URI="file://$work/cyclone.xml" ;;
        rti)
            export NDDS_DISCOVERY_PEERS="udpv4://127.0.0.1"
            export LD_LIBRARY_PATH="${NDDSHOME:?}/lib/${NDDSARCH:?}:${LD_LIBRARY_PATH:-}" ;;
        fastdds)
            export LD_LIBRARY_PATH="${FASTDDS_HOME:-/opt/fastdds}/lib:${LD_LIBRARY_PATH:-}" ;;
        ros-fastrtps)
            # The probe cannot set this one in code: `rmw_fastrtps_cpp`
            # builds the participant and the writer, not the caller. With
            # the built-in transports and data sharing at their defaults
            # two processes on one host exchange the payload through
            # shared memory and the capture holds no user DATA at all —
            # the publisher reports six writes, the subscriber reports
            # six reads, and nothing measured anything.
            cat > "$work/fastdds.xml" <<'XML'
<?xml version="1.0" encoding="UTF-8" ?>
<dds xmlns="http://www.eprosima.com/XMLSchemas/fastRTPS_Profiles">
  <profiles>
    <transport_descriptors>
      <transport_descriptor>
        <transport_id>zdw_udp_lo</transport_id>
        <type>UDPv4</type>
        <interfaceWhiteList><address>127.0.0.1</address></interfaceWhiteList>
      </transport_descriptor>
    </transport_descriptors>
    <participant profile_name="zdw_participant" is_default_profile="true">
      <rtps>
        <userTransports><transport_id>zdw_udp_lo</transport_id></userTransports>
        <useBuiltinTransports>false</useBuiltinTransports>
      </rtps>
    </participant>
    <data_writer profile_name="zdw_writer" is_default_profile="true">
      <qos>
        <data_sharing><kind>OFF</kind></data_sharing>
        <reliability><kind>RELIABLE</kind></reliability>
      </qos>
      <topic><historyQos><kind>KEEP_ALL</kind></historyQos></topic>
      <historyMemoryPolicy>PREALLOCATED_WITH_REALLOC</historyMemoryPolicy>
    </data_writer>
    <data_reader profile_name="zdw_reader" is_default_profile="true">
      <qos>
        <data_sharing><kind>OFF</kind></data_sharing>
        <reliability><kind>RELIABLE</kind></reliability>
      </qos>
      <topic><historyQos><kind>KEEP_ALL</kind></historyQos></topic>
      <historyMemoryPolicy>PREALLOCATED_WITH_REALLOC</historyMemoryPolicy>
    </data_reader>
  </profiles>
</dds>
XML
            export FASTRTPS_DEFAULT_PROFILES_FILE="$work/fastdds.xml"
            export RMW_FASTRTPS_USE_QOS_FROM_XML=1
            export LD_LIBRARY_PATH="${ROS_PREFIX}/lib:${LD_LIBRARY_PATH:-}" ;;
        opendds)
            # Two settings here, one file per role, and each was a
            # measurement rather than a preference.
            #
            # **`use_multicast=0`.** `rtps_udp` has it on by default, so
            # an OpenDDS reader advertises a multicast locator —
            # 239.255.0.2 — and the writer sends the user `DATA` there.
            # With the transport bound to 127.0.0.1 that datagram does
            # not reach the reader on this host, and the failure is the
            # quietest this harness has: the publisher writes six samples
            # and reports a matched reader, the capture holds all six,
            # and the subscriber reads nothing. `struct-final` XCDR2,
            # 2026-08-19, the same binary with and without the line:
            # `read 0` against `read 6`.
            #
            # **The pinned ports.** Unicast RTPS out of
            # `local_address=127.0.0.1:0` leaves on an ephemeral port,
            # and `extract-rtps.py` attributes a packet by mapping its
            # destination port back to a domain (§9.6.1.1). An ephemeral
            # port maps to no domain, so with multicast off and ports
            # unpinned every OpenDDS run reported `no user DATA in the
            # capture` while its subscriber read all six. Under
            # multicast the data went to the domain's multicast port and
            # was attributable, which is why the first sweep of this
            # harness never met either half.
            #
            # The offsets are inside the domain's 250-port block and
            # clear of §9.6.1.1's own assignments, and the two roles take
            # different ones because they are two processes on one host.
            # Neither setting moves an octet of the `SerializedPayload`:
            # they decide which socket it leaves from.
            local base=$((7400 + 250 * domain)) role
            for role in sub pub; do
                local slot=$((base + 100)); [ "$role" = pub ] && slot=$((base + 110))
                cat > "$work/opendds-$role.ini" <<INI
[common]
DCPSDefaultDiscovery=DEFAULT_RTPS
DCPSGlobalTransportConfig=\$file

[transport/the_rtps]
transport_type=rtps_udp
local_address=127.0.0.1:$slot
use_multicast=0

[rtps_discovery/DEFAULT_RTPS]
SedpLocalAddress=127.0.0.1:$((slot + 2))
SpdpLocalAddress=127.0.0.1:$((slot + 4))
InteropMulticastOverride=239.255.0.1
INI
            done
            export ZD_OPENDDS_SUB="-DCPSConfigFile $work/opendds-sub.ini"
            export ZD_OPENDDS_PUB="-DCPSConfigFile $work/opendds-pub.ini"
            export LD_LIBRARY_PATH="${OPENDDS_HOME:-/opt/opendds}/lib:${LD_LIBRARY_PATH:-}" ;;
        opensplice)
            # The single-process deployment with the `ddsi2` service and
            # nothing else: no durability service, whose alignment
            # traffic would share the capture, and no shared memory, so
            # the two processes meet on the socket. The domain id is the
            # case's, because OpenSplice takes the participant's domain
            # from the configuration it names.
            #
            # `ParticipantIndex` for the reason Cyclone's has it above
            # (DDSI2 is Cyclone's ancestor and binds ephemeral unicast
            # ports without it), and unicast discovery to the loopback
            # because `lo` carries no multicast.
            local home="${OSPL_HOME:-/opt/opensplice-6.9}"
            cat > "$work/ospl.xml" <<XML
<OpenSplice>
  <Domain>
    <Name>zdw</Name>
    <Id>$domain</Id>
    <SingleProcess>true</SingleProcess>
    <Service name="ddsi2"><Command>ddsi2</Command></Service>
  </Domain>
  <DDSI2Service name="ddsi2">
    <General>
      <NetworkInterfaceAddress>127.0.0.1</NetworkInterfaceAddress>
      <AllowMulticast>false</AllowMulticast>
    </General>
    <Discovery>
      <ParticipantIndex>auto</ParticipantIndex>
      <Peers><Peer Address="127.0.0.1"/></Peers>
    </Discovery>
    <Compatibility><StandardsConformance>lax</StandardsConformance></Compatibility>
  </DDSI2Service>
</OpenSplice>
XML
            export OSPL_HOME="$home"
            export OSPL_URI="file://$work/ospl.xml"
            # `ospl-error.log` and `ospl-info.log` go to the run's own
            # directory, not to the caller's working directory.
            export OSPL_LOGPATH="$work"
            export PATH="$home/bin:$PATH"
            export LD_LIBRARY_PATH="$home/lib:${LD_LIBRARY_PATH:-}" ;;
        hdds)
            # Every locator on the loopback, and discovery by unicast to
            # the metatraffic ports of the two participant ids the probe
            # pins (DDSI-RTPS 2.5 §9.6.1.1: 7400 + 250 d + 10 + 2 p),
            # because `lo` carries no multicast.
            local base=$((7400 + 250 * domain))
            export HDDS_INTERFACE=127.0.0.1
            export HDDS_UNICAST_IF=127.0.0.1
            export HDDS_MULTICAST_IF=127.0.0.1
            export HDDS_SPDP_UNICAST_PEERS="127.0.0.1:$((base + 10)),127.0.0.1:$((base + 12))" ;;
    esac
}

# ------------------------------------------------------------- the runs

port_low()  { echo $((7400 + 250 * $1)); }
port_high() { echo $((7400 + 250 * $1 + 249)); }

# run_dds <case> <domain> <version> <work> <log prefix>
#
# Capture, subscriber, publisher, capture off. Returns the extracted
# rows on stdout; the checks are the caller's.
run_dds() {
    local case_name=$1 domain=$2 version=$3 work=$4 log=$5
    # ROS topic names take no hyphen, and a corpus case name is full of
    # them. The same substitution for every implementation, so the topic
    # is one thing across the runs and not one thing per product.
    local topic="zdw_${case_name//-/_}"
    local pcap="$work/cap.pcap"
    local low high
    low=$(port_low "$domain"); high=$(port_high "$domain")
    # The domain's ports **or** anything whose UDP payload starts with
    # `RTPS`. The port half scopes the capture to this case; the magic
    # half is there because a product that does not use DDSI-RTPS
    # §9.6.1.1's port scheme would otherwise leave an empty capture, and
    # an empty capture is the quietest false green available here.
    # Cyclone with `ParticipantIndex` unset is exactly that product, and
    # it cost this harness a run before the setting went in.
    # `crates/ros2-rmw/interop/run_capture.sh` had the magic-number
    # filter from the start; this is that idea, kept.
    #
    # `ZD_CAPTURE_FILTER` overrides both, for the diagnostic case where
    # the question is what the product sent at all.
    tshark -i lo -f "${ZD_CAPTURE_FILTER:-(udp portrange $low-$high) or (udp[8:4] = 0x52545053)}" \
        -s 0 -F pcap -w "$pcap" -q \
        > "$work/tshark.log" 2>&1 &
    local capture=$!
    capture_pid=$capture
    # tshark reports "Capturing on" only once the socket is up; polling
    # the file's existence is the same wait without parsing its prose.
    for _ in $(seq 1 60); do [ -s "$pcap" ] && break; sleep 0.2; done
    sleep 1

    # Under a timeout, both of them. A run that hangs is a run that
    # never produces a row, and one hung process stops a 46-case sweep
    # for as long as nobody is watching it. The bound is well past the
    # 15 s the publisher waits for a match plus the 10 s it waits for
    # acknowledgments.
    timeout -k 5 ${ZD_RUN_TIMEOUT:-90} "$work/probe" ${ZD_OPENDDS_SUB:-} sub "$domain" "$version" \
        "$count" "$topic" > "$log.sub" 2>&1 &
    local sub=$!
    sleep 1
    timeout -k 5 ${ZD_RUN_TIMEOUT:-90} "$work/probe" ${ZD_OPENDDS_PUB:-} pub "$domain" "$version" \
        "$count" "$topic" > "$log.pub" 2>&1
    local pub_status=$?
    wait $sub; local sub_status=$?
    sleep 1
    kill -INT $capture 2>/dev/null; wait $capture 2>/dev/null
    capture_pid=""

    echo "$pub_status $sub_status"
}

# ------------------------------------------------------------- the loop

python_check="$TOOLS/check-run.py"

while IFS=$'\t' read -r case_name domain control inventory_rows why; do
    [ -n "$case_name" ] || continue
    type_name=$(cat "$CORPUS/$case_name/type" 2>/dev/null || echo Sample)
    domain=$((domain + domain_offset))
    case "$impl" in
        omniorb|tao|jacorb) versions=1 ;;   # CORBA CDR is XCDR1 and nothing else
        # An RMW has no data-representation argument: `rmw_publish` takes
        # a message and nothing else, and both type supports write
        # encoding version 1. Asking for XCDR2 would produce a second
        # identical row under a name nothing measured.
        ros-fastrtps|ros-cyclone) versions=1 ;;
        # OpenSplice 6.9 has one representation, XCDR1; its probe refuses
        # a request for 2 rather than writing 1 under that name.
        opensplice) versions=1 ;;
        # The published HDDS runtime (1.1.2) encodes through `encode_cdr2`
        # alone; its probe refuses a request for 1.
        hdds) versions=2 ;;
        *) versions=${encodings//,/ } ;;
    esac
    for version in $versions; do
        label="$case_name.xcdr$version"
        work=$(mktemp -d)
        current_work=$work
        log="$out/logs/$impl/$label"
        build_status=0
        case "$impl" in
            cyclone)  build_cyclone "$case_name" "$work" "$type_name" 2> "$log.build" || build_status=1 ;;
            fastdds)  build_fastdds "$case_name" "$work" "$type_name" 2> "$log.build" || build_status=1 ;;
            rti)      build_rti "$case_name" "$work" "$type_name" 2> "$log.build" || build_status=1 ;;
            opendds)  build_opendds "$case_name" "$work" "$type_name" 2> "$log.build" || build_status=1 ;;
            opensplice) build_opensplice "$case_name" "$work" "$type_name" 2> "$log.build" || build_status=1 ;;
            hdds)     build_hdds "$case_name" "$work" "$type_name" 2> "$log.build" || build_status=1 ;;
            ros-fastrtps) build_ros "$case_name" "$work" "$type_name" rmw_fastrtps_cpp 2> "$log.build" || build_status=1 ;;
            ros-cyclone)  build_ros "$case_name" "$work" "$type_name" rmw_cyclonedds_cpp 2> "$log.build" || build_status=1 ;;
            omniorb)  build_omniorb "$case_name" "$work" "$type_name" 2> "$log.build" || build_status=1 ;;
            tao)      build_tao "$case_name" "$work" "$type_name" 2> "$log.build" || build_status=1 ;;
            jacorb)   build_jacorb "$case_name" "$work" "$type_name" 2> "$log.build" || build_status=1 ;;
            *) echo "wire-golden: --impl $impl has no builder here" >&2; exit 2 ;;
        esac
        if [ $build_status -ne 0 ]; then
            printf '%s\t%s\txcdr%s\t!refused\t%s\n' "$impl" "$case_name" "$version" \
                "$(head -c 300 "$log.build" | tr '\n\t' '  ')" >> "$rows"
            rm -rf "$work"; continue
        fi

        transport_env "$work" "$domain"
        frames="$out/frames/$impl/$label.frames"
        case "$impl" in
            omniorb|tao|jacorb)
                if ! run_giop "$case_name" "$domain" "$work" "$log"; then
                    printf '%s\t%s\txcdr%s\t!no-server\tthe ORB wrote no IOR\n' \
                        "$impl" "$case_name" "$version" >> "$rows"
                    rm -rf "$work"; continue
                fi
                extractor=(python3 "$TOOLS/extract-giop.py" "$work/cap.pcap"
                           --operation put --frames "$frames")
                checker=(--giop) ;;
            *)
                run_dds "$case_name" "$domain" "$version" "$work" "$log" > /dev/null
                extractor=(python3 "$TOOLS/extract-rtps.py" "$work/cap.pcap"
                           --domain "$domain" --frames "$frames")
                checker=(--topic "zdw_${case_name//-/_}") ;;
        esac
        # A publisher that died says so in its own words, and the row
        # says what it said. Without this the run reaches the extractor,
        # finds no writer, and reports "SEDP never named the topic" —
        # true, and three steps downstream of the reason.
        if ! grep -q "^matched " "$log.pub" 2>/dev/null; then
            printf '%s\t%s\txcdr%s\t!publisher\t%s\n' "$impl" "$case_name" \
                "$version" "$(tail -n 3 "$log.pub" 2>/dev/null | tr '\n\t' '  ' \
                              | head -c 300)" >> "$rows"
            rm -rf "$work"; continue
        fi
        if ! "${extractor[@]}" > "$work/rows.tsv" 2> "$log.extract"; then
            printf '%s\t%s\txcdr%s\t!nothing-captured\t%s\n' "$impl" "$case_name" \
                "$version" "$(head -c 200 "$log.extract" | tr '\n\t' '  ')" >> "$rows"
            [ $keep_pcap -eq 1 ] && cp "$work/cap.pcap" "$out/frames/$impl/$label.pcap"
            rm -rf "$work"; continue
        fi
        [ $keep_pcap -eq 1 ] && cp "$work/cap.pcap" "$out/frames/$impl/$label.pcap"
        python3 "$python_check" --rows "$work/rows.tsv" --count "$count" \
            --impl "$impl" --case "$case_name" --version "$version" \
            --control "$control" --sub-log "$log.sub" \
            "${checker[@]}" >> "$rows" 2>> "$log.check"
        # The three numbers that are properties of the run and of nothing
        # in the capture: a writer nobody was listening to leaves exactly
        # the octets a matched one does. They are read out of the logs
        # here, while the logs exist — `run-log.tsv` is where they have
        # to survive, because the logs stay on the measurement host.
        if [ "$(tail -n 1 "$rows" | cut -f4)" = ok ]; then
            printf '%s\t%s\txcdr%s\t%s\t%s\t%s\n' "$impl" "$case_name" "$version" \
                "$(grep -c '^wrote ' "$log.pub" 2>/dev/null || echo 0)" \
                "$(awk '/^matched /{print $2; exit}' "$log.pub" 2>/dev/null)" \
                "$(awk '/^read /{print $2; exit}' "$log.sub" 2>/dev/null)" \
                >> "$run_log"
        fi
        rm -rf "$work"
    done
done < <(selected)

# The rows the retry did not touch, back where they were. Written after
# the loop and not before it so that a retry that dies half way leaves
# `.previous` intact rather than a half-merged table.
if [ $retry -eq 1 ]; then
    awk -F'\t' -v wanted=",$retry_cases," \
        'index(wanted, "," $2 ",") == 0' "$rows.previous" >> "$rows"
    if [ -f "$run_log.previous" ]; then
        awk -F'\t' -v wanted=",$retry_cases," \
            'index(wanted, "," $2 ",") == 0' "$run_log.previous" >> "$run_log"
    fi
fi

echo "wire-golden: $impl — $(wc -l < "$rows") rows in $rows" >&2
