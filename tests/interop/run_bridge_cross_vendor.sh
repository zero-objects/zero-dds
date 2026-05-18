#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
# Top-Level Runner fuer Cross-Vendor-Tests aller 7 Bridges.
# Spec: §10 + §12.3 jeder Bridge-Spec.

set -uo pipefail
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/../.." && pwd)"

cd "$ROOT_DIR"

PASS=0
FAIL=0
SKIP=0
LOG="$SCRIPT_DIR/bridge_cross_vendor.log"
: > "$LOG"

run() {
    local crate="$1"; shift
    echo "=== Cross-Vendor: $crate ==="
    if cargo test -p "$crate" --features cross-vendor-tests "$@" -- --ignored 2>&1 | tee -a "$LOG" | tail -10; then
        PASS=$((PASS+1))
    else
        FAIL=$((FAIL+1))
    fi
}

run zerodds-coap-bridge      --test cross_vendor
run zerodds-mqtt-bridge      --test cross_vendor
run zerodds-amqp-endpoint    --test cross_vendor
run zerodds-grpc-bridge      --test cross_vendor
run zerodds-websocket-bridge --test cross_vendor
run zerodds-corba-dds-bridge --test cross_vendor
run zerodds-ros2-rmw         --test cross_vendor

echo
echo "============= bridges cross-vendor ============="
echo "PASS: $PASS  FAIL: $FAIL  SKIP: $SKIP"
echo "Log: $LOG"
[ "$FAIL" -eq 0 ]
