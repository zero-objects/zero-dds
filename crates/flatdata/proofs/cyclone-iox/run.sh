#!/usr/bin/env bash
# Cross-vendor same-host zero-copy proof: an external iceoryx (C++/POSH) consumer
# reads a Cyclone DDS sample over real iceoryx shared memory.
#
# Unlike the iceoryx2 oracle (ZeroDDS publishes, an iceoryx2-Rust consumer reads),
# this targets the OTHER iceoryx — the C++ POSH stack Cyclone DDS uses via its
# iox_psmx plugin, with the iox-roudi daemon. The consumer links ONLY the iceoryx
# C binding (libiceoryx_binding_c) and the reverse-engineered Cyclone PSMX
# user-header layout (../../../docs/interop/cyclone-iceoryx-shm-ground-truth.md) —
# no Cyclone/DDS library — and decodes both the dds_psmx_metadata_t header and the
# raw self-contained sample payload, byte-exact.
#
# Prereqs (present on the codepit bench host): iceoryx POSH v2.0.6 + iox-roudi +
# libiceoryx_binding_c headers; Cyclone DDS 0.11 with libpsmx_iox + idlc. This is
# a Linux proof (iceoryx POSH / RouDi is Linux). Run it where that stack lives.
set -euo pipefail
HERE="$(cd "$(dirname "$0")" && pwd)"
: "${CYCLONE:=/opt/cyclone}"
: "${IOX_INC:=/usr/include/iceoryx/v2.0.6}"

cd "$HERE"
"$CYCLONE/bin/idlc" sample.idl                      # IoxOracle::Sample (fixed → iox-eligible)
gcc pub.c sample.c -I. -I"$CYCLONE/include" -L"$CYCLONE/lib" -lddsc -o pub
gcc consumer.c -I"$IOX_INC" -liceoryx_binding_c -o consumer

# Fresh RouDi daemon.
pkill -f iox-roudi 2>/dev/null || true; sleep 1; rm -f /dev/shm/iceoryx* 2>/dev/null || true
iox-roudi -c roudi.toml >roudi.log 2>&1 &
ROUDI=$!
trap 'kill $ROUDI 2>/dev/null || true' EXIT
sleep 4

export LD_LIBRARY_PATH="$CYCLONE/lib:${LD_LIBRARY_PATH:-}"
export CYCLONEDDS_URI="file://$HERE/cyclone_iox.xml"
./pub >pub.log 2>&1 &
PUB=$!
sleep 2

if ./consumer | grep -q "ORACLE OK"; then
  echo "PROOF: external iceoryx(C++) consumer read a Cyclone sample over SHM — PASS"
  kill $PUB 2>/dev/null || true
  exit 0
else
  echo "PROOF: cross-vendor iceoryx SHM read — FAIL"
  kill $PUB 2>/dev/null || true
  exit 1
fi
