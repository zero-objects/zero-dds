#!/bin/bash
# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 ZeroDDS Contributors
#
# ZeroDDS <-> foreign DDS, one shape topic, both directions, four stacks.
#
#   ./tools/dcps-shapes-interop.sh            # every case
#   ./tools/dcps-shapes-interop.sh B D        # named cases only
#
# Build the two probes first:
#   cargo build -p zerodds-dcps --example omg_shapes --example classic_shapes
#
# WHAT IT PROVES, AND WHAT IT DOES NOT
#
# Each case starts one ZeroDDS process and one foreign process, waits, and
# counts the samples the *receiving* side printed. A case passes only on a
# non-zero count from the receiver — an exit status is not evidence, and
# neither is a process that started. Cases V1..V4 run a foreign stack against
# itself with no ZeroDDS code in the loop at all; run those first, because a
# zero there means the vendor side is broken and every other number in this
# file is meaningless.
#
# TWO TYPES, BOTH CALLED ShapeType
#
#   A..D, P*  omg_shapes      — OMG dds-rtps shape.idl: five members,
#                               @appendable, DHEADER, encapsulation 0x0009.
#                               The vendors' shape_main release binaries.
#   E..H      classic_shapes  — four members, @final, no DHEADER. The RTI
#                               ShapeType_publisher/subscriber examples, the
#                               OpenDDS shapes example, and
#                               zerodds-examples/cross-vendor-shapes.
#
# They are not wire-compatible; pairing one against the other is a type
# error that shows up as a decode failure, not as a discovery failure.
#
# THREE CONFIGURATION TRAPS, EACH OF WHICH LOOKS LIKE AN INTEROP FAILURE
#
#   1. RTI without NDDSHOME set refuses to create a DomainParticipant ("No
#      source for License information") and prints nothing else. The pair
#      then reports zero samples for a run in which RTI never existed.
#   2. The OpenDDS binaries on the lab host are linked against the
#      security-enabled tree; an LD_LIBRARY_PATH naming the other one
#      overrides their RPATH and they die on a missing get_crypto_handle
#      symbol. tools/interop-env.sh handles this; do not undo it.
#   3. The vendor apps block-buffer stdout into a pipe, so a run that is
#      killed at the end of its window loses every line it produced. Every
#      process below is started under `stdbuf -oL`.
#
# THE REPRESENTATION MATRIX — cases M1..M16
#
# Four stacks, both directions, both DataRepresentations: sixteen runs in
# which the representation the samples travel in is set by configuration on
# whichever side owns it.
#
#   ZeroDDS writes    ZERODDS_DATA_REPR_OFFER=XCDR1|XCDR2 on our process. The
#                     first element of the offer is what the body is
#                     serialized as and what the encapsulation header
#                     declares, so the peer's acceptance is the evidence.
#   Cyclone writes    the client's own `-x 1|2`.
#   Fast DDS writes   the client's own `-x 1|2`.
#   RTI writes        an own USER_QOS_PROFILES.xml, written to $OUT by
#                     rti_profiles() and picked up because the run's cwd is
#                     $OUT. The vendor's tree is not touched.
#   OpenDDS writes    NOT settable from outside the application, and the two
#                     runs do not separate it. The demo hardcodes domain 0 and
#                     its ShapeType.idl carries no representation annotation;
#                     M15 and M16 vary OUR READER's accept list instead.
#
# What M15 and M16 do and do not establish. Both pass, and both carry the same
# octets: measured on a capture of the pairing,
# `tools/wire-capture/encap-histogram.py` reports 194 user samples at
# PLAIN_CDR2_LE (0x0007) and nothing else, so the OpenDDS demo writer emits
# XCDR2. What the pair shows is that our reader matches and decodes it under
# either accept list — it does not show OpenDDS writing XCDR1, and no
# configuration reachable from outside that application makes it.
#
# Read a sample count as "the receiving side got samples", never as "in the
# representation the case is named after". For the writing side that is the
# same thing, because the writer serializes from its own first offered
# element; for the reading side the encapsulation header decides, which is why
# the OpenDDS pair needed the capture.
#
# HISTORY — why the matrix exists
#
# ZeroDDS's DataWriter used to offer DataRepresentation [XCDR2] only. The
# Cyclone and Fast DDS shape_main clients request the DDS default [XCDR1],
# and §7.6.3.1.2 matches the writer's FIRST offered element against the
# reader's list, so they refused with DATAREPRESENTATION incompatible QoS —
# cases A and C failed while B and D passed. The default offer is now
# [XCDR1, XCDR2] and the matrix below is what holds it in place.

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO="$(cd "$HERE/.." && pwd)"
# The vendors' own setenv scripts append to LD_LIBRARY_PATH without
# defining it first, so it has to exist before they are sourced. `set -u`
# is deliberately not on here for the same reason: it turns a vendor
# script's unset reference into an abort of this whole harness.
: "${LD_LIBRARY_PATH:=}"
export LD_LIBRARY_PATH
# shellcheck disable=SC1091
. "$HERE/interop-env.sh"

