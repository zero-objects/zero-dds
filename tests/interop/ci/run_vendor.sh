#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 ZeroDDS Contributors
# ============================================================================
# Cross-vendor DDS interop CI gate runner (issue #28).
#
# Exchanges typed `Robot` samples over live DDS/RTPS between ZeroDDS and one
# open-source vendor (CycloneDDS or Fast DDS), in BOTH directions, and asserts
# endpoint matching AND actual decoded sample delivery — SPDP discovery alone
# is never a pass.
#
# Usage:
#   run_vendor.sh <cyclone|fastdds|rti> <outdir>
#
# Vendor client configuration (all optional, sensible CI defaults):
#   PYBIN            python with `cyclonedds` importable         (cyclone)
#   FASTDDS_ROBOT    path to the fastdds_robot pub/sub binary    (fastdds)
#   RTI_ROBOT        path to the rti_robot pub/sub binary        (rti)
#   READ_WINDOW      reader observation window, seconds          (default 12)
#   WRITE_WINDOW     writer lifetime, seconds                    (default 30)
#   START_DELAY      bounded writer warm-up before the reader    (default 3)
#   DOMAIN_BASE      first DDS domain id used                    (per vendor)
#
# Exit status: 0 iff every cell is green (PASS or EXPECTED_NEGATIVE).
# Writes <outdir>/<vendor>-result.json (machine-readable) and prints a human
# summary. Per-cell logs are kept in <outdir>/.
# ============================================================================
set -uo pipefail

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
VENDOR="${1:?usage: run_vendor.sh <cyclone|fastdds|rti> <outdir>}"
OUTDIR="${2:?usage: run_vendor.sh <cyclone|fastdds|rti> <outdir>}"
case "$VENDOR" in cyclone|rti) export ZERODDS_WIRE_PL_CDR1_READING=tolerant ;; esac
# A reader that announces no TypeConsistencyEnforcement (PID 0x0074). DDS-XTypes
# 1.3 section 7.6.3.4.1 has a writer assume DISALLOW_TYPE_COERCION for it, the
# default of ZeroDDS (xtypes.type_consistency_absent=disallow), under which an
# assignable but not equivalent pair does not match. Cyclone DDS 11.0.1 announces
# no such PID and its reader coerces (measured: the Cyclone reader matches the
# ZeroDDS writer of the TypeLookup cell below and ZeroDDS refused it with
# "DISALLOW_TYPE_COERCION: the types are not equivalent"). The ZeroDDS writers of
# the Cyclone pairs read the absent policy as ALLOW_TYPE_COERCION. This is the
# only place the runner sets it.
case "$VENDOR" in cyclone) export ZERODDS_TYPE_CONSISTENCY_ABSENT=allow ;; esac

HERE="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$HERE/../../.." && pwd)"
READER_DIR="$REPO_ROOT/interop/cyclone-xtypes-27/reader"
ZR="$READER_DIR/target/debug/reader"
ZW="$READER_DIR/target/debug/writer"
CYC_DIR="$REPO_ROOT/interop/cyclone-xtypes-27/writers"
RESULT_PY=("python3" "$HERE/interop_result.py")

PYBIN="${PYBIN:-${INTEROP_PYBIN:-python3}}"
FASTDDS_ROBOT="${FASTDDS_ROBOT:-$HERE/fastdds/build/fastdds_robot}"
RTI_ROBOT="${RTI_ROBOT:-$HERE/rti/build/rti_robot}"
FASTDDS_PREFIX="${FASTDDS_PREFIX:-/opt/fastdds}"
READ_WINDOW="${READ_WINDOW:-12}"
WRITE_WINDOW="${WRITE_WINDOW:-30}"
START_DELAY="${START_DELAY:-3}"

mkdir -p "$OUTDIR"
log() { printf '%s\n' "$*" >&2; }

# ---- exact-PID cleanup (never a broad pkill) -------------------------------
# shellcheck source=tests/interop/ci/lib.sh
. "$HERE/lib.sh"
trap cleanup EXIT INT TERM

# ---- per-vendor domain base (isolate cells within a job) -------------------
case "$VENDOR" in
  cyclone) DOMAIN_BASE="${DOMAIN_BASE:-121}";;
  fastdds) DOMAIN_BASE="${DOMAIN_BASE:-141}";;
  rti)     DOMAIN_BASE="${DOMAIN_BASE:-220}";;
  *) log "unknown vendor: $VENDOR"; exit 2;;
