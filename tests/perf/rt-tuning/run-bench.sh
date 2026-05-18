#!/usr/bin/env bash
# run-bench.sh — RT-Bench-Runner fuer roundtrip-1us (D.5b Phase-C).
#
# Workflow:
#   1) preflight.sh — Host-Config verifizieren.
#   2) cyclictest-Baseline — Hardware-Floor messen.
#   3) roundtrip-1us pong+ping mit chrt -f 80 / taskset auf isolierten Cores.
#   4) Histogram-Persistenz nach /tmp/zerodds-rt-bench.hgrm.
#
# Aufruf:
#   sudo bash tests/perf/rt-tuning/run-bench.sh \
#     [--remote 192.0.2.10:7400] [--samples 1000000]

set -euo pipefail

REMOTE="${REMOTE:-127.0.0.1:7400}"
BIND_PONG="${BIND_PONG:-0.0.0.0:7400}"
BIND_PING="${BIND_PING:-0.0.0.0:7401}"
SAMPLES="${SAMPLES:-1000000}"
WARMUP="${WARMUP:-10000}"
PAYLOAD="${PAYLOAD:-64}"
PONG_CORE="${PONG_CORE:-2}"
PING_CORE="${PING_CORE:-3}"
HGRM="${HGRM:-/tmp/zerodds-rt-bench.hgrm}"
ROUNDTRIP_BIN="${ROUNDTRIP_BIN:-./target/release/roundtrip-1us}"

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"

# ---- 1) Preflight ----
echo "=== Preflight ==="
bash "$SCRIPT_DIR/preflight.sh"

# ---- 2) cyclictest-Baseline ----
echo "=== cyclictest baseline (60s) ==="
if command -v cyclictest >/dev/null; then
  taskset -c "$PONG_CORE" chrt -f 80 \
    cyclictest -p 80 -t 1 -n -m -i 200 -l 300000 -q || true
fi

# ---- 3) Bench ----
echo "=== roundtrip-1us bench ==="
if [ ! -x "$ROUNDTRIP_BIN" ]; then
  echo "FAIL: $ROUNDTRIP_BIN nicht gefunden — vorher 'cargo build -p dds-bench-suite --release'" >&2
  exit 1
fi

# Pong im Hintergrund, isoliert.
taskset -c "$PONG_CORE" chrt -f 80 \
  "$ROUNDTRIP_BIN" --role pong --bind "$BIND_PONG" --max-runtime 600 \
  > /tmp/zerodds-rt-pong.log 2>&1 &
PONG_PID=$!
trap 'kill $PONG_PID 2>/dev/null || true' EXIT
sleep 1

# Ping mit ci-gate.
set +e
taskset -c "$PING_CORE" chrt -f 80 \
  "$ROUNDTRIP_BIN" --role ping \
    --remote "$REMOTE" --bind "$BIND_PING" \
    --warmup "$WARMUP" --samples "$SAMPLES" \
    --payload "$PAYLOAD" \
    --hgrm "$HGRM" \
    --ci-gate
PING_RC=$?
set -e

kill $PONG_PID 2>/dev/null || true
wait $PONG_PID 2>/dev/null || true

echo ""
echo "=== Pong-Log (last 10 lines) ==="
tail -10 /tmp/zerodds-rt-pong.log

echo ""
if [ "$PING_RC" -ne 0 ]; then
  echo "FAIL: ping ci-gate verletzt (exit=$PING_RC). Histogram in $HGRM"
  exit "$PING_RC"
fi
echo "OK: ping ci-gate erfuellt. Histogram in $HGRM"
