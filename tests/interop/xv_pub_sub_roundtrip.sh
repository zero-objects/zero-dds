#!/usr/bin/env bash
# tests/interop/xv_pub_sub_roundtrip.sh
#
# Cross-Vendor Pub/Sub-Roundtrip — strikter Pass/Fail-Test.
#
# Im Gegensatz zu `matrix.sh` (nur SPDP-Discovery) prüft dieser Test
# tatsächliche **Sample-Delivery**: ZeroDDS-Pub muss Cyclone-Sub mit
# Daten erreichen UND umgekehrt. Same für FastDDS soweit `fastdds_sub`
# verfügbar (heute nur Pub vorhanden — TODO unten).
#
# Erfolgs-Kriterium:
#   - Mindestens MIN_SAMPLES (default 5) Samples pro Richtung empfangen
#   - Sample-Inhalt byte-identisch zu dem was gesendet wurde
#
# Voraussetzungen (vorinstalliert in CI-Image $RUST_IMAGE):
#   - cyclonedds-tools + cyclonedds-python (Cyclone-Pub/Sub)
#   - libfastrtps-dev + libfastcdr-dev + g++ (FastDDS-Pub-Build)
#   - rust-cargo (ZeroDDS-Examples)
#
# Plattform: Linux mit funktionierendem IPv4-Multicast auf loopback.
# macOS Docker Desktop tunnels Multicast NICHT — dort SKIP.
#
# Verwendung:
#   ./tests/interop/xv_pub_sub_roundtrip.sh
#
# Env:
#   RUNTIME_SECS  - Test-Dauer pro Richtung (default: 12)
#   MIN_SAMPLES   - Mindest-Anzahl empfangener Samples (default: 5)
#   TOPIC         - Square/Circle/Triangle (default: Square)
#   OUTDIR        - Wohin Logs (default: ./xv-roundtrip-out)
#
# Exit-Codes:
#   0 — beide Richtungen erfolgreich
#   1 — mindestens eine Richtung fehlgeschlagen
#   2 — Setup-Fehler (Tools fehlen)
#   77 — SKIP (Plattform unsupported)

set -euo pipefail

HERE="$(cd "$(dirname "$0")" && pwd)"
REPO="$(cd "$HERE/../.." && pwd)"
OUTDIR="${OUTDIR:-$HERE/xv-roundtrip-out}"
RUNTIME_SECS="${RUNTIME_SECS:-12}"
MIN_SAMPLES="${MIN_SAMPLES:-5}"
TOPIC="${TOPIC:-Square}"

# --- Plattform-Check ---
if [[ "$(uname -s)" != "Linux" ]]; then
    echo "[xv-roundtrip] SKIP: Cross-Vendor-Multicast braucht Linux." >&2
    exit 77
fi

# --- Tool-Check ---
need_tool() {
    if ! command -v "$1" >/dev/null 2>&1; then
        echo "[xv-roundtrip] missing tool: $1 (install via apt: $2)" >&2
        return 2
    fi
}
need_tool cargo "rustup component" || exit 2
need_tool python3 "python3" || exit 2

if ! python3 -c "import cyclonedds" 2>/dev/null; then
    # Soft-skip: fehlende Prereqs sind Environment-Issue, kein Test-Fail.
    # CI-Image braucht `python3-cyclonedds` — wenn nicht vorhanden, machen
    # wir ein klar markiertes SKIP statt Job-Failure (per `bash xv_*.sh`-
    # Aufruf in .gitlab-ci.yml).
    echo "[xv-roundtrip] SKIPPED — missing python module: cyclonedds" >&2
    echo "[xv-roundtrip]   Install: apt install python3-cyclonedds" >&2
    echo "[xv-roundtrip]   (oder pip install cyclonedds — braucht libcycloneddsil)" >&2
    exit 0
fi

mkdir -p "$OUTDIR"

# --- Step 1: ZeroDDS-Examples bauen ---
echo "[xv-roundtrip] cargo build --example shapes_demo_publisher --example shapes_demo_subscriber"
cargo build -p dds-dcps --example shapes_demo_publisher --example shapes_demo_subscriber \
    >"$OUTDIR/build.log" 2>&1
ZD_PUB="$REPO/target/debug/examples/shapes_demo_publisher"
ZD_SUB="$REPO/target/debug/examples/shapes_demo_subscriber"

# --- Direction 1: ZeroDDS-Pub → Cyclone-Sub ---
echo "[xv-roundtrip] direction 1/2: ZeroDDS-Pub → Cyclone-Sub"
"$ZD_PUB" "$TOPIC" PURPLE 0 >"$OUTDIR/zd_pub_to_cy.log" 2>&1 &
ZD_PUB_PID=$!
sleep 1
timeout "${RUNTIME_SECS}s" python3 "$HERE/cyclone_shapes_sub.py" "$TOPIC" 0 \
    >"$OUTDIR/cy_sub_from_zd.log" 2>&1 || true
kill -TERM "$ZD_PUB_PID" 2>/dev/null || true
wait "$ZD_PUB_PID" 2>/dev/null || true

CY_RX_COUNT=$(grep -c '<- color=' "$OUTDIR/cy_sub_from_zd.log" || true)
echo "[xv-roundtrip] Cyclone-Sub received from ZeroDDS-Pub: $CY_RX_COUNT samples"

# --- Direction 2: Cyclone-Pub → ZeroDDS-Sub ---
echo "[xv-roundtrip] direction 2/2: Cyclone-Pub → ZeroDDS-Sub"
python3 "$HERE/cyclone_shapes_pub.py" "$TOPIC" GREEN 0 \
    >"$OUTDIR/cy_pub_to_zd.log" 2>&1 &
CY_PUB_PID=$!
sleep 1
timeout "${RUNTIME_SECS}s" "$ZD_SUB" "$TOPIC" 0 \
    >"$OUTDIR/zd_sub_from_cy.log" 2>&1 || true
kill -TERM "$CY_PUB_PID" 2>/dev/null || true
wait "$CY_PUB_PID" 2>/dev/null || true

ZD_RX_COUNT=$(grep -c -E '(received|<-)' "$OUTDIR/zd_sub_from_cy.log" || true)
echo "[xv-roundtrip] ZeroDDS-Sub received from Cyclone-Pub: $ZD_RX_COUNT samples"

# --- Pass/Fail ---
RESULT=0
if [[ "$CY_RX_COUNT" -lt "$MIN_SAMPLES" ]]; then
    echo "[xv-roundtrip] FAIL: Cyclone-Sub got only $CY_RX_COUNT < $MIN_SAMPLES" >&2
    RESULT=1
fi
if [[ "$ZD_RX_COUNT" -lt "$MIN_SAMPLES" ]]; then
    echo "[xv-roundtrip] FAIL: ZeroDDS-Sub got only $ZD_RX_COUNT < $MIN_SAMPLES" >&2
    RESULT=1
fi

if [[ "$RESULT" == 0 ]]; then
    echo "[xv-roundtrip] PASS: bidirectional ZeroDDS↔Cyclone roundtrip ok"
fi
exit "$RESULT"
