#!/usr/bin/env bash
# tests/interop/xcdr2_cross_vendor.sh
#
# L4 Cross-Vendor XCDR2 Conformance — `zerodds-xcdr2-bindings-conformance-1.0` §8.
#
# Verifiziert dass ZeroDDS-Encoder die gleichen XCDR2-Wire-Bytes
# produziert wie Cyclone DDS, und dass ZeroDDS-Decoder Cyclone-Bytes
# korrekt liest. Die Test-Vektoren V-1..V-12 sind in der Master-Spec
# §6 woertlich und gelten als spec-genaue Wire-Form.
#
# Zwei Varianten werden gepruft:
#
# 1. **Static fixtures**: `crates/discovery/tests/fixtures/cyclone-xcdr2/v*.bin`
#    sind mit ddsperf/Cyclone-Tools aufgenommene Wire-Frames pro V-i.
#    ZeroDDS-Decoder muss sie ohne Verlust dekodieren.
#
# 2. **Live roundtrip**: `cyclonedds-tools` (cycdr o.ae.) muss im PATH
#    sein. Wir starten Cyclone-Pub mit V-i-Sample, ZeroDDS-Sub liest
#    via xv_pub_sub_roundtrip.sh-Pattern.
#
# Erfolgs-Kriterium:
#   - Fixtures: jeder v*.bin dekodiert byte-genau zum erwarteten Sample.
#   - Live: ZeroDDS encode -> Cyclone decode roundtrips ohne Mismatch.
#
# Voraussetzungen:
#   - cyclonedds-tools installiert (`ddsperf`, `cycdr`)
#   - rust-cargo (ZeroDDS-side Encoder/Decoder)
#
# Plattform: Linux mit IPv4-Multicast auf Loopback. macOS Docker
# Desktop tunnels Multicast nicht — dort SKIP.
#
# Verwendung:
#   ./tests/interop/xcdr2_cross_vendor.sh

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
FIXTURES_DIR="${REPO_ROOT}/crates/discovery/tests/fixtures/cyclone-xcdr2"

skip() {
    echo "SKIP: $1"
    exit 0
}

# Plattform-Check: macOS-Docker hat kein Multicast-Loopback.
if [[ "$(uname -s)" == "Darwin" ]]; then
    if [[ -n "${SKIP_LIVE_INTEROP:-}" ]]; then
        echo "INFO: macOS detected, live-multicast disabled. Static fixture check only."
    fi
fi

# 1. Static-Fixtures-Check.
if [[ ! -d "$FIXTURES_DIR" ]]; then
    echo "WARN: fixtures directory $FIXTURES_DIR missing — generate with"
    echo "  ./tests/interop/capture.sh xcdr2"
    echo "and re-run. Skipping static check."
else
    fixture_count=$( (ls -1 "$FIXTURES_DIR" 2>/dev/null || true) | (grep -E '^v[0-9]+\.bin$' || true) | wc -l | tr -d ' ')
    fixture_count=${fixture_count:-0}
    if [[ "$fixture_count" -eq 0 ]]; then
        echo "WARN: no fixtures in $FIXTURES_DIR — generate via capture.sh."
    else
        echo "INFO: $fixture_count Cyclone-recorded fixtures found"
        # zerodds-cdr decode test reads each fixture and asserts the
        # decoded sample matches the master-spec V-i sample.
        ( cd "$REPO_ROOT" && cargo test -p zerodds-cdr --test xcdr2_cross_vendor_fixtures ) \
            || { echo "FAIL: fixture decoder test failed"; exit 1; }
    fi
fi

# 2. Live-Roundtrip-Check (nur wenn Cyclone-Tools verfuegbar).
if ! command -v ddsperf >/dev/null 2>&1; then
    skip "ddsperf not in PATH — install cyclonedds-tools for live xcdr2 roundtrip"
fi

# Live-Pfad delegiert an existierendes Script.
echo "INFO: delegating live roundtrip to xv_pub_sub_roundtrip.sh"
"${REPO_ROOT}/tests/interop/xv_pub_sub_roundtrip.sh" || {
    echo "FAIL: live xcdr2 roundtrip failed"
    exit 1
}

echo "OK: xcdr2 cross-vendor conformance"