esac
CELL_IDX=0

# ---- ZeroDDS reader-type build (per extensibility) -------------------------
# The Robot type is generated in the reader's build.rs (zerodds-build);
# the case's extensibility reaches it through the environment and a
# value build.rs does not know fails the build rather than defaulting.
BUILT_EXT=""
build_zerodds() {  # $1 = appendable|final
  local ext="$1"; [ "$ext" = "$BUILT_EXT" ] && return 0
  ( cd "$READER_DIR" && ZERODDS_ROBOT_EXTENSIBILITY="$ext" cargo build -q 2>&1 | tail -3 ) \
      || { log "ZeroDDS reader/writer build failed ($ext)"; return 1; }
  BUILT_EXT="$ext"
}

# ---- vendor client commands ------------------------------------------------
# vendor_writer <ext> <rep> <secs> <domain> <logfile> -> starts bg, echoes PID
vendor_writer() {
  local ext="$1" rep="$2" secs="$3" domain="$4" logf="$5"
  case "$VENDOR" in
    cyclone) "$PYBIN" "$CYC_DIR/cyclone_writer.py" "$ext" "$rep" "$secs" "$domain" >"$logf" 2>&1 & echo $!;;
    fastdds) "$FASTDDS_ROBOT" pub "$secs" "$domain" >"$logf" 2>&1 & echo $!;;
    rti)     "$RTI_ROBOT" pub "$secs" "$domain" >"$logf" 2>&1 & echo $!;;
  esac
}
# vendor_reader <ext> <secs> <domain> <logfile> -> prints the vendor RESULT line.
# Hard-bounded by `timeout` on top of the client's own internal window.
vendor_reader() {
  local ext="$1" secs="$2" domain="$3" logf="$4" hard=$(( $2 + 15 ))
  case "$VENDOR" in
    cyclone) timeout -k 5 "$hard" "$PYBIN" "$CYC_DIR/cyclone_reader.py" "$ext" "$secs" "$domain" 2>"$logf" | grep -E '^CYCLONE_RESULT';;
    fastdds) timeout -k 5 "$hard" "$FASTDDS_ROBOT" sub "$secs" "$domain" 2>"$logf" | grep -E '^FASTDDS_RESULT';;
    rti)     timeout -k 5 "$hard" "$RTI_ROBOT" sub "$secs" "$domain" 2>"$logf" | grep -E '^RTI_RESULT';;
  esac
}

# ---- cell runners ----------------------------------------------------------
CELLS_JSON=()
record() { CELLS_JSON+=("$1"); }

# domain_report <expected> <logfile> <party> -> a mismatch message, or empty
# when the party's log reports exactly the requested domain. Used to prove that
# each started party actually ran on the domain the runner requested.
domain_report() {
  local exp="$1" logf="$2" party="$3" got
  got="$(grep -aoE "domain=[0-9]+" "$logf" 2>/dev/null | head -1 | cut -d= -f2)"
  [ "$got" = "$exp" ] || echo "$party reported domain ${got:-none} != requested $exp; "
}

