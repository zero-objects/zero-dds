#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
# Top-Level Runner fuer Cross-Vendor-Tests aller 7 Bridges.
# Spec: §10 + §12.3 jeder Bridge-Spec.

set -uo pipefail

# Empty ACKNACK of a reader that holds nothing: Fast DDS (and, as recorded, OpenDDS
# and RTI Connext) send it with bitmapBase 0 and numBits 0, which DDSI-RTPS 2.5
# section 8.3.5.5 makes invalid; ZeroDDS by default drops the rest of such a
# datagram (section 8.3.4.1 rule 6), and with it the SEDP data that follows in
# it (measured: sec-diag16/17, every red zerodds->fastdds cell had rejections
# with left_out > 0, every green cell none). The ZeroDDS processes of the
# vendor pairs, writers and readers, therefore accept it
# (wire.accept_empty_acknack_base_zero). This is the only place the runner sets it.
export ZERODDS_WIRE_ACCEPT_EMPTY_ACKNACK_BASE_ZERO=1
# PL_CDR1 parameters with the rounded length. Cyclone DDS (extended header) and
# RTI Connext (short header) write the length of a PL_CDR1 parameter rounded up
# to four, the form of DDSI-RTPS 2.5 section 9.4.2.11; XTypes 1.3 section
# 7.4.1.2.2 has the exact length and the default reader of ZeroDDS refuses the
# other form. This runner meets both vendors, so every pair reads tolerantly; the vendors whose writers use the rounded form (taken from the
# recorded rows: ast-layout Dialect::Cyclone and Dialect::Rti,
# pads_declared_length) get a ZeroDDS reader with wire.pl_cdr1_reading=tolerant;
# Fast DDS and OpenDDS write the exact length. This is the only place the runner
# sets it.
export ZERODDS_WIRE_PL_CDR1_READING=tolerant
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