OUT="${ZD_INTEROP_OUT:-/root/zd-interop-out}"
mkdir -p "$OUT"
STAMP=$(date +%Y%m%d-%H%M%S)

ZD="${ZD_EXAMPLES:-$REPO/target/debug/examples}"
CYC="${ZD_CYCLONE_SHAPE_MAIN:-/root/omg-vendors/cyclone/eclipse_cyclone-11.0.1_shape_main_linux}"
FD="${ZD_FASTDDS_SHAPE_MAIN:-/root/omg-vendors/fastdds/eprosima_fastdds-3.6.1_shape_main_linux}"
RTI="${ZD_RTI_DEMO:-/root/xv-demos/rti/objs/x64Linux4gcc8.5.0}"
RTIDIR="${ZD_RTI_DIR:-/root/xv-demos/rti}"
OD="${ZD_OPENDDS_DEMO:-/root/xv-demos/opendds}"
RTIB="${NDDSHOME:-}/lib/${NDDSARCH:-}"

# Empty ACKNACK of a reader that holds nothing: Fast DDS (and, as recorded, OpenDDS
# and RTI Connext) send it with bitmapBase 0 and numBits 0, which DDSI-RTPS 2.5
# section 8.3.5.5 makes invalid; ZeroDDS by default drops the rest of such a
# datagram (section 8.3.4.1 rule 6), and with it the SEDP data that follows in
# it (measured: sec-diag16/17, every red zerodds->fastdds cell had rejections
# with left_out > 0, every green cell none). The ZeroDDS processes of the
# vendor pairs, writers and readers, therefore accept it
# (wire.accept_empty_acknack_base_zero). This is the only place the runner sets it.
export ZERODDS_WIRE_ACCEPT_EMPTY_ACKNACK_BASE_ZERO=1

# PL_CDR1 parameters with the rounded length. Cyclone DDS (extended header) and
# RTI Connext (short header) write the length of a PL_CDR1 parameter rounded up
# to four, the form of DDSI-RTPS 2.5 section 9.4.2.11; XTypes 1.3 section
# 7.4.1.2.2 has the exact length and the default reader of ZeroDDS refuses the
# other form. The vendors whose writers use the rounded form (taken from the
# recorded rows: ast-layout Dialect::Cyclone and Dialect::Rti,
# pads_declared_length) get a ZeroDDS reader with wire.pl_cdr1_reading=tolerant;
# Fast DDS and OpenDDS write the exact length. This is the only place the runner
# sets it.
# Set on the ZeroDDS reader of the pairs with a Cyclone or an RTI writer (B, F, M3, M4,
# M11, M12).
ZD_ROUNDED_PL="ZERODDS_WIRE_PL_CDR1_READING=tolerant"

# pair <label> <first-cmd> <second-cmd> <window-s> <count-regex> <count-side>
# Captures are evidence: each run writes new files stamped with its start
# time and nothing here removes or truncates an earlier capture.
pair() {
  local L="$1" A="$2" B="$3" W="$4" RE="$5" SIDE="$6"
  local FA="$OUT/$L-$STAMP.a" FB="$OUT/$L-$STAMP.b"
  setsid stdbuf -oL -eL bash -c "$A" > "$FA" 2>&1 < /dev/null &
  local PA=$!
  sleep 4
  setsid stdbuf -oL -eL bash -c "$B" > "$FB" 2>&1 < /dev/null &
  local PB=$!
  sleep "$W"
  kill -TERM -"$PA" -"$PB" 2>/dev/null; sleep 2
  kill -KILL -"$PA" -"$PB" 2>/dev/null
  local F="$FA"; [ "$SIDE" = b ] && F="$FB"
  local n; n=$(grep -cE "$RE" "$F")
  local verdict=FAIL; [ "$n" -gt 0 ] && verdict=PASS
  printf '%-6s %-4s samples=%-5s %s\n' "$L" "$verdict" "$n" "$F"
}