# forward: vendor writer -> ZeroDDS reader
#   $1 case  $2 writer-ext  $3 rep  $4 zerodds-ext  $5 expect(samples|negative)
forward() {
  local case="$1" wext="$2" rep="$3" zext="$4" expect="$5"
  local dom=$((DOMAIN_BASE + CELL_IDX)); CELL_IDX=$((CELL_IDX + 1))
  local wlog="$OUTDIR/${VENDOR}-fwd-${CELL_IDX}-writer.log"
  local rlog="$OUTDIR/${VENDOR}-fwd-${CELL_IDX}-reader.log"
  build_zerodds "$zext" || { record "$("${RESULT_PY[@]}" --vendor "$VENDOR" --direction vendor_to_zerodds --case "$case" --expect "$expect" --setup-failed --detail 'zerodds build failed')"; return; }
  local wp; wp="$(vendor_writer "$wext" "$rep" "$WRITE_WINDOW" "$dom" "$wlog")"; track "$wp"
  sleep "$START_DELAY"
  if ! kill -0 "$wp" 2>/dev/null; then
    record "$("${RESULT_PY[@]}" --vendor "$VENDOR" --direction vendor_to_zerodds --case "$case" --expect "$expect" --setup-failed --detail "vendor writer died: $(tail -1 "$wlog" 2>/dev/null)")"
    return
  fi
  local out timed=0
  out="$(timeout -k 5 "$((READ_WINDOW + 20))" "$ZR" "$READ_WINDOW" "$dom" 2>"$rlog" | grep -E '^RESULT')" || timed=1
  kill_pid "$wp"
  local flags=(); [ "$timed" = 1 ] && flags+=(--timed-out)
  local dmis; dmis="$(domain_report "$dom" "$wlog" 'vendor writer')$(domain_report "$dom" "$rlog" 'zerodds reader')"
  if [ -n "$dmis" ]; then
    record "$("${RESULT_PY[@]}" --vendor "$VENDOR" --direction vendor_to_zerodds --case "$case" --expect "$expect" --setup-failed --detail "domain contract: ${dmis}")"
    return
  fi
  record "$("${RESULT_PY[@]}" --vendor "$VENDOR" --direction vendor_to_zerodds --case "$case" --expect "$expect" --result-line "${out:-no-result}" "${flags[@]}")"
}

# reverse: ZeroDDS writer -> vendor reader
#   $1 case  $2 zerodds-ext  $3 vendor-ext
reverse() {  # <case> <zerodds-ext> <vendor-ext> [env for the ZeroDDS writer]
  local case="$1" zext="$2" vext="$3" zenv="${4:-}"
  local dom=$((DOMAIN_BASE + CELL_IDX)); CELL_IDX=$((CELL_IDX + 1))
  local wlog="$OUTDIR/${VENDOR}-rev-${CELL_IDX}-writer.log"
  local rlog="$OUTDIR/${VENDOR}-rev-${CELL_IDX}-reader.log"
  build_zerodds "$zext" || { record "$("${RESULT_PY[@]}" --vendor "$VENDOR" --direction zerodds_to_vendor --case "$case" --expect samples --setup-failed --detail 'zerodds build failed')"; return; }
  # shellcheck disable=SC2086 # zenv is a list of NAME=value words
  local wp; wp="$(env $zenv "$ZW" "$WRITE_WINDOW" "$dom" >"$wlog" 2>&1 & echo $!)"; track "$wp"
  sleep "$START_DELAY"
  if ! kill -0 "$wp" 2>/dev/null; then
    record "$("${RESULT_PY[@]}" --vendor "$VENDOR" --direction zerodds_to_vendor --case "$case" --expect samples --setup-failed --detail "zerodds writer died: $(tail -1 "$wlog" 2>/dev/null)")"
    return
  fi
  local out timed=0
  out="$(vendor_reader "$vext" "$READ_WINDOW" "$dom" "$rlog")" || timed=1
  kill_pid "$wp"
  local flags=(); [ "$timed" = 1 ] && flags+=(--timed-out)
  local dmis; dmis="$(domain_report "$dom" "$wlog" 'zerodds writer')$(domain_report "$dom" "$rlog" 'vendor reader')"
  if [ -n "$dmis" ]; then
    record "$("${RESULT_PY[@]}" --vendor "$VENDOR" --direction zerodds_to_vendor --case "$case" --expect samples --setup-failed --detail "domain contract: ${dmis}")"
    return
  fi
  record "$("${RESULT_PY[@]}" --vendor "$VENDOR" --direction zerodds_to_vendor --case "$case" --expect samples --result-line "${out:-no-result}" "${flags[@]}")"
}

