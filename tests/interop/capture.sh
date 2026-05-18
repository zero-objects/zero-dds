#!/usr/bin/env bash
# tests/interop/capture.sh
#
# Hilfs-Skript zum Capture von Cyclone-DDS-RTPS-Frames mit tshark.
# Schreibt PCAP nach $OUT (default: cyclone_dump.pcap) und exportiert
# parallel ein Hex-Dump aller DATA-Submessages nach $HEX_OUT.
#
# Voraussetzungen:
#   - tshark installiert (Wireshark/CLI)
#   - sudo-Rechte fuer Live-Capture
#   - tests/interop/docker-compose.yml laeuft (docker compose up -d)
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

# Filter: nur RTPS-Datagrams mit DATA-Submessage
tshark -r "$OUT" -Y "rtps.sm.id == 0x15" -T fields -e data \
    > "$HEX_OUT" || true

echo ""
echo "Done. Konvertiere ein Frame in eine Fixture mit:"
echo "  head -1 $HEX_OUT | xxd -r -p > frame.bin"
echo "  xxd frame.bin   # Hex-Inspektion"
echo ""
echo "Manuelle Aufnahme als Fixture:"
echo "  - Bytes in crates/rtps/tests/fixtures/cyclone/<name>.hex einfuegen"
echo "  - Header-Bytes mit '#' kommentieren"
echo "  - Test in crates/rtps/tests/cyclone_compliance.rs ergaenzen"
