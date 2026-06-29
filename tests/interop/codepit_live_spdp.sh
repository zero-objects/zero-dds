#!/usr/bin/env bash
# Codepit-Live-SPDP-Interop — startet Vendor-Publisher (Cyclone/RTI),
# laesst ZeroDDS-spdp_demo discover, prueft ob Vendor-Participants in
# der Discovery-Liste auftauchen.
#
# Vorbedingungen (auf codepit):
#  * Cyclone DDS in /opt/cyclone (built from source, 0.10.5)
#  * RTI Connext DDS in /opt/rti.com/rti_connext_dds-7.3.0
#  * rti_license.dat in $NDDSHOME oder /root/rti_license.dat
#  * Fast-DDS in /opt/fastdds (optional, falls vorhanden)
#  * cargo build --release -p zerodds-discovery --example spdp_demo
#
# Aufruf:
#   ./tests/interop/codepit_live_spdp.sh [domain]
#
# Exit-Codes:
#   0 — alle vorhandenen Vendoren wurden discovered
#   1 — mindestens ein vorhandener Vendor nicht discovered
#   2 — Setup-Fehler

set -uo pipefail

DOMAIN="${1:-0}"
LOG_DIR="$(mktemp -d)"
trap 'rm -rf "$LOG_DIR"; pkill -P $$ 2>/dev/null; true' EXIT
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
DEMO="$ROOT/target/release/examples/spdp_demo"

if [ ! -x "$DEMO" ]; then
    echo "ERR: $DEMO nicht gebaut. Run: cargo build --release -p zerodds-discovery --example spdp_demo" >&2
    exit 2
fi

# ---- Cyclone-Pub ----
EXPECT_VENDORS=()
if [ -x /opt/cyclone/bin/ddsperf ]; then
    EXPECT_VENDORS+=("1, 240")  # Eclipse Cyclone
    LD_LIBRARY_PATH=/opt/cyclone/lib timeout 15 \
        /opt/cyclone/bin/ddsperf -D "$DOMAIN" pub 1Hz \
        >"$LOG_DIR/cyclone.log" 2>&1 &
    echo "Started Cyclone ddsperf (PID $!)"
fi

# ---- RTI-Pub ----
NDDSHOME=/opt/rti.com/rti_connext_dds-7.3.0
if [ -x "$NDDSHOME/bin/rtiddsping" ]; then
    EXPECT_VENDORS+=("1, 16")  # RTI Connext
    export NDDSHOME
    export LD_LIBRARY_PATH=$NDDSHOME/lib/x64Linux4gcc7.3.0:${LD_LIBRARY_PATH:-}
    timeout 15 "$NDDSHOME/bin/rtiddsping" -domainId "$DOMAIN" -publisher \
        >"$LOG_DIR/rti.log" 2>&1 &
    echo "Started RTI rtiddsping (PID $!)"
fi

# ---- Fast-DDS-Pub ----
FASTDDS_HW=/opt/fastdds/examples/cpp/dds/HelloWorldExample/bin/DDSHelloWorldExample
if [ -x "$FASTDDS_HW" ]; then
    EXPECT_VENDORS+=("1, 15")  # eProsima Fast-DDS
    export LD_LIBRARY_PATH=/opt/fastdds/lib:${LD_LIBRARY_PATH:-}
    timeout 15 "$FASTDDS_HW" publisher \
        >"$LOG_DIR/fastdds.log" 2>&1 &
    echo "Started Fast-DDS DDSHelloWorldExample (PID $!)"
fi

if [ ${#EXPECT_VENDORS[@]} -eq 0 ]; then
    echo "ERR: kein Vendor installiert (Cyclone/RTI/Fast-DDS)" >&2
    exit 2
fi

# Vendoren brauchen einen Tick fuer SPDP-Beacon-Setup.
sleep 2

# ---- ZeroDDS SPDP-Demo ----
timeout 10 "$DEMO" 2>&1 | tee "$LOG_DIR/zerodds.log"

# ---- Auswertung ----
echo ""
echo "=== Auswertung ==="
RC=0
for v in "${EXPECT_VENDORS[@]}"; do
    if grep -q "vendor=VendorId(\[$v\])" "$LOG_DIR/zerodds.log"; then
        echo "✓ Vendor [$v] discovered"
    else
        echo "✗ Vendor [$v] NICHT discovered" >&2
        RC=1
    fi
done

exit $RC
