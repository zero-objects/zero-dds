#!/usr/bin/env bash
# tests/interop/run_interop.sh
#
# End-to-end interop runner.
#
# Starts vendor containers, runs the ZeroDDS spdp_demo against them
# and writes a result report to $REPORT.
#
# Usage:
#
#   ./tests/interop/run_interop.sh [vendor]
#
#     vendor: cyclone | fastdds | both (default: both)
#
# Prerequisites:
#   - docker compose available
#   - Linux with working IP multicast (macOS Docker Desktop
#     does NOT tunnel multicast between container and host).
#   - cargo build available
#
# Exit codes:
#   0  at least one vendor was discovered
#   1  no vendor discovered (discovery failure)
#   2  setup error (Docker / cargo)

set -euo pipefail

VENDOR="${1:-both}"
REPORT="${REPORT:-$(dirname "$0")/INTEROP_REPORT.md}"
DEMO_RUNTIME_SECS="${DEMO_RUNTIME_SECS:-30}"
COMPOSE_FILE="$(dirname "$0")/docker-compose.yml"
LOG_DIR="$(mktemp -d)"
trap 'rm -rf "$LOG_DIR"' EXIT

# ---- Platform check ----
case "$(uname -s)" in
  Darwin)
    echo "WARN: macOS detected. Docker Desktop blocks multicast between" >&2
    echo "      container and host. The test will probably discover nothing." >&2
    echo "      Recommended: run it on Linux." >&2
    ;;
  Linux) : ;;
  *)
    echo "ERROR: unsupported platform: $(uname -s)" >&2
    exit 2
    ;;
esac

# ---- Vendor selection ----
SERVICES=()
case "$VENDOR" in
  cyclone) SERVICES=(cyclone-pub) ;;
  fastdds) SERVICES=(fastdds-pub) ;;
  both) SERVICES=(cyclone-pub fastdds-pub) ;;
  *)
    echo "ERROR: unknown vendor '$VENDOR' (use cyclone|fastdds|both)" >&2
    exit 2
    ;;
esac

echo "=== ZeroDDS Interop Runner ==="
echo "Vendors: ${SERVICES[*]}"
echo "Demo runtime: ${DEMO_RUNTIME_SECS}s"
echo "Report: $REPORT"
echo ""

# ---- Start containers ----
echo ">> Starting vendor containers..."
if ! docker compose -f "$COMPOSE_FILE" up -d "${SERVICES[@]}"; then
  echo "ERROR: docker compose up failed" >&2
  exit 2
fi
trap 'docker compose -f "'"$COMPOSE_FILE"'" down --timeout 5 >/dev/null 2>&1; rm -rf "'"$LOG_DIR"'"' EXIT

echo ">> Waiting 5s for containers to start emitting beacons..."
sleep 5

# ---- Build the ZeroDDS demo ----
echo ">> Building spdp_demo..."
if ! cargo build --example spdp_demo -p dds-discovery 2>"$LOG_DIR/build.log"; then
  echo "ERROR: cargo build failed; see $LOG_DIR/build.log" >&2
  cat "$LOG_DIR/build.log" >&2
  exit 2
fi

# ---- Run the demo against the containers ----
echo ">> Running spdp_demo for ${DEMO_RUNTIME_SECS}s..."
DEMO_OUT="$LOG_DIR/demo.out"
DEMO_BIN="$(cargo build --example spdp_demo -p dds-discovery --message-format=short 2>/dev/null;
            ls target/debug/examples/spdp_demo)"

# The demo ends internally after 30s; we kill it if it runs longer.
"$DEMO_BIN" >"$DEMO_OUT" 2>&1 &
DEMO_PID=$!
sleep "$DEMO_RUNTIME_SECS"
kill "$DEMO_PID" 2>/dev/null || true
wait "$DEMO_PID" 2>/dev/null || true

# ---- Extract discovery results ----
DISCOVERED=$(grep -c '✓ DISCOVERED' "$DEMO_OUT" || true)
FOREIGN_DISCOVERED=$(grep '✓ DISCOVERED' "$DEMO_OUT" |
                    grep -v 'vendor=VendorId(\[1, 240\])' || true)
FOREIGN_COUNT=$(echo "$FOREIGN_DISCOVERED" | grep -c '✓ DISCOVERED' || true)

# ---- Write the report ----
{
  echo "# ZeroDDS Interop Run Report"
  echo ""
  echo "**Date**: $(date -u +%Y-%m-%dT%H:%M:%SZ)"
  echo "**Platform**: $(uname -srm)"
  echo "**Vendors tested**: ${SERVICES[*]}"
  echo "**Demo runtime**: ${DEMO_RUNTIME_SECS}s"
  echo ""
  echo "## Discovery results"
  echo ""
  echo "- Total participants discovered: $DISCOVERED"
  echo "- Of these non-ZeroDDS (vendor != 0x01F0): $FOREIGN_COUNT"
  echo ""
  if [[ "$FOREIGN_COUNT" -gt 0 ]]; then
    echo "### Vendor participants"
    echo ""
    echo '```'
    echo "$FOREIGN_DISCOVERED"
    echo '```'
  fi
  echo ""
  echo "## Demo output (truncated)"
  echo ""
  echo '```'
  head -40 "$DEMO_OUT"
  echo '```'
} >"$REPORT"

echo ""
echo "=== Result ==="
echo "Total participants discovered: $DISCOVERED"
echo "Vendor (non-ZeroDDS) discovered: $FOREIGN_COUNT"
echo "Report written to: $REPORT"

if [[ "$FOREIGN_COUNT" -gt 0 ]]; then
  echo "✓ INTEROP OK: at least one foreign-vendor participant discovered"
  exit 0
else
  echo "✗ INTEROP FAILED: no foreign-vendor participant discovered"
  exit 1
fi
