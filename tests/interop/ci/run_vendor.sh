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
#   run_vendor.sh <cyclone|fastdds> <outdir>
#
# Vendor client configuration (all optional, sensible CI defaults):
#   PYBIN            python with `cyclonedds` importable         (cyclone)
#   FASTDDS_ROBOT    path to the fastdds_robot pub/sub binary    (fastdds)
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

VENDOR="${1:?usage: run_vendor.sh <cyclone|fastdds> <outdir>}"
OUTDIR="${2:?usage: run_vendor.sh <cyclone|fastdds> <outdir>}"

HERE="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$HERE/../../.." && pwd)"
READER_DIR="$REPO_ROOT/interop/cyclone-xtypes-27/reader"
ZR="$READER_DIR/target/debug/reader"
ZW="$READER_DIR/target/debug/writer"
CYC_DIR="$REPO_ROOT/interop/cyclone-xtypes-27/writers"
RESULT_PY=("python3" "$HERE/interop_result.py")

PYBIN="${PYBIN:-python3}"
FASTDDS_ROBOT="${FASTDDS_ROBOT:-$HERE/fastdds/build/fastdds_robot}"
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
  *) log "unknown vendor: $VENDOR"; exit 2;;
esac
CELL_IDX=0
next_domain() { echo $((DOMAIN_BASE + CELL_IDX)); CELL_IDX=$((CELL_IDX + 1)); }

# ---- ZeroDDS reader-type build (per extensibility) -------------------------
IDLC=(cargo run -q --manifest-path "$REPO_ROOT/Cargo.toml" -p zerodds-idlc --)
BUILT_EXT=""
build_zerodds() {  # $1 = appendable|final
  local ext="$1"; [ "$ext" = "$BUILT_EXT" ] && return 0
  local extra=(); [ "$ext" = "final" ] && extra=(--cyclone)
  rm -f "$READER_DIR/src/robot.rs"
  "${IDLC[@]}" generate "$REPO_ROOT/interop/cyclone-xtypes-27/robot.idl" \
      --rust "${extra[@]}" -o "$READER_DIR/src" >/dev/null 2>&1
  mv "$READER_DIR/src/Robot.rs" "$READER_DIR/src/robot.rs" 2>/dev/null || true
  ( cd "$READER_DIR" && cargo build -q 2>&1 | tail -3 ) \
      || { log "ZeroDDS reader/writer build failed ($ext)"; return 1; }
  BUILT_EXT="$ext"
}

# ---- vendor client commands ------------------------------------------------
# vendor_writer <ext> <rep> <secs> <logfile> -> starts bg, echoes PID
vendor_writer() {
  local ext="$1" rep="$2" secs="$3" logf="$4"
  case "$VENDOR" in
    cyclone) "$PYBIN" "$CYC_DIR/cyclone_writer.py" "$ext" "$rep" "$secs" >"$logf" 2>&1 & echo $!;;
    fastdds) "$FASTDDS_ROBOT" pub "$secs" >"$logf" 2>&1 & echo $!;;
  esac
}
# vendor_reader <ext> <secs> <logfile> -> prints the vendor RESULT line.
# Hard-bounded by `timeout` on top of the client's own internal window.
vendor_reader() {
  local ext="$1" secs="$2" logf="$3" hard=$(( $2 + 15 ))
  case "$VENDOR" in
    cyclone) timeout -k 5 "$hard" "$PYBIN" "$CYC_DIR/cyclone_reader.py" "$ext" "$secs" 2>"$logf" | grep -E '^CYCLONE_RESULT';;
    fastdds) timeout -k 5 "$hard" "$FASTDDS_ROBOT" sub "$secs" 2>"$logf" | grep -E '^FASTDDS_RESULT';;
  esac
}

# ---- cell runners ----------------------------------------------------------
CELLS_JSON=()
record() { CELLS_JSON+=("$1"); }

