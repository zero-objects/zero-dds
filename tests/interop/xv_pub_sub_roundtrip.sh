#!/usr/bin/env bash
# tests/interop/xv_pub_sub_roundtrip.sh
#
# Cross-vendor pub/sub roundtrip — strict pass/fail test.
#
# Unlike `matrix.sh` (SPDP discovery only), this test checks
# actual **sample delivery**: the ZeroDDS pub must reach the Cyclone sub with
# data AND vice versa. The same for FastDDS as far as `fastdds_sub` is
# available (today only the pub exists — TODO below).
#
# Success criterion:
#   - At least MIN_SAMPLES (default 5) samples received per direction
#   - Both processes complete their observation windows without errors.
# This legacy harness counts delivery, not payload byte identity.
#
# Prerequisites (preinstalled in the CI image $RUST_IMAGE):
#   - cyclonedds-tools + cyclonedds-python (Cyclone pub/sub)
#   - libfastrtps-dev + libfastcdr-dev + g++ (FastDDS pub build)
#   - rust-cargo (ZeroDDS examples)
#
# Platform: Linux with working IPv4 multicast on loopback.
# macOS Docker Desktop does NOT tunnel multicast — SKIP there.
#
# Usage:
#   ./tests/interop/xv_pub_sub_roundtrip.sh
#
# Env:
#   RUNTIME_SECS  - test duration per direction (default: 12)
#   MIN_SAMPLES   - minimum number of received samples (default: 5)
#   TOPIC         - Square/Circle/Triangle (default: Square)
#   DOMAIN        - DDS domain (default: 62)
#   OUTDIR        - where logs go (default: ./xv-roundtrip-out)
#
# Exit codes:
#   0 — both directions successful
#   1 — at least one direction failed
#   2 — setup error (tools missing)
#   77 — SKIP (platform unsupported)

set -euo pipefail

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
# other form. This runner meets Cyclone DDS, so every pair reads tolerantly; the vendors whose writers use the rounded form (taken from the
# recorded rows: ast-layout Dialect::Cyclone and Dialect::Rti,
# pads_declared_length) get a ZeroDDS reader with wire.pl_cdr1_reading=tolerant;
# Fast DDS and OpenDDS write the exact length. This is the only place the runner
# sets it.
export ZERODDS_WIRE_PL_CDR1_READING=tolerant

HERE="$(cd "$(dirname "$0")" && pwd)"
REPO="$(cd "$HERE/../.." && pwd)"
OUTDIR="${OUTDIR:-$HERE/xv-roundtrip-out}"
RUNTIME_SECS="${RUNTIME_SECS:-12}"
MIN_SAMPLES="${MIN_SAMPLES:-5}"
TOPIC="${TOPIC:-Square}"
# The domain was written as the literal `0` in four places. Domain 0 is
# the one every unconfigured participant on the host takes, so a stray
# process writes into the same topics: this test then counts that
# process's samples as its evidence, or loses its own to it. Measured on
# the Linux test host: with a foreign participant on port 7400
# (= domain 0), direction 1 reported 0 samples and direction 2 reported
# 118 — neither number says anything about ZeroDDS and Cyclone.
#
# 62 is in `zerodds_test_domains::RESERVED_DOMAINS`, so the leasing crate
# never hands it out, and no other harness in the tree names it.
DOMAIN="${DOMAIN:-62}"

# --- Platform check ---
if [[ "$(uname -s)" != "Linux" ]]; then
    echo "[xv-roundtrip] SKIP: cross-vendor multicast needs Linux." >&2
    exit 77
fi

# --- Tool check ---
need_tool() {
    if ! command -v "$1" >/dev/null 2>&1; then
        echo "[xv-roundtrip] missing tool: $1 (install via apt: $2)" >&2
        return 2
    fi
}
need_tool cargo "rustup component" || exit 2
need_tool python3 "python3" || exit 2
need_tool timeout "coreutils" || exit 2

if ! python3 -c "import cyclonedds" 2>/dev/null; then
    echo "[xv-roundtrip] SETUP FAILED — missing python module: cyclonedds" >&2
    echo "[xv-roundtrip]   Install: apt install python3-cyclonedds" >&2
    echo "[xv-roundtrip]   (or pip install cyclonedds — needs libcycloneddsil)" >&2
    exit 2
fi

# Subscribers are intentionally long-running. Only the timed observation
# window (124) or normal completion (0) is acceptable, not a crash/setup error.
observe_window() {
    local rc=0
    timeout "${RUNTIME_SECS}s" "$@" || rc=$?
    if [[ "$rc" != 0 && "$rc" != 124 ]]; then
        echo "[xv-roundtrip] observation process failed: rc=$rc" >&2
        return "$rc"
    fi
}

mkdir -p "$OUTDIR"
cleanup() {
    local pid
    for pid in "${ZD_PUB_PID:-}" "${CY_PUB_PID:-}"; do
        if [[ -n "$pid" ]]; then
            kill -TERM "$pid" 2>/dev/null || :
            wait "$pid" 2>/dev/null || :
        fi
    done
}
trap cleanup EXIT

# --- Step 1: build the ZeroDDS examples ---
echo "[xv-roundtrip] cargo build --example shapes_demo_publisher --example shapes_demo_subscriber"
cargo build -p zerodds-dcps --example shapes_demo_publisher --example shapes_demo_subscriber \
    >"$OUTDIR/build.log" 2>&1
ZD_PUB="$REPO/target/debug/examples/shapes_demo_publisher"
ZD_SUB="$REPO/target/debug/examples/shapes_demo_subscriber"

# --- Direction 1: ZeroDDS-Pub → Cyclone-Sub ---
echo "[xv-roundtrip] direction 1/2: ZeroDDS-Pub → Cyclone-Sub"
"$ZD_PUB" "$TOPIC" PURPLE "$DOMAIN" >"$OUTDIR/zd_pub_to_cy.log" 2>&1 &
ZD_PUB_PID=$!
sleep 1
observe_window python3 "$HERE/cyclone_shapes_sub.py" "$TOPIC" "$DOMAIN" \
    >"$OUTDIR/cy_sub_from_zd.log" 2>&1
kill -0 "$ZD_PUB_PID" || { echo "ZeroDDS publisher exited during observation" >&2; exit 1; }
kill -TERM "$ZD_PUB_PID" 2>/dev/null || true
wait "$ZD_PUB_PID" 2>/dev/null || true
ZD_PUB_PID=

CY_RX_COUNT=$(grep -c '<- color=' "$OUTDIR/cy_sub_from_zd.log" || true)
echo "[xv-roundtrip] Cyclone-Sub received from ZeroDDS-Pub: $CY_RX_COUNT samples"

# --- Direction 2: Cyclone-Pub → ZeroDDS-Sub ---
echo "[xv-roundtrip] direction 2/2: Cyclone-Pub → ZeroDDS-Sub"
python3 "$HERE/cyclone_shapes_pub.py" "$TOPIC" GREEN "$DOMAIN" \
    >"$OUTDIR/cy_pub_to_zd.log" 2>&1 &
CY_PUB_PID=$!
sleep 1
observe_window "$ZD_SUB" "$TOPIC" "$DOMAIN" \
    >"$OUTDIR/zd_sub_from_cy.log" 2>&1
kill -0 "$CY_PUB_PID" || { echo "Cyclone publisher exited during observation" >&2; exit 1; }
kill -TERM "$CY_PUB_PID" 2>/dev/null || true
wait "$CY_PUB_PID" 2>/dev/null || true
CY_PUB_PID=

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
