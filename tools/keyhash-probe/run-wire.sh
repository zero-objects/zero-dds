#!/bin/bash
# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 ZeroDDS Contributors
#
# run-wire.sh — does the KeyHash reach the wire at all?
#
# `run-all.sh` asks each vendor's type support what hash it computes.
# This asks a different question, and the two answers are not the same
# finding: a hash that is computed and never put in a DATA submessage is
# invisible to a peer. Loopback RTPS is captured for a keyed and a
# keyless topic per vendor and the capture is searched for
# `PID_KEY_HASH` (0x0070) inside the user DATA submessage.
#
#   tools/keyhash-probe/run-wire.sh
#
# Needs `tcpdump` and `tshark`, and root or `CAP_NET_RAW` — it captures
# on the loopback interface. The `wire` programs beside the probes are
# the publisher/subscriber pairs; they are built the same way `run-all.sh`
# builds the probes.
#
# ## What this replaces
#
# `/root/keyhash-probe/run-wire.sh`.
set -uo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$HERE/../.." && pwd)"
# shellcheck source=../interop-vendors.sh
. "$ROOT/tools/interop-vendors.sh"

work="${KEYHASH_WORK:-${TMPDIR:-/tmp}/keyhash-probe}"
iface="${KEYHASH_IFACE:-lo}"

for tool in tcpdump tshark; do
    command -v "$tool" >/dev/null 2>&1 || {
        echo "run-wire: $tool is not installed — no capture, 0 rows verified" >&2
        exit 1; }
done

# cap <pcapfile> <subcmd> <pubcmd>
cap() {
    local pcap=$1 sub=$2 pub=$3 td sb
    rm -f "$pcap"
    tcpdump -i "$iface" -w "$pcap" -s 0 udp >/dev/null 2>&1 & td=$!
    sleep 1
    eval "$sub" >/dev/null 2>&1 & sb=$!
    sleep 3
    eval "$pub" >/dev/null 2>&1
    sleep 1
    kill $sb 2>/dev/null; wait $sb 2>/dev/null || true
    kill -INT $td 2>/dev/null; wait $td 2>/dev/null || true
}

build_cyclone_wire() {
    local w="$work/cyclone-wire"; rm -rf "$w"; mkdir -p "$w"
    cp "$HERE/keyhash.idl" "$HERE/cyclone/wire.c" "$HERE/cyclone/cyclone-keyhash.xml" "$w/"
    ( cd "$w" && "$CYCLONE_HOME/bin/idlc" keyhash.idl > idlc.log 2>&1 \
      && gcc -O0 -g -o wire wire.c keyhash.c -I. -I"$CYCLONE_HOME/include" \
             -L"$CYCLONE_HOME/lib" -lddsc -Wl,-rpath,"$CYCLONE_HOME/lib" > cc.log 2>&1 ) \
      || { echo "run-wire: the Cyclone wire program did not build" >&2; return 1; }
    printf '%s' "$w"
}

build_fastdds_wire() {
    local w="$work/fastdds-wire"; rm -rf "$w"; mkdir -p "$w"
    cp "$HERE/keyhash.idl" "$HERE/fastdds/wire.cpp" "$w/"
    ( cd "$w" && "$FASTDDSGEN" -t . -replace keyhash.idl > gen.log 2>&1 \
      && g++ -std=c++17 -O0 -g -o wire wire.cpp keyhashPubSubTypes.cxx \
             keyhashTypeObjectSupport.cxx -I. -I"$FASTDDS_HOME/include" \
             -L"$FASTDDS_HOME/lib" -lfastcdr -lfastdds \
             -Wl,-rpath,"$FASTDDS_HOME/lib" > cc.log 2>&1 ) \
      || { echo "run-wire: the Fast DDS wire program did not build" >&2; return 1; }
    printf '%s' "$w"
}

# The writer entity ids differ per vendor, which is why the display
# filter is not shared: 0x…02 is a keyed user writer and 0x…03 a
# keyless one, and the vendor decides the high bytes.
report() { # <pcap> <filter>
    tshark -r "$1" -Y "$2" -T fields -e rtps.sm.wrEntityId -e rtps.param.id -e rtps.guid \
        2>/dev/null | sort -u | head -2
}

if vendor_probe cyclone && w="$(build_cyclone_wire)"; then
    echo "--- Cyclone (GenerateKeyhash=true) ---"
    export CYCLONEDDS_URI="file://$w/cyclone-keyhash.xml"
    for T in Foo NoKeyFinal NoKeyMutable; do
        cap "$w/cap-$T.pcap" "$w/wire sub $T" "$w/wire pub $T"
        printf '%s: ' "$T"
        report "$w/cap-$T.pcap" \
            "rtps.sm.id == 0x15 && (rtps.sm.wrEntityId == 0x00000202 || rtps.sm.wrEntityId == 0x00000203)"
    done
    unset CYCLONEDDS_URI
else
    vendor_skip cyclone 3
fi

if vendor_probe fastdds && w="$(build_fastdds_wire)"; then
    echo "--- Fast DDS ---"
    for T in Foo NoKeyFinal; do
        cap "$w/cap-$T.pcap" "$w/wire sub $T" "$w/wire pub $T"
        printf '%s: ' "$T"
        report "$w/cap-$T.pcap" \
            "rtps.sm.id == 0x15 && (rtps.sm.wrEntityId == 0x00000102 || rtps.sm.wrEntityId == 0x00000103)"
    done
else
    vendor_skip fastdds 2
fi

# RTI's half needs `rtiddsgen -example`, which writes the publisher and
# subscriber next to the type support; `rti-wire/` holds the two pairs
# after the edits that made them run unattended.
if vendor_probe rti; then
    echo "--- RTI Connext ---"
    w="$work/rti-wire"; rm -rf "$w"; mkdir -p "$w"
    cp "$HERE"/rti-wire/*.idl "$HERE"/rti-wire/*.c "$w/"
    ( cd "$w" || exit 1
      for t in keyed nokey; do
          "$NDDSHOME/bin/rtiddsgen" -language C -platform "$RTI_TARGET" -replace "$t.idl" > "gen-$t.log" 2>&1
          for side in publisher subscriber; do
              gcc -O0 -g -o "${t}_${side%lisher}" "${t}_${side}.c" "$t.c" "${t}Plugin.c" "${t}Support.c" \
                  -I. -I"$NDDSHOME/include" -I"$NDDSHOME/include/ndds" -L"$RTI_LIB" \
                  -lnddsc -lnddscore -ldl -lm -lpthread -lrt -Wl,-rpath,"$RTI_LIB" \
                  -DRTI_UNIX -DRTI_LINUX -DRTI_64BIT > "cc-$t-$side.log" 2>&1 || true
          done
      done )
    cap "$w/cap-keyed.pcap" "timeout 25 $w/keyed_sub 0 10" "timeout 15 $w/keyed_pub 0 5"
    printf 'Foo: '
    report "$w/cap-keyed.pcap" "rtps.sm.id == 0x15 && rtps.sm.wrEntityId == 0x80000002"
    cap "$w/cap-nokey.pcap" "timeout 25 $w/nokey_sub 0 10" "timeout 15 $w/nokey_pub 0 5"
    printf 'NoKeyFinal: '
    report "$w/cap-nokey.pcap" "rtps.sm.id == 0x15 && rtps.sm.wrEntityId == 0x80000003"
else
    vendor_skip rti 2
fi

vendor_report || true