# pair_none — same arguments as `pair`, inverted verdict: the case passes on a
# count of ZERO. For a pairing that must not match at all, where a non-zero
# count is the defect. Without this the run would print FAIL for a correct
# outcome, and a FAIL nobody can act on is worse than no line.
pair_none() {
  local L="$1"
  local out; out=$(pair "$@")
  local n; n=$(sed -E 's/.*samples=([0-9]+).*/\1/' <<<"$out")
  local file; file=${out##* }
  local verdict=FAIL; [ "$n" -eq 0 ] && verdict=PASS
  printf '%-6s %-4s samples=%-5s (expected 0) %s\n' "$L" "$verdict" "$n" "$file"
}

# rti_profiles <XCDR|XCDR2> — writes an own USER_QOS_PROFILES.xml into $OUT and
# echoes its directory. The RTI demo loads the file from its working directory
# and asks the QoS provider for `ShapeType_Library::Periodic_Best_Effort_Profile`
# (ShapeType_publisher.cxx) resp. `..::Aperiodic_Reliable_Profile`, so a copy of
# the vendor file with a `<representation>` added to `datawriter_qos` decides
# what its writer offers. The vendor's own copy is read, never written.
rti_profiles() {
  local WANT="$1"
  # Two statements, not `local WANT=.. DIR=..$WANT`: bash expands every word of
  # a `local` command before assigning any of them, so DIR would take $WANT
  # from the enclosing scope — empty — and both representations would land in
  # one directory named after neither.
  local DIR="$OUT/rti-qos-$WANT"
  mkdir -p "$DIR"
  # `_DATA_REPRESENTATION` is RTI's element spelling; XCDR is XCDR1.
  python3 - "$RTIDIR/USER_QOS_PROFILES.xml" "$DIR/USER_QOS_PROFILES.xml" "$WANT" <<'PY'
import re, sys
src, dst, want = sys.argv[1], sys.argv[2], sys.argv[3]
xml = open(src, encoding="utf-8").read()
block = (
    "<datawriter_qos>\n"
    "                <representation>\n"
    "                    <value>\n"
    f"                        <element>{want}_DATA_REPRESENTATION</element>\n"
    "                    </value>\n"
    "                </representation>"
)
# Every datawriter_qos in the file, so whichever profile the demo picks carries
# the representation. A count of zero would mean the vendor file changed shape.
xml, n = re.subn(r"<datawriter_qos>", block, xml)
if n == 0:
    sys.exit("no <datawriter_qos> in " + src)
open(dst, "w", encoding="utf-8").write(xml)
PY
  echo "$DIR"
}

run_case() {
case "$1" in
# --- foreign stack against itself: no ZeroDDS code in the loop -------------
V1) pair V1 "$CYC -P -d 60 -t Square -c BLUE -r -w --write-period 100" \
            "$FD -S -d 60 -t Square -r" 25 '^Square' b ;;
V2) pair V2 "$CYC -P -d 61 -t Square -c BLUE -r -w --write-period 100" \
            "$CYC -S -d 61 -t Square -r" 25 '^Square' b ;;
V3) pair V3 "cd $RTIDIR && LD_LIBRARY_PATH=$RTIB $RTI/ShapeType_publisher -d 62" \
            "cd $RTIDIR && LD_LIBRARY_PATH=$RTIB $RTI/ShapeType_subscriber -d 62" 25 '^\[color:' b ;;
V4) pair V4 "cd $OD && ./build-sec/opendds_pub -DCPSConfigFile rtps.ini" \
            "cd $OD && ./build-sec/opendds_sub -DCPSConfigFile rtps.ini" 20 'RECEIVED' b ;;
# --- ZeroDDS <-> foreign, OMG five-member @appendable ShapeType ------------
A)  pair A  "ZERODDS_RUN_SECS=45 $ZD/omg_shapes pub Square 41 BLUE" \
            "$CYC -S -d 41 -t Square -r" 40 '^Square' b ;;
B)  pair B  "$CYC -P -d 42 -t Square -c BLUE -r -w --write-period 100" \
            "$ZD_ROUNDED_PL ZERODDS_RUN_SECS=35 $ZD/omg_shapes sub Square 42" 40 '^  <- color' b ;;
