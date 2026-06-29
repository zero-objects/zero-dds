#!/usr/bin/env bash
# DCPS-level proof: a normal ZeroDDS typed DataWriter<KMsg>/DataReader<KMsg>,
# once enable_cyclone_iox()-ed, interoperates with Cyclone DDS over the iceoryx
# C++ (POSH) transport — both directions, for a variable-length, KEYED, @final
# type (IoxOracle::KMsg) carried as Cyclone's serialized PSMX form (classic CDR).
#
#   READ        : a Cyclone publisher of KMsg → DataReader<KMsg>::take()  → READER OK
#   WRITE       : DataWriter<KMsg>::write() (online RTPS participant) → a Cyclone
#                 subscriber that discovers the writer over SEDP and accepts its
#                 serialized iceoryx sample by GUID match                → CYCLONE OK
#   WRITE-bytes : DataWriter<KMsg>::write() → an external libiceoryx_binding_c
#                 consumer reads the exact serialized bytes ZeroDDS produced
#
# The WRITE leg needs RTPS discovery between ZeroDDS and Cyclone (domain 0, SPDP
# multicast) so the iceoryx chunk's metadata GUID (= the writer's real RTPS GUID)
# matches a writer Cyclone discovered — no ALLOW_NONDISCOVERED_WRITERS shortcut.
#
# Linux host with iceoryx POSH + iox-roudi + Cyclone-iox (e.g. codepit).
set -euo pipefail
HERE="$(cd "$(dirname "$0")" && pwd)"
: "${CYCLONE:=/opt/cyclone}"
: "${IOX_INC:=/usr/include/iceoryx/v2.0.6}"
export LD_LIBRARY_PATH="$CYCLONE/lib:/usr/lib/x86_64-linux-gnu:${LD_LIBRARY_PATH:-}"

cd "$HERE"
"$CYCLONE/bin/idlc" svar.idl
gcc pubk.c svar.c -I. -I"$CYCLONE/include" -L"$CYCLONE/lib" -lddsc -o pubk
gcc subk.c svar.c -I. -I"$CYCLONE/include" -L"$CYCLONE/lib" -lddsc -o subk
gcc capk.c -I"$IOX_INC" -liceoryx_binding_c -o capk
cargo build
PROOF="$HERE/target/debug/proof-cyclone-dcps"

pkill -f iox-roudi 2>/dev/null || true; sleep 1; rm -f /dev/shm/iceoryx* 2>/dev/null || true
iox-roudi -c ../roudi.toml >roudi.log 2>&1 &
ROUDI=$!
trap 'kill $ROUDI 2>/dev/null || true' EXIT
sleep 4
rc=0

echo "=== READ: Cyclone publisher → ZeroDDS DataReader<KMsg> (real take) ==="
CYCLONEDDS_URI="file://$HERE/../cyclone_read.xml" ./pubk >pubk.log 2>&1 &
PUB=$!
sleep 2
timeout 14 "$PROOF" read | grep -q "READER OK" && echo "  READ PASS" || { echo "  READ FAIL"; rc=1; }
kill $PUB 2>/dev/null || true; sleep 1

echo "=== WRITE: ZeroDDS DataWriter<KMsg> (online RTPS) → Cyclone subscriber ==="
CYCLONEDDS_URI="file://$HERE/cyclone_online.xml" ./subk >subk.log 2>&1 &
SUB=$!
sleep 2
timeout 25 "$PROOF" writeonline >zdw.log 2>&1 &
WR=$!
sleep 18
grep -q "CYCLONE OK" subk.log && echo "  WRITE PASS" || { echo "  WRITE FAIL"; rc=1; }
kill $SUB $WR 2>/dev/null || true

[ $rc -eq 0 ] && echo "PROOF: ZeroDDS DCPS DataWriter/DataReader ↔ Cyclone over iceoryx (typed, serialized, keyed, both directions) — PASS" \
             || echo "PROOF: FAIL"
exit $rc
