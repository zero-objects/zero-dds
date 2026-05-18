#!/usr/bin/env bash
# tests/interop/run_interop.sh
#
# End-to-End-Interop-Runner.
#
# Startet Vendor-Container, laesst ZeroDDS-spdp_demo gegen sie laufen
# und schreibt Ergebnis-Report nach $REPORT.
#
# Verwendung:
#
#   ./tests/interop/run_interop.sh [vendor]
#
#     vendor: cyclone | fastdds | both (default: both)
#
# Voraussetzungen:
#   - docker compose verfuegbar
#   - Linux mit funktionierendem IP-Multicast (macOS Docker Desktop
#     tunnels Multicast NICHT zwischen Container und Host).
#   - cargo build verfuegbar
#
# Exit-Codes:
#   0  mind. ein Vendor wurde discovered
#   1  kein Vendor discovered (Discovery-Failure)
#   2  Setup-Fehler (Docker / cargo)

set -euo pipefail

VENDOR="${1:-both}"
REPORT="${REPORT:-$(dirname "$0")/INTEROP_REPORT.md}"
DEMO_RUNTIME_SECS="${DEMO_RUNTIME_SECS:-30}"
COMPOSE_FILE="$(dirname "$0")/docker-compose.yml"
LOG_DIR="$(mktemp -d)"
trap 'rm -rf "$LOG_DIR"' EXIT

# ---- Plattform-Check ----
case "$(uname -s)" in
  Darwin)
    echo "WARN: macOS detected. Docker Desktop blockiert Multicast zwischen" >&2
    echo "      Container und Host. Test wird wahrscheinlich nichts entdecken." >&2
    echo "      Empfohlen: auf Linux laufen lassen." >&2
    ;;
  Linux) : ;;
  *)
    echo "ERROR: unsupported platform: $(uname -s)" >&2
    exit 2
    ;;
esac

# ---- Vendor-Selection ----
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

echo "=== ZeroDDS Interop-Runner ==="
echo "Vendors: ${SERVICES[*]}"
echo "Demo-Runtime: ${DEMO_RUNTIME_SECS}s"
echo "Report: $REPORT"
echo ""

# ---- Container starten ----
echo ">> Starting vendor containers..."
if ! docker compose -f "$COMPOSE_FILE" up -d "${SERVICES[@]}"; then
  echo "ERROR: docker compose up failed" >&2
  exit 2
fi
trap 'docker compose -f "'"$COMPOSE_FILE"'" down --timeout 5 >/dev/null 2>&1; rm -rf "'"$LOG_DIR"'"' EXIT

echo ">> Waiting 5s for containers to start emitting beacons..."
sleep 5

# ---- ZeroDDS-Demo bauen ----
echo ">> Building spdp_demo..."
if ! cargo build --example spdp_demo -p dds-discovery 2>"$LOG_DIR/build.log"; then
  echo "ERROR: cargo build failed; see $LOG_DIR/build.log" >&2
  cat "$LOG_DIR/build.log" >&2
  exit 2
fi

# ---- Demo gegen die Container laufen lassen ----
echo ">> Running spdp_demo for ${DEMO_RUNTIME_SECS}s..."
DEMO_OUT="$LOG_DIR/demo.out"
DEMO_BIN="$(cargo build --example spdp_demo -p dds-discovery --message-format=short 2>/dev/null;
            ls target/debug/examples/spdp_demo)"

# Demo wird intern nach 30s beenden; wir killen falls laenger.
"$DEMO_BIN" >"$DEMO_OUT" 2>&1 &
DEMO_PID=$!
sleep "$DEMO_RUNTIME_SECS"
kill "$DEMO_PID" 2>/dev/null || true
wait "$DEMO_PID" 2>/dev/null || true

# ---- Discovery-Ergebnisse extrahieren ----
DISCOVERED=$(grep -c '✓ DISCOVERED' "$DEMO_OUT" || true)
FOREIGN_DISCOVERED=$(grep '✓ DISCOVERED' "$DEMO_OUT" |
                    grep -v 'vendor=VendorId(\[1, 240\])' || true)
FOREIGN_COUNT=$(echo "$FOREIGN_DISCOVERED" | grep -c '✓ DISCOVERED' || true)

# ---- Report schreiben ----
{
  echo "# ZeroDDS Interop-Run Report"
  echo ""
  echo "**Datum**: $(date -u +%Y-%m-%dT%H:%M:%SZ)"
  echo "**Plattform**: $(uname -srm)"
  echo "**Vendors getestet**: ${SERVICES[*]}"
  echo "**Demo-Runtime**: ${DEMO_RUNTIME_SECS}s"
  echo ""
  echo "## Discovery-Ergebnisse"
  echo ""
  echo "- Total entdeckte Participants: $DISCOVERED"
  echo "- Davon nicht-ZeroDDS (Vendor != 0x01F0): $FOREIGN_COUNT"
  echo ""
  if [[ "$FOREIGN_COUNT" -gt 0 ]]; then
    echo "### Vendor-Participants"
    echo ""
    echo '```'
    echo "$FOREIGN_DISCOVERED"
    echo '```'
  fi
  echo ""
  echo "## Demo-Output (gekuerzt)"
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