# ---- preflight -------------------------------------------------------------
preflight() {
  log "=== preflight: $VENDOR ==="
  case "$VENDOR" in
    cyclone)
      if ! "$PYBIN" -c 'import cyclonedds' 2>/dev/null; then
        log "SETUP: '$PYBIN' cannot import cyclonedds"; return 1
      fi
      "$PYBIN" -c 'import cyclonedds, sys; print("cyclonedds", getattr(cyclonedds,"__version__","?"))' >&2 2>/dev/null || true
      ;;
    fastdds)
      # Build the fastdds_robot harness client against the pre-installed Fast DDS
      # SDK ($FASTDDS_PREFIX) if it is not already present. This compiles OUR
      # interop client (like the cargo build of the ZeroDDS reader) — it never
      # builds the vendor SDK, which the image ships pinned.
      if [ ! -x "$FASTDDS_ROBOT" ]; then
        log "fastdds_robot absent — building against $FASTDDS_PREFIX"
        cmake -S "$HERE/fastdds" -B "$HERE/fastdds/build" \
              -DCMAKE_BUILD_TYPE=Release \
              -DCMAKE_PREFIX_PATH="$FASTDDS_PREFIX" >&2 \
          || { log "SETUP: cmake configure for fastdds_robot failed"; return 1; }
        cmake --build "$HERE/fastdds/build" --parallel >&2 \
          || { log "SETUP: fastdds_robot build failed"; return 1; }
      fi
      if [ ! -x "$FASTDDS_ROBOT" ]; then
        log "SETUP: fastdds_robot still missing at $FASTDDS_ROBOT after build"; return 1
      fi
      "$FASTDDS_ROBOT" version >&2 \
        || { log "SETUP: fastdds_robot version check failed"; return 1; }
      ;;
    rti)
      # Same shape as the Fast DDS branch: build OUR client against the
      # pre-installed Connext, never Connext itself. `rti/build.sh` generates
      # the type support from ../fastdds/robot.idl, so both vendor cells
      # measure the same type.
      # Where Connext is and what its target architecture is called live in
      # one file for the whole tree; sourced here rather than at the top so
      # the Cyclone and Fast DDS cells keep running on a host that has no
      # Connext at all.
      # shellcheck source=tools/interop-vendors.sh
      . "$REPO_ROOT/tools/interop-vendors.sh"
      if [ ! -x "$RTI_ROBOT" ]; then
        log "rti_robot absent — building against NDDSHOME=$NDDSHOME"
        bash "$HERE/rti/build.sh" >&2 \
          || { log "SETUP: rti_robot build failed"; return 1; }
      fi
      if [ ! -x "$RTI_ROBOT" ]; then
        log "SETUP: rti_robot still missing at $RTI_ROBOT after build"; return 1
      fi
      # The runtime — unlike rtiddsgen — refuses to start without a licence,
      # and it looks for the QoS profile through the environment rather than
      # the working directory.
      export RTI_LICENSE_FILE="${RTI_LICENSE_FILE:-$NDDSHOME/rti_license.dat}"
      export NDDS_QOS_PROFILES="${NDDS_QOS_PROFILES:-$HERE/rti/USER_QOS_PROFILES.xml}"
      export LD_LIBRARY_PATH="$RTI_LIB:${LD_LIBRARY_PATH:-}"
      "$RTI_ROBOT" version >&2 \
        || { log "SETUP: rti_robot version check failed"; return 1; }
      ;;
  esac
  return 0
}

# ---- run -------------------------------------------------------------------
if ! preflight; then
  # Emit a single SETUP_FAIL cell so the artifact states what happened.
  record "$("${RESULT_PY[@]}" --vendor "$VENDOR" --direction preflight --case "vendor available" --expect samples --setup-failed --detail 'vendor preflight failed')"
