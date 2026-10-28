#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 ZeroDDS Contributors
#
# Separate-process #32 writer-restart harness. Binaries are PRE-BUILT (never
# compiled inside a restart window). One persistent, timeout-tolerant reader
# runs across a full generation-1 -> death -> generation-2 sequence with
# IDENTICAL restart timing; the reader PID must stay alive so a stopped stream
# after the ZeroDDS writer restart is a product signal, not a reader timeout.
#
# Exit status: NON-ZERO if any cell is FAIL / INVALID / SETUP. Every cell prints
# `RESULT <cell>: <verdict>`.
#
# Restart timing is anchored on the writer WRITER_STREAMING event and is
# IDENTICAL for both death modes:
#   graceful — the orchestrator creates the stop file; the writer leaves its
#              loop and returns from main, so participant/publisher/writer Drop
#              runs and emits an SEDP dispose (WRITER_STOPPING).
#   hard     — the writer is SIGKILLed; no destructor, no dispose (lease only).
# The post-death gap is measured from CONFIRMED process death, not a blind sleep.
#
# PID hygiene: every child is reaped (waited) once; the exit trap only kills PIDs
# that are still alive AND were spawned by this harness. No pkill / name globs.
#
# Usage: run_matrix.sh <outdir> [reader_secs]
set -uo pipefail
HERE="$(cd "$(dirname "$0")" && pwd)"
OUT="${1:?usage: run_matrix.sh <outdir> [reader_secs]}"
RSECS="${2:-40}"
mkdir -p "$OUT"

RR="$HERE/target/release/restart_reader"
RW="$HERE/target/release/restart_writer"
[ -x "$RR" ] || { echo "FATAL: build restart_reader first"; exit 2; }
[ -x "$RW" ] || { echo "FATAL: build restart_writer first"; exit 2; }
ROBR="$HERE/target/release/robot_reader"
ROBW="$HERE/target/release/robot_writer"
[ -x "$ROBR" ] || { echo "FATAL: build robot_reader first"; exit 2; }
[ -x "$ROBW" ] || { echo "FATAL: build robot_writer first"; exit 2; }
GEN2_MIN_ID=100000

GEN1_CAP=120        # safety cap only; gen-1 lifetime is driven from STREAMING
GEN1_SECS=8         # identical gen-1 lifetime from the streaming anchor
GAP_SECS=3          # post-death gap, started only after confirmed death
GEN2_SECS=14
GEN2_START_SEQ=100000
MATCH_TO=10

FAILS=0
PIDS=()
track() { PIDS+=("$1"); }
alive() { kill -0 "$1" 2>/dev/null; }
# Reap a child exactly once and drop it from the tracked set so a later PID
# reuse cannot be mistaken for our process.
reap() {
  local target="$1" x new=()
  wait "$target" 2>/dev/null
  for x in "${PIDS[@]:-}"; do [ -n "$x" ] && [ "$x" != "$target" ] && new+=("$x"); done
  PIDS=("${new[@]:-}")
}
# Exit trap: only signal children that are demonstrably still alive.
cleanup() { local p; for p in "${PIDS[@]:-}"; do [ -n "$p" ] && alive "$p" && kill -9 "$p" 2>/dev/null; done; }
trap cleanup EXIT INT TERM
# Poll until a spawned child is confirmed dead (bounded).
await_death() { local p="$1" _; for _ in $(seq 1 200); do alive "$p" || return 0; sleep 0.1; done; return 1; }
wait_ready() { local _; for _ in $(seq 1 60); do grep -q READER_READY "$1" 2>/dev/null && return 0; sleep 0.2; done; return 1; }
wait_streaming() { local log="$1" gen="$2" _; for _ in $(seq 1 200); do grep -q "WRITER_STREAMING gen=$gen " "$log" 2>/dev/null && return 0; sleep 0.1; done; return 1; }
count_gen() { grep -c "READER_SAMPLE .*gen=$2 " "$1" 2>/dev/null || true; }

verdict() {
  local cell="$1" v="$2" extra="${3:-}"
  echo "  RESULT $cell: $v $extra"
  case "$v" in PASS) ;; *) FAILS=$((FAILS + 1)) ;; esac
}