C)  pair C  "ZERODDS_RUN_SECS=45 $ZD/omg_shapes pub Square 43 BLUE" \
            "$FD -S -d 43 -t Square -r" 40 '^Square' b ;;
D)  pair D  "$FD -P -d 44 -t Square -c BLUE -r -w --write-period 100" \
            "ZERODDS_RUN_SECS=35 $ZD/omg_shapes sub Square 44" 40 '^  <- color' b ;;
# --- ZeroDDS <-> foreign, classic four-member @final ShapeType -------------
E)  pair E  "ZERODDS_RUN_SECS=45 $ZD/classic_shapes pub Square 51 GREEN" \
            "cd $RTIDIR && LD_LIBRARY_PATH=$RTIB $RTI/ShapeType_subscriber -d 51" 40 '^\[color:' b ;;
F)  pair F  "cd $RTIDIR && LD_LIBRARY_PATH=$RTIB $RTI/ShapeType_publisher -d 52 -s 400" \
            "$ZD_ROUNDED_PL ZERODDS_RUN_SECS=35 $ZD/classic_shapes sub Square 52" 40 '^  <- color' b ;;
G)  pair G  "ZERODDS_RUN_SECS=45 $ZD/classic_shapes pub Square 0 GREEN" \
            "cd $OD && ./build-sec/opendds_sub -DCPSConfigFile rtps.ini" 30 'RECEIVED' b ;;
H)  pair H  "cd $OD && ./build-sec/opendds_pub -DCPSConfigFile rtps.ini" \
            "ZERODDS_RUN_SECS=35 $ZD/classic_shapes sub Square 0" 40 '^  <- color' b ;;
# --- the one pairing that must NOT carry samples ---------------------------
# The writer serializes one body, from the first element of its offer. Against
# a reader that accepts only the other representation there is nothing it could
# send, so the correct outcome is no match and no samples — reported by the
# peer as DATAREPRESENTATION incompatible QoS. M2 is this same pair with
# ZERODDS_DATA_REPR_OFFER=XCDR2, and it carries samples.
#
# P1..P5 were the diagnostic probes that led to the matrix. M1..M8 run both of
# their axes against both clients, so they are gone rather than kept as a
# second spelling of the same runs.
N1)  pair_none N1 "ZERODDS_RUN_SECS=35 $ZD/omg_shapes pub Square 54 BLUE" \
                  "$CYC -S -d 54 -t Square -r -x 2" 30 '^Square' b ;;

# --- M1..M16: four stacks x both directions x both representations ---------
# Cyclone, five-member @appendable ShapeType. `-x 1|2` is the client's own
# representation on either side, so its acceptance resp. its output is the
# evidence for which one the samples travelled in.
M1)  pair M1  "ZERODDS_DATA_REPR_OFFER=XCDR1 ZERODDS_RUN_SECS=35 $ZD/omg_shapes pub Square 101 BLUE" \
              "$CYC -S -d 101 -t Square -r -x 1" 30 '^Square' b ;;
M2)  pair M2  "ZERODDS_DATA_REPR_OFFER=XCDR2 ZERODDS_RUN_SECS=35 $ZD/omg_shapes pub Square 102 BLUE" \
              "$CYC -S -d 102 -t Square -r -x 2" 30 '^Square' b ;;
M3)  pair M3  "$CYC -P -d 103 -t Square -c BLUE -r -w -x 1 --write-period 100" \
              "$ZD_ROUNDED_PL ZERODDS_DATA_REPR_OFFER=XCDR1 ZERODDS_RUN_SECS=35 $ZD/omg_shapes sub Square 103" 30 '^  <- color' b ;;
M4)  pair M4  "$CYC -P -d 104 -t Square -c BLUE -r -w -x 2 --write-period 100" \
              "$ZD_ROUNDED_PL ZERODDS_DATA_REPR_OFFER=XCDR2 ZERODDS_RUN_SECS=35 $ZD/omg_shapes sub Square 104" 30 '^  <- color' b ;;
# Fast DDS, same type and the same two flags.
M5)  pair M5  "ZERODDS_DATA_REPR_OFFER=XCDR1 ZERODDS_RUN_SECS=35 $ZD/omg_shapes pub Square 105 BLUE" \
              "$FD -S -d 105 -t Square -r -x 1" 30 '^Square' b ;;
M6)  pair M6  "ZERODDS_DATA_REPR_OFFER=XCDR2 ZERODDS_RUN_SECS=35 $ZD/omg_shapes pub Square 106 BLUE" \
              "$FD -S -d 106 -t Square -r -x 2" 30 '^Square' b ;;