else
  case "$VENDOR" in
    cyclone)
      # Retain the #29 XTypes regression semantics.
      #
      # The two `refusal` cells put a Cyclone `@final` writer against a
      # ZeroDDS `@appendable` reader. A `@final` type is NOT assignable from
      # an `@appendable` one (XTypes 1.3 §7.2.4.4: the two carry different
      # extensibility, and the rule admits no pair whose extensibility kinds
      # differ), so the correct outcome is that the endpoints never connect
      # and no octet crosses. Cyclone against Cyclone, both announcing full
      # type information, was measured doing exactly that.
      #
      # These two cells used to deliver samples, and that was not a feature:
      # ZeroDDS announced no PID_TYPE_INFORMATION (issue #42), so the peer
      # had nothing to check and fell back to comparing type NAMES. The
      # `@final`/`@appendable` difference then surfaced where it does the
      # most damage — while decoding a sample, one sample at a time — which
      # is what the third cell used to record as 39 decode errors. Announcing
      # the type moves the difference to where it belongs, the match.
      #
      # The `samples` cells beside them are what keeps this honest: cases 2
      # and 5 pair equal extensibilities and still deliver, so a regression
      # in which ZeroDDS announces something the peer cannot read at all
      # would take those down too, and could not hide behind a `refusal`.
      #
      # The last two cells make ZeroDDS the side that asks. The writer's
      # type hashes differently from the reader's, so the reader has to fetch
      # it with a TypeLookup `getTypes` request and decide on the object it
      # gets back: a member appended (assignable, XTypes 1.3 §7.2.4.4.8) has
      # to deliver, `label` retyped to a string has to be refused. Without
      # the lookup both cells would end alike. Cyclone answers `getTypes` and
      # ignores the `getTypeDependencies` request ZeroDDS sends after it
      # ("unknown req-type 95091505" in its trace); this type has no
      # dependencies.
      forward "final+XCDR1 vs appendable (refused)"  final xcdr1 appendable refusal
      forward "appendable+XCDR2 compatible"   appendable xcdr2 appendable samples
      forward "final+XCDR2 vs appendable (refused, #27)" final xcdr2 appendable refusal
      reverse "ZeroDDS@appendable -> Cyclone" appendable appendable
      forward "final+XCDR2 -> ZeroDDS@final (--cyclone)" final xcdr2 final samples
      forward "appendable+member appended -> ZeroDDS@appendable (TypeLookup)" appendable-extended xcdr2 appendable samples
      forward "appendable, label a string vs ZeroDDS@appendable (TypeLookup, refused)" appendable-retyped xcdr2 appendable refusal
      # The other direction: the Cyclone reader has a member appended and asks
      # ZeroDDS for the writer's type. The ZeroDDS writer offers XCDR2 in this
      # cell: under XCDR1 an appendable sample has no DHEADER, and the Python
      # binding of Cyclone fails to read a sample shorter than its type
      # ("struct.error: unpack_from requires a buffer of at least 16 bytes",
      # measured), where XTypes 1.3 section 7.2.4.4.8 gives the missing member
      # its default.
      reverse "ZeroDDS@appendable -> Cyclone@appendable+member appended (Cyclone asks, TypeLookup)" appendable appendable-extended "ZERODDS_DATA_REPR_OFFER=XCDR2"
      ;;
    fastdds)
      # Real data both directions (shared wire path; no XTypes negative dup).
      forward "FastDDS writer -> ZeroDDS reader" final xcdr1 final samples
      reverse "ZeroDDS writer -> FastDDS reader" final final
      ;;
    rti)
      # Real data both directions, on the same @final/XCDR1 pairing as the
      # Fast DDS cells. RTI is the strictest of the four at discovery — it
      # cross-checks the `typeobject_serialized_size` a peer announces beside
      # a COMPLETE TypeIdentifier against its own and refuses the endpoint
      # when the two disagree — so a cell that reports 0 samples here is
      # usually a discovery refusal and the vendor log says so by name.
      forward "RTI writer -> ZeroDDS reader" final xcdr1 final samples
      reverse "ZeroDDS writer -> RTI reader" final final
      ;;
  esac
fi

# ---- aggregate + report ----------------------------------------------------
RUN_JSON="$OUTDIR/${VENDOR}-result.json"
python3 - "$VENDOR" "$RUN_JSON" "${CELLS_JSON[@]}" <<'PY'
import json, sys
vendor, out = sys.argv[1], sys.argv[2]
cells = [json.loads(x) for x in sys.argv[3:] if x.strip()]
green = {"PASS", "EXPECTED_NEGATIVE", "EXPECTED_REFUSAL"}
ok = bool(cells) and all(c["status"] in green for c in cells)
doc = {"vendor": vendor, "ok": ok, "cells": cells}
with open(out, "w") as f:
    json.dump(doc, f, indent=2)
print(f"\n==================== {vendor} interop gate ====================", file=sys.stderr)
for c in cells:
    print(f"  [{c['status']:<17}] {c['direction']:<18} {c['case']}"
          + (f"  {c['observed']}" if c.get('observed') else ""), file=sys.stderr)
print("=" * 62, file=sys.stderr)
print(("PASS" if ok else "FAIL") + f": {vendor} interop gate", file=sys.stderr)
sys.exit(0 if ok else 1)
PY