# ---- primary ZeroDDS -> ZeroDDS restart cell -------------------------------
# args: cell domain mode(graceful|hard) reliability(reliable|besteffort)
run_zz() {
  local cell="$1" dom="$2" mode="$3" rel="$4"
  local rlog="$OUT/${cell}.reader.log" w1="$OUT/${cell}.gen1.log" w2="$OUT/${cell}.gen2.log"
  local stopf="$OUT/${cell}.stop"
  rm -f "$stopf"
  echo "===== CELL $cell : ZeroDDS->ZeroDDS domain=$dom mode=$mode rel=$rel ====="
  "$RR" "$dom" "R_$cell" "$RSECS" "$rel" >"$rlog" 2>&1 & local rpid=$!; track "$rpid"
  wait_ready "$rlog" || { verdict "$cell" SETUP "(reader never READY)"; alive "$rpid" && kill -9 "$rpid" 2>/dev/null; reap "$rpid"; return; }

  # Generation 1: cap is only a safety net; lifetime is driven from STREAMING.
  "$RW" "$dom" "R_$cell" 1 "$GEN1_CAP" "$rel" 0 "$MATCH_TO" "$stopf" >"$w1" 2>&1 & local w1pid=$!; track "$w1pid"
  wait_streaming "$w1" 1 || { verdict "$cell" SETUP "(gen1 never streamed)"; alive "$w1pid" && kill -9 "$w1pid" 2>/dev/null; reap "$w1pid"; alive "$rpid" && kill -9 "$rpid" 2>/dev/null; reap "$rpid"; return; }
  # Identical gen-1 lifetime for graceful and hard, measured from the anchor.
  sleep "$GEN1_SECS"
  local g1; g1=$(count_gen "$rlog" 1)

  if [ "$mode" = hard ]; then
    kill -9 "$w1pid" 2>/dev/null                 # SIGKILL: no drop, no dispose
  else
    : > "$stopf"                                 # cooperative stop request -> Drop
  fi
  await_death "$w1pid" || echo "  WARN gen1 did not die within bound"
  reap "$w1pid"
  local disposed=no; grep -q "WRITER_STOPPING gen=1" "$w1" && disposed=yes
  # Gap begins only now, from confirmed death.
  sleep "$GAP_SECS"
  local matched_after_death; matched_after_death=$(grep -oE "matched=[0-9]+" "$rlog" | tail -1)

  "$RW" "$dom" "R_$cell" 2 "$GEN2_SECS" "$rel" "$GEN2_START_SEQ" "$MATCH_TO" >"$w2" 2>&1 & local w2pid=$!; track "$w2pid"
  sleep $((GEN2_SECS + 2))

  local g2; g2=$(count_gen "$rlog" 2)
  local w2m; w2m=$(grep -oE "WRITER_(MATCHED|NOMATCH) gen=2[^ ]* (count|matched)=[0-9]+" "$w2" | head -1)
  local anchor; anchor=$(grep -oE "WRITER_STREAMING gen=1[^ ]* .*t=[0-9.]+" "$w1" | head -1)
  local ralive=no; alive "$rpid" && ralive=yes
  alive "$w2pid" && kill -9 "$w2pid" 2>/dev/null; reap "$w2pid"
  alive "$rpid" && kill -9 "$rpid" 2>/dev/null; reap "$rpid"
  rm -f "$stopf"
  echo "  gen1=$g1 gen2=$g2 reader_alive=$ralive death=[$mode] disposed=[$disposed] matched_after_death=[$matched_after_death] w2match=[$w2m]"
  # Death modes must differ meaningfully: graceful drops (dispose), hard does not.
  if [ "$mode" = graceful ] && [ "$disposed" != yes ]; then verdict "$cell" INVALID "(graceful cell showed no cooperative Drop)"; return; fi
  if [ "$mode" = hard ] && [ "$disposed" = yes ]; then verdict "$cell" INVALID "(hard cell must not run Drop)"; return; fi
  if [ "$ralive" != yes ]; then verdict "$cell" INVALID "(reader died — timeout, not product)"; return; fi
  if [ "$g1" -gt 0 ] && [ "$g2" -gt 0 ]; then verdict "$cell" PASS
  elif [ "$g1" -gt 0 ]; then verdict "$cell" FAIL "(reader stopped after ZeroDDS writer restart — #32)"
  else verdict "$cell" SETUP "(gen1 never delivered)"; fi
}

# ---- control: fresh late joiner (no restart) must match --------------------
run_fresh_late() {
  local cell=fresh_late dom=160
  local rlog="$OUT/${cell}.reader.log" wl="$OUT/${cell}.writer.log"
  echo "===== CELL $cell : fresh late joiner (control) domain=$dom ====="
  "$RR" "$dom" "R_$cell" 26 reliable >"$rlog" 2>&1 & local rpid=$!; track "$rpid"
  wait_ready "$rlog" || { verdict "$cell" SETUP "(reader never READY)"; alive "$rpid" && kill -9 "$rpid" 2>/dev/null; reap "$rpid"; return; }
  sleep 10
  "$RW" "$dom" "R_$cell" 1 10 reliable 0 "$MATCH_TO" >"$wl" 2>&1 & local wpid=$!; track "$wpid"
  sleep 12
  local g1; g1=$(count_gen "$rlog" 1)
  alive "$wpid" && kill -9 "$wpid" 2>/dev/null; reap "$wpid"
  alive "$rpid" && kill -9 "$rpid" 2>/dev/null; reap "$rpid"
  echo "  gen1=$g1 match=[$(grep -oE "WRITER_(MATCHED|NOMATCH)[^ ]* (count|matched)=[0-9]+" "$wl" | head -1)]"
  [ "$g1" -gt 0 ] && verdict "$cell" PASS || verdict "$cell" FAIL "(fresh late joiner must match)"
}

