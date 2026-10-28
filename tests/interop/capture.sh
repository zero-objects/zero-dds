#!/usr/bin/env bash
# tests/interop/capture.sh
#
# Helper script to capture Cyclone DDS RTPS frames with tshark.
# Writes the PCAP to $OUT (default: cyclone_dump.pcap) and in parallel
# exports a hex dump of all DATA submessages to $HEX_OUT.
#
# Prerequisites:
#   - tshark installed (Wireshark/CLI)
#   - sudo rights for live capture
#   - tests/interop/docker-compose.yml running (docker compose up -d)
#
# Usage:
#   ./tests/interop/capture.sh [count] [out.pcap]

set -euo pipefail

COUNT="${1:-50}"
OUT="${2:-cyclone_dump.pcap}"
HEX_OUT="${OUT%.pcap}.hex"
IFACE="${IFACE:-any}"
PORTS="udp port 7400 or udp port 7410 or udp port 7411 or udp port 7412"

if ! command -v tshark >/dev/null 2>&1; then
  echo "ERROR: tshark not found in PATH" >&2
  exit 1
fi

echo "Capturing $COUNT Cyclone-DDS RTPS frames on iface=$IFACE..."
sudo tshark -i "$IFACE" -f "$PORTS" -w "$OUT" -c "$COUNT"

echo ""
echo "Capture written to: $OUT"
echo "Exporting hex of all RTPS DATA submessages to: $HEX_OUT"

# Filter: only RTPS datagrams with a DATA submessage
tshark -r "$OUT" -Y "rtps.sm.id == 0x15" -T fields -e data \
    > "$HEX_OUT" || true

echo ""
echo "Done. Convert one frame into a fixture with:"
echo "  head -1 $HEX_OUT | xxd -r -p > frame.bin"
echo "  xxd frame.bin   # hex inspection"
echo ""
echo "Manual capture as a fixture:"
echo "  - insert the bytes into crates/rtps/tests/fixtures/cyclone/<name>.hex"
echo "  - comment the header bytes with '#'"
echo "  - add a test in crates/rtps/tests/cyclone_compliance.rs"