M7)  pair M7  "$FD -P -d 107 -t Square -c BLUE -r -w -x 1 --write-period 100" \
              "ZERODDS_DATA_REPR_OFFER=XCDR1 ZERODDS_RUN_SECS=35 $ZD/omg_shapes sub Square 107" 30 '^  <- color' b ;;
M8)  pair M8  "$FD -P -d 108 -t Square -c BLUE -r -w -x 2 --write-period 100" \
              "ZERODDS_DATA_REPR_OFFER=XCDR2 ZERODDS_RUN_SECS=35 $ZD/omg_shapes sub Square 108" 30 '^  <- color' b ;;
# RTI, classic four-member @final ShapeType. The writing side's representation
# comes from an own USER_QOS_PROFILES.xml in the run's working directory.
M9)  pair M9  "ZERODDS_DATA_REPR_OFFER=XCDR1 ZERODDS_RUN_SECS=45 $ZD/classic_shapes pub Square 109 GREEN" \
              "cd $RTIDIR && LD_LIBRARY_PATH=$RTIB $RTI/ShapeType_subscriber -d 109" 40 '^\[color:' b ;;
M10) pair M10 "ZERODDS_DATA_REPR_OFFER=XCDR2 ZERODDS_RUN_SECS=45 $ZD/classic_shapes pub Square 110 GREEN" \
              "cd $RTIDIR && LD_LIBRARY_PATH=$RTIB $RTI/ShapeType_subscriber -d 110" 40 '^\[color:' b ;;
M11) pair M11 "cd $(rti_profiles XCDR) && LD_LIBRARY_PATH=$RTIB $RTI/ShapeType_publisher -d 111 -s 400" \
              "$ZD_ROUNDED_PL ZERODDS_DATA_REPR_OFFER=XCDR1 ZERODDS_RUN_SECS=35 $ZD/classic_shapes sub Square 111" 30 '^  <- color' b ;;
M12) pair M12 "cd $(rti_profiles XCDR2) && LD_LIBRARY_PATH=$RTIB $RTI/ShapeType_publisher -d 112 -s 400" \
              "$ZD_ROUNDED_PL ZERODDS_DATA_REPR_OFFER=XCDR2 ZERODDS_RUN_SECS=35 $ZD/classic_shapes sub Square 112" 30 '^  <- color' b ;;
# OpenDDS, classic type, domain 0 (hardcoded in the demo, so these four run on
# the same domain and must not overlap with each other or with G/H/V4).
M13) pair M13 "ZERODDS_DATA_REPR_OFFER=XCDR1 ZERODDS_RUN_SECS=45 $ZD/classic_shapes pub Square 0 GREEN" \
              "cd $OD && ./build-sec/opendds_sub -DCPSConfigFile rtps.ini" 30 'RECEIVED' b ;;
M14) pair M14 "ZERODDS_DATA_REPR_OFFER=XCDR2 ZERODDS_RUN_SECS=45 $ZD/classic_shapes pub Square 0 GREEN" \
              "cd $OD && ./build-sec/opendds_sub -DCPSConfigFile rtps.ini" 30 'RECEIVED' b ;;
# M15/M16 both carry XCDR2 — see the note at the top of this file. The accept
# list is the only thing that differs, and what they prove is that our reader
# matches this writer under either one and decodes from the header it gets.
M15) pair M15 "cd $OD && ./build-sec/opendds_pub -DCPSConfigFile rtps.ini" \
              "ZERODDS_DATA_REPR_OFFER=XCDR1 ZERODDS_RUN_SECS=35 $ZD/classic_shapes sub Square 0" 30 '^  <- color' b ;;
M16) pair M16 "cd $OD && ./build-sec/opendds_pub -DCPSConfigFile rtps.ini" \
              "ZERODDS_DATA_REPR_OFFER=XCDR2 ZERODDS_RUN_SECS=35 $ZD/classic_shapes sub Square 0" 30 '^  <- color' b ;;
*)  echo "unknown case '$1'"; return 2 ;;
esac
}

CASES=("$@")
if [ ${#CASES[@]} -eq 0 ]; then
  CASES=(V1 V2 V3 V4 A B C D E F G H N1
         M1 M2 M3 M4 M5 M6 M7 M8 M9 M10 M11 M12 M13 M14 M15 M16)
fi
for c in "${CASES[@]}"; do run_case "$c"; done