# forward: vendor writer -> ZeroDDS reader
#   $1 case  $2 writer-ext  $3 rep  $4 zerodds-ext  $5 expect(samples|negative)
forward() {
  local case="$1" wext="$2" rep="$3" zext="$4" expect="$5"
  local dom; dom="$(next_domain)"
  local wlog="$OUTDIR/${VENDOR}-fwd-${CELL_IDX}-writer.log"
  local rlog="$OUTDIR/${VENDOR}-fwd-${CELL_IDX}-reader.log"
  build_zerodds "$zext" || { record "$("${RESULT_PY[@]}" --vendor "$VENDOR" --direction vendor_to_zerodds --case "$case" --expect "$expect" --setup-failed --detail 'zerodds build failed')"; return; }
  local wp; wp="$(ZERODDS_DOMAIN="$dom" vendor_writer "$wext" "$rep" "$WRITE_WINDOW" "$wlog")"; track "$wp"
  sleep "$START_DELAY"
  if ! kill -0 "$wp" 2>/dev/null; then
    record "$("${RESULT_PY[@]}" --vendor "$VENDOR" --direction vendor_to_zerodds --case "$case" --expect "$expect" --setup-failed --detail "vendor writer died: $(tail -1 "$wlog" 2>/dev/null)")"
    return
  fi
  local out timed=0
  out="$(ZERODDS_DOMAIN="$dom" timeout -k 5 "$((READ_WINDOW + 20))" "$ZR" "$READ_WINDOW" 2>"$rlog" | grep -E '^RESULT')" || timed=1
  kill_pid "$wp"
  local flags=(); [ "$timed" = 1 ] && flags+=(--timed-out)
  record "$("${RESULT_PY[@]}" --vendor "$VENDOR" --direction vendor_to_zerodds --case "$case" --expect "$expect" --result-line "${out:-no-result}" "${flags[@]}")"
}

# reverse: ZeroDDS writer -> vendor reader
#   $1 case  $2 zerodds-ext  $3 vendor-ext
reverse() {
  local case="$1" zext="$2" vext="$3"
  local dom; dom="$(next_domain)"
  local wlog="$OUTDIR/${VENDOR}-rev-${CELL_IDX}-writer.log"
  local rlog="$OUTDIR/${VENDOR}-rev-${CELL_IDX}-reader.log"
  build_zerodds "$zext" || { record "$("${RESULT_PY[@]}" --vendor "$VENDOR" --direction zerodds_to_vendor --case "$case" --expect samples --setup-failed --detail 'zerodds build failed')"; return; }
  local wp; wp="$(ZERODDS_DOMAIN="$dom" "$ZW" "$WRITE_WINDOW" >"$wlog" 2>&1 & echo $!)"; track "$wp"
  sleep "$START_DELAY"
  if ! kill -0 "$wp" 2>/dev/null; then
    record "$("${RESULT_PY[@]}" --vendor "$VENDOR" --direction zerodds_to_vendor --case "$case" --expect samples --setup-failed --detail "zerodds writer died: $(tail -1 "$wlog" 2>/dev/null)")"
    return
  fi
  local out timed=0
  out="$(ZERODDS_DOMAIN="$dom" vendor_reader "$vext" "$READ_WINDOW" "$rlog")" || timed=1
  kill_pid "$wp"
  local flags=(); [ "$timed" = 1 ] && flags+=(--timed-out)
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
      if [ ! -x "$FASTDDS_ROBOT" ]; then
        log "SETUP: fastdds_robot binary not found/executable at $FASTDDS_ROBOT"; return 1
      fi
      "$FASTDDS_ROBOT" version >&2 2>/dev/null || true
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
      forward "final+XCDR1 compatible"        final      xcdr1 appendable samples
      forward "appendable+XCDR2 compatible"   appendable xcdr2 appendable samples
      forward "final+XCDR2 vs appendable (neg #27)" final xcdr2 appendable negative
      reverse "ZeroDDS@appendable -> Cyclone" appendable appendable
      forward "final+XCDR2 -> ZeroDDS@final (--cyclone)" final xcdr2 final samples
      ;;
    fastdds)
      # Real data both directions (shared wire path; no XTypes negative dup).
      forward "FastDDS writer -> ZeroDDS reader" final xcdr1 final samples
      reverse "ZeroDDS writer -> FastDDS reader" final final
      ;;
  esac
fi

# ---- aggregate + report ----------------------------------------------------
RUN_JSON="$OUTDIR/${VENDOR}-result.json"
python3 - "$VENDOR" "$RUN_JSON" "${CELLS_JSON[@]}" <<'PY'
import json, sys
vendor, out = sys.argv[1], sys.argv[2]
cells = [json.loads(x) for x in sys.argv[3:] if x.strip()]
green = {"PASS", "EXPECTED_NEGATIVE"}
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
