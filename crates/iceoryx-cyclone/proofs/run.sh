#!/usr/bin/env bash
# Bidirectional ZeroDDS ↔ Cyclone DDS same-host zero-copy proof over the iceoryx
# C++ (POSH) shared-memory transport, using the zerodds-iceoryx-cyclone crate
# (libiceoryx_binding_c FFI) on both ends.
#
#   READ  : a Cyclone publisher → the ZeroDDS reader bin reads it   (READER OK)
#   WRITE : the ZeroDDS writer bin → a Cyclone subscriber reads it  (CYCLONE OK)
#
# Linux host with iceoryx POSH + iox-roudi + Cyclone-iox (e.g. codepit). The
# Cyclone URI configures the installed psmx_iox: INSTANCE_NAME is the iceoryx
# service string, ALLOW_NONDISCOVERED_WRITERS lets Cyclone accept the ZeroDDS
# writer (which is not a discovered DDS writer). NOTE: do NOT also use the legacy
# <SharedMemory><Enable> element — it spawns a shadow default PSMX that shadows
# this config.
set -euo pipefail
HERE="$(cd "$(dirname "$0")" && pwd)"
CRATE="$(cd "$HERE/.." && pwd)"
: "${CYCLONE:=/opt/cyclone}"
export LD_LIBRARY_PATH="$CYCLONE/lib:/usr/lib/x86_64-linux-gnu:${LD_LIBRARY_PATH:-}"
# Two configs: ALLOW_NONDISCOVERED_WRITERS (needed so Cyclone accepts the ZeroDDS
# writer) also makes Cyclone's OWN publisher's writer iox-ineligible, so the READ
# direction uses the plain config and the WRITE direction the allow-nondisc one.
READ_URI="file://$HERE/cyclone_read.xml"
WRITE_URI="file://$HERE/cyclone_write.xml"

cd "$HERE"
"$CYCLONE/bin/idlc" sample.idl
gcc cyclone_pub.c sample.c -I. -I"$CYCLONE/include" -L"$CYCLONE/lib" -lddsc -o cyclone_pub
gcc cyclone_sub.c sample.c -I. -I"$CYCLONE/include" -L"$CYCLONE/lib" -lddsc -o cyclone_sub
( cd "$CRATE" && cargo build --features posh )
READER="$CRATE/target/debug/zerodds-cyclone-iox-reader"
WRITER="$CRATE/target/debug/zerodds-cyclone-iox-writer"

pkill -f iox-roudi 2>/dev/null || true; sleep 1; rm -f /dev/shm/iceoryx* 2>/dev/null || true
iox-roudi -c roudi.toml >roudi.log 2>&1 &
ROUDI=$!
trap 'kill $ROUDI 2>/dev/null || true' EXIT
sleep 4

rc=0

echo "=== READ: Cyclone publisher → ZeroDDS reader ==="
CYCLONEDDS_URI="$READ_URI" ./cyclone_pub >cyclone_pub.log 2>&1 &
PUB=$!
sleep 2
"$READER" | grep -q "READER OK" && echo "  READ PASS" || { echo "  READ FAIL"; rc=1; }
kill $PUB 2>/dev/null || true
sleep 1

echo "=== WRITE: ZeroDDS writer → Cyclone subscriber ==="
CYCLONEDDS_URI="$WRITE_URI" ./cyclone_sub >cyclone_sub.log 2>&1 &
SUB=$!
sleep 2
"$WRITER" >/dev/null 2>&1 &
WR=$!
sleep 4
grep -q "CYCLONE OK" cyclone_sub.log && echo "  WRITE PASS" || { echo "  WRITE FAIL"; rc=1; }
kill $SUB $WR 2>/dev/null || true

[ $rc -eq 0 ] && echo "PROOF: bidirectional ZeroDDS ↔ Cyclone iceoryx SHM — PASS" \
             || echo "PROOF: bidirectional ZeroDDS ↔ Cyclone iceoryx SHM — FAIL"
exit $rc
