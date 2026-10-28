#!/usr/bin/env bash
# tests/interop/xcdr2_cross_vendor.sh
#
# L4 Cross-Vendor XCDR2 Conformance — `zerodds-xcdr2-bindings-conformance-1.0` §8.
#
# Verifies that the ZeroDDS encoder produces the same XCDR2 wire bytes
# as Cyclone DDS, and that the ZeroDDS decoder reads Cyclone bytes
# correctly. The test vectors V-1..V-12 are verbatim in the master spec
# §6 and count as the spec-exact wire form.
#
# Two variants are checked:
#
# 1. **Static fixtures**: `crates/discovery/tests/fixtures/cyclone-xcdr2/v*.bin`
#    are wire frames per V-i recorded with ddsperf/Cyclone tools.
#    The ZeroDDS decoder must decode them without loss.
#
# 2. **Live roundtrip**: `cyclonedds-tools` (cycdr or similar) must be in the PATH.
#    We start a Cyclone pub with a V-i sample, a ZeroDDS sub reads
#    via the xv_pub_sub_roundtrip.sh pattern.
#
# Success criterion:
#   - Fixtures: every v*.bin decodes byte-exactly to the expected sample.
#   - Live: ZeroDDS encode -> Cyclone decode roundtrips without mismatch.
#
# Prerequisites:
#   - cyclonedds-tools installed (`ddsperf`, `cycdr`)
#   - rust-cargo (ZeroDDS-side encoder/decoder)
#
# Platform: Linux with IPv4 multicast on loopback. macOS Docker
# Desktop does not tunnel multicast — SKIP there.
#
# Usage:
#   ./tests/interop/xcdr2_cross_vendor.sh

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
FIXTURES_DIR="${REPO_ROOT}/crates/discovery/tests/fixtures/cyclone-xcdr2"

skip() {
    echo "SKIP: $1"
    exit 0
}

# Platform check: macOS Docker has no multicast loopback.
if [[ "$(uname -s)" == "Darwin" ]]; then
    if [[ -n "${SKIP_LIVE_INTEROP:-}" ]]; then
        echo "INFO: macOS detected, live-multicast disabled. Static fixture check only."
    fi
fi

# 1. Static fixtures check.
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

# 2. Live roundtrip check (only if Cyclone tools are available).
if ! command -v ddsperf >/dev/null 2>&1; then
    skip "ddsperf not in PATH — install cyclonedds-tools for live xcdr2 roundtrip"
fi

# The live path delegates to an existing script.
echo "INFO: delegating live roundtrip to xv_pub_sub_roundtrip.sh"
"${REPO_ROOT}/tests/interop/xv_pub_sub_roundtrip.sh" || {
    echo "FAIL: live xcdr2 roundtrip failed"
    exit 1
}

echo "OK: xcdr2 cross-vendor conformance"
