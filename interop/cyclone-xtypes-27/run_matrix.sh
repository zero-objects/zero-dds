#!/usr/bin/env bash
# ============================================================================
# CycloneDDS <-> ZeroDDS XTypes interop regression matrix (issue #27).
#
# Live DCPS-over-UDP interop on domain 100, topic `robot`. Proves:
#   * default (XCDR1) interop works both directions,
#   * the reporter's forced-XCDR2 failure (Cyclone @final vs ZeroDDS
#     @appendable framing) is a decode error SURFACED via take() -> WireError,
#   * `zerodds-idlc --cyclone` (PR: default-final) fixes exactly that path.
#
# Reference vendor: CycloneDDS 11.0.1 (0.10.5 is a manual compat check).
#
# OPT-IN / GATED: needs a Python with `cyclonedds` importable and its C
# library. Configure via env, else the script LOUD-SKIPS (exit 0):
#   PYBIN            python that can `import cyclonedds`   (e.g. a venv python)
#   CYCLONEDDS_HOME  CycloneDDS C install prefix           (for the native lib)
# Example: PYBIN=/path/to/venv/bin/python3 CYCLONEDDS_HOME=/path/to/cyclone \
#          interop/cyclone-xtypes-27/run_matrix.sh
# ============================================================================
set -uo pipefail

HERE="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$HERE/../.." && pwd)"
READER_DIR="$HERE/reader"
PYBIN="${PYBIN:-python3}"
READ_WINDOW="${READ_WINDOW:-12}"
WRITE_WINDOW="${WRITE_WINDOW:-40}"

log() { printf '%s\n' "$*" >&2; }

# ---- gate ------------------------------------------------------------------
if ! "$PYBIN" -c 'import cyclonedds' >/dev/null 2>&1; then
  log "SKIP: '$PYBIN' cannot import cyclonedds (set PYBIN to a venv python + CYCLONEDDS_HOME)."
  exit 0
fi
export CYCLONEDDS_HOME="${CYCLONEDDS_HOME:-}"

# ---- helpers ---------------------------------------------------------------
IDLC=(cargo run -q --manifest-path "$REPO_ROOT/Cargo.toml" -p zerodds-idlc --)

# Regenerate the reader type with the requested extensibility, then build.
#   $1 = appendable | final    (final => generated via --cyclone)
build_reader() {
  local ext="$1"; local extra=()
  [ "$ext" = "final" ] && extra=(--cyclone)
  rm -f "$READER_DIR/src/robot.rs"
  "${IDLC[@]}" generate "$HERE/robot.idl" --rust "${extra[@]}" -o "$READER_DIR/src" >/dev/null 2>&1
  mv "$READER_DIR/src/Robot.rs" "$READER_DIR/src/robot.rs" 2>/dev/null || true
  ( cd "$READER_DIR" && cargo build -q 2>&1 | tail -3 ) || { log "reader build failed ($ext)"; return 1; }
}

FAILS=0
declare -a ROWS

# Forward: Cyclone writer -> ZeroDDS reader.
#   $1 label  $2 writer-ext  $3 rep  $4 expect (samples|nosamples)
forward() {
  local label="$1" wext="$2" rep="$3" expect="$4"
  "$PYBIN" "$HERE/writers/cyclone_writer.py" "$wext" "$rep" "$WRITE_WINDOW" \
    >/tmp/c27_w.log 2>&1 &
  local wp=$!
  sleep 3
  if ! kill -0 "$wp" 2>/dev/null; then log "  writer died ($label): $(cat /tmp/c27_w.log)"; ROWS+=("$label|WRITER-DIED"); FAILS=$((FAILS+1)); return; fi
  local out
  out="$("$READER_DIR/target/debug/reader" "$READ_WINDOW" 2>/dev/null | grep '^RESULT')"
  kill -9 "$wp" 2>/dev/null; sleep 1
  local matched samples errors
  matched="$(sed -n 's/.*matched=\([0-9]*\).*/\1/p' <<<"$out")"
  samples="$(sed -n 's/.*samples=\([0-9]*\).*/\1/p' <<<"$out")"
  errors="$(sed -n 's/.*errors=\([0-9]*\).*/\1/p' <<<"$out")"
  local verdict="PASS"
  if [ "$expect" = "samples" ]; then
    [ "${matched:-0}" = 1 ] && [ "${samples:-0}" -gt 0 ] && [ "${errors:-0}" = 0 ] || verdict="FAIL"
  else # nosamples: matched but zero decoded + decode errors visible
    [ "${matched:-0}" = 1 ] && [ "${samples:-0}" = 0 ] && [ "${errors:-0}" -gt 0 ] || verdict="FAIL"
  fi
  [ "$verdict" = FAIL ] && FAILS=$((FAILS+1))
  ROWS+=("$label|matched=${matched:-?} samples=${samples:-?} errors=${errors:-?}|$verdict")
}

# Reverse: ZeroDDS writer -> Cyclone reader (one green config).
reverse() {
  "$READER_DIR/target/debug/writer" "$WRITE_WINDOW" >/tmp/c27_zw.log 2>&1 &
  local wp=$!
  sleep 3
  local out
  out="$("$PYBIN" "$HERE/writers/cyclone_reader.py" appendable "$READ_WINDOW" 2>/dev/null | grep '^CYCLONE_RESULT')"
  kill -9 "$wp" 2>/dev/null; sleep 1
  local samples; samples="$(sed -n 's/.*samples=\([0-9]*\).*/\1/p' <<<"$out")"
  local verdict="PASS"; [ "${samples:-0}" -gt 0 ] || { verdict="FAIL"; FAILS=$((FAILS+1)); }
  ROWS+=("reverse: ZeroDDS@appendable/XCDR2 -> Cyclone|samples=${samples:-?}|$verdict")
}

# ---- run -------------------------------------------------------------------
log "=== building @appendable reader (ZeroDDS default = reporter's reader) ==="
build_reader appendable || exit 1
forward "Cyclone final+XCDR1  -> ZeroDDS @appendable" final      xcdr1 samples
forward "Cyclone appendable+XCDR2 -> ZeroDDS @appendable" appendable xcdr2 samples
forward "Cyclone final+XCDR2  -> ZeroDDS @appendable (#27)" final  xcdr2 nosamples
reverse

log "=== building @final reader via --cyclone (PR fix) ==="
build_reader final || exit 1
forward "Cyclone final+XCDR2  -> ZeroDDS @final (--cyclone)" final xcdr2 samples

# ---- report ----------------------------------------------------------------
log ""
log "==================== #27 interop matrix ===================="
for row in "${ROWS[@]}"; do
  IFS='|' read -r name data verdict <<<"$row"
  printf '  [%-4s] %-52s %s\n' "${verdict:-?}" "$name" "$data" >&2
done
log "==========================================================="
if [ "$FAILS" -ne 0 ]; then log "REGRESSION: $FAILS case(s) failed"; exit 1; fi
log "all cases as expected"
exit 0