# ---- control: two concurrently live writers must both match ----------------
run_two_live() {
  local cell=two_live dom=161
  local rlog="$OUT/${cell}.reader.log" a="$OUT/${cell}.w1.log" b="$OUT/${cell}.w2.log"
  echo "===== CELL $cell : two live writers (control) domain=$dom ====="
  "$RR" "$dom" "R_$cell" 26 reliable >"$rlog" 2>&1 & local rpid=$!; track "$rpid"
  wait_ready "$rlog" || { verdict "$cell" SETUP; alive "$rpid" && kill -9 "$rpid" 2>/dev/null; reap "$rpid"; return; }
  "$RW" "$dom" "R_$cell" 1 22 reliable 0 "$MATCH_TO" >"$a" 2>&1 & local ap=$!; track "$ap"
  sleep 7
  "$RW" "$dom" "R_$cell" 2 14 reliable 100000 "$MATCH_TO" >"$b" 2>&1 & local bp=$!; track "$bp"
  sleep 16
  local g1 g2; g1=$(count_gen "$rlog" 1); g2=$(count_gen "$rlog" 2)
  alive "$ap" && kill -9 "$ap" 2>/dev/null; reap "$ap"
  alive "$bp" && kill -9 "$bp" 2>/dev/null; reap "$bp"
  alive "$rpid" && kill -9 "$rpid" 2>/dev/null; reap "$rpid"
  echo "  gen1=$g1 gen2=$g2 w1=[$(grep -oE "WRITER_(MATCHED|NOMATCH) gen=1[^ ]* count=[0-9]+" "$a" | head -1)] w2=[$(grep -oE "WRITER_(MATCHED|NOMATCH) gen=2[^ ]* count=[0-9]+" "$b" | head -1)]"
  { [ "$g1" -gt 0 ] && [ "$g2" -gt 0 ]; } && verdict "$cell" PASS || verdict "$cell" FAIL "(both live writers must match)"
}

# ---- control: several fast restarts, the final generation must match -------
run_multi_restart() {
  local cell=multi_restart dom=162
  local rlog="$OUT/${cell}.reader.log"
  echo "===== CELL $cell : several fast restarts domain=$dom ====="
  "$RR" "$dom" "R_$cell" 40 reliable >"$rlog" 2>&1 & local rpid=$!; track "$rpid"
  wait_ready "$rlog" || { verdict "$cell" SETUP; alive "$rpid" && kill -9 "$rpid" 2>/dev/null; reap "$rpid"; return; }
  local gen
  for gen in 1 2 3 4; do
    "$RW" "$dom" "R_$cell" "$gen" 12 reliable $((gen*100000)) "$MATCH_TO" >"$OUT/${cell}.gen${gen}.log" 2>&1 & local wp=$!; track "$wp"
    wait_streaming "$OUT/${cell}.gen${gen}.log" "$gen" || true
    sleep 3
    kill -9 "$wp" 2>/dev/null                     # hard death each round
    await_death "$wp" || true; reap "$wp"
    sleep 2                                        # short gap each round
  done
  sleep 4
  local gf; gf=$(count_gen "$rlog" 4)
  local ralive=no; alive "$rpid" && ralive=yes
  alive "$rpid" && kill -9 "$rpid" 2>/dev/null; reap "$rpid"
  echo "  final_gen4=$gf reader_alive=$ralive gen4match=[$(grep -oE "WRITER_(MATCHED|NOMATCH) gen=4[^ ]* (count|matched)=[0-9]+" "$OUT/${cell}.gen4.log" | head -1)]"
  [ "$ralive" = yes ] || { verdict "$cell" INVALID "(reader died)"; return; }
  [ "$gf" -gt 0 ] && verdict "$cell" PASS || verdict "$cell" FAIL "(final generation must match after repeated churn)"
}


# ---- typed reporter-equivalent Robot restart cell (final acceptance) --------
# Exchanges the codegen `Robot { id, label }` type. The reader asserts concrete
# generation-2 values (label==2, id in the disjoint gen-2 range); zero valid
# gen-2 samples is a non-zero FAIL. args: cell domain mode reliability
run_robot() {
  local cell="$1" dom="$2" mode="$3" rel="$4"
  local rlog="$OUT/${cell}.reader.log" w1="$OUT/${cell}.gen1.log" w2="$OUT/${cell}.gen2.log"
  local stopf="$OUT/${cell}.stop"
  rm -f "$stopf"
  echo "===== CELL $cell : typed Robot ZeroDDS->ZeroDDS domain=$dom mode=$mode rel=$rel ====="
  "$ROBR" "$dom" "R_$cell" "$RSECS" "$rel" "$GEN2_MIN_ID" >"$rlog" 2>&1 & local rpid=$!; track "$rpid"
  wait_ready "$rlog" || { verdict "$cell" SETUP "(reader never READY)"; alive "$rpid" && kill -9 "$rpid" 2>/dev/null; reap "$rpid"; return; }

  "$ROBW" "$dom" "R_$cell" 1 "$GEN1_CAP" "$rel" 0 "$MATCH_TO" "$stopf" >"$w1" 2>&1 & local w1pid=$!; track "$w1pid"
  wait_streaming "$w1" 1 || { verdict "$cell" SETUP "(gen1 never streamed)"; alive "$w1pid" && kill -9 "$w1pid" 2>/dev/null; reap "$w1pid"; alive "$rpid" && kill -9 "$rpid" 2>/dev/null; reap "$rpid"; return; }
  sleep "$GEN1_SECS"

  if [ "$mode" = hard ]; then
    kill -9 "$w1pid" 2>/dev/null
  else
    : > "$stopf"
  fi
  await_death "$w1pid" || echo "  WARN gen1 did not die within bound"
  reap "$w1pid"
  local disposed=no; grep -q "WRITER_STOPPING gen=1" "$w1" && disposed=yes
  sleep "$GAP_SECS"

  "$ROBW" "$dom" "R_$cell" 2 "$GEN2_SECS" "$rel" "$GEN2_MIN_ID" "$MATCH_TO" >"$w2" 2>&1 & local w2pid=$!; track "$w2pid"
  sleep $((GEN2_SECS + 2))

  local g1 g2ok g2bad
  g1=$(grep -cE "ROBOT_SAMPLE .*gen=1 " "$rlog" 2>/dev/null || true)
  g2ok=$(grep -c "ROBOT_GEN2_OK" "$rlog" 2>/dev/null || true)
  g2bad=$(grep -c "ROBOT_GEN2_BAD" "$rlog" 2>/dev/null || true)
  local sampleok; sampleok=$(grep -m1 "ROBOT_GEN2_OK" "$rlog" 2>/dev/null || true)
  local w2m; w2m=$(grep -oE "WRITER_(MATCHED|NOMATCH) gen=2[^ ]* (count|matched)=[0-9]+" "$w2" | head -1)
  local ralive=no; alive "$rpid" && ralive=yes
  alive "$w2pid" && kill -9 "$w2pid" 2>/dev/null; reap "$w2pid"
  alive "$rpid" && kill -9 "$rpid" 2>/dev/null; reap "$rpid"
  rm -f "$stopf"
  echo "  gen1=$g1 gen2_ok=$g2ok gen2_bad=$g2bad reader_alive=$ralive death=[$mode] disposed=[$disposed] w2match=[$w2m] [$sampleok]"
  if [ "$mode" = graceful ] && [ "$disposed" != yes ]; then verdict "$cell" INVALID "(graceful cell showed no cooperative Drop)"; return; fi
  if [ "$mode" = hard ] && [ "$disposed" = yes ]; then verdict "$cell" INVALID "(hard cell must not run Drop)"; return; fi
  if [ "$ralive" != yes ]; then verdict "$cell" INVALID "(reader died — timeout, not product)"; return; fi
  if [ "$g2bad" -gt 0 ]; then verdict "$cell" FAIL "(typed gen2 value out of the expected range)"; return; fi
  if [ "$g1" -gt 0 ] && [ "$g2ok" -gt 0 ]; then verdict "$cell" PASS
  elif [ "$g1" -gt 0 ]; then verdict "$cell" FAIL "(no typed gen2 samples after ZeroDDS writer restart — #32)"
  else verdict "$cell" SETUP "(gen1 never delivered)"; fi
}

run_zz zz_hard_reliable     132 hard     reliable
run_zz zz_graceful_reliable 133 graceful reliable
run_zz zz_hard_besteffort   134 hard     besteffort
run_fresh_late
run_two_live
run_multi_restart
run_robot robot_hard_reliable     150 hard     reliable
run_robot robot_graceful_reliable 151 graceful reliable

echo "===== matrix done: $FAILS failing cell(s); logs in $OUT ====="
[ "$FAILS" -eq 0 ] || exit 1
exit 0
