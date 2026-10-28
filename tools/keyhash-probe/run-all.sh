#!/bin/bash
# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 ZeroDDS Contributors
#
# run-all.sh — the KeyHash each vendor computes, for sixty-odd types.
#
#   tools/keyhash-probe/run-all.sh
#   tools/keyhash-probe/run-all.sh cyclone
#
# One `keyhash.idl` holds every shape the RTPS §9.6.4.8 rule can be
# asked about: no key at all, one key, a key wider than sixteen octets,
# a key inside a nested struct, a key inside a nested struct inside a
# keyless one, `@key` on a union discriminator, and the same set under
# `@final`, `@appendable` and `@mutable`. Each vendor's own type
# support computes the hash; nothing here computes one.
#
# The three probes print `<type> <hash>` lines. They disagree, and where
# they do the divergence is a row of `docs/wire-feature-inventory.md`.
#
# ## What this replaces
#
# `/root/keyhash-probe/run-all.sh`, which named the harness directory
# and each vendor prefix absolutely.
#
# The generated type support is not in the tree: `idlc`, `fastddsgen`
# and `rtiddsgen` write it from `keyhash.idl` every run, and a copy in
# git would be a second thing to keep in step with the IDL.
set -uo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$HERE/../.." && pwd)"
# shellcheck source=../interop-vendors.sh
. "$ROOT/tools/interop-vendors.sh"

work="${KEYHASH_WORK:-${TMPDIR:-/tmp}/keyhash-probe}"
want="${1:-cyclone fastdds rti opensplice}"

run_cyclone() {
    local w="$work/cyclone"; rm -rf "$w"; mkdir -p "$w"
    cp "$HERE/keyhash.idl" "$HERE/cyclone/probe.c" "$w/"
    ( cd "$w" \
      && "$CYCLONE_HOME/bin/idlc" keyhash.idl > idlc.log 2>&1 \
      && gcc -O0 -g -o probe probe.c keyhash.c -I. -I"$CYCLONE_HOME/include" \
             -L"$CYCLONE_HOME/lib" -lddsc -Wl,-rpath,"$CYCLONE_HOME/lib" > cc.log 2>&1 ) \
      || { echo "keyhash: the Cyclone probe did not build" >&2
           grep -m5 -i error "$w"/*.log >&2; return 1; }
    "$w/probe"
}

run_fastdds() {
    local w="$work/fastdds"; rm -rf "$w"; mkdir -p "$w"
    cp "$HERE/keyhash.idl" "$HERE/fastdds/probe.cpp" "$w/"
    ( cd "$w" \
      && "$FASTDDSGEN" -t . -replace keyhash.idl > gen.log 2>&1 \
      && g++ -std=c++17 -O0 -g -o probe probe.cpp keyhashPubSubTypes.cxx \
             keyhashTypeObjectSupport.cxx -I. -I"$FASTDDS_HOME/include" \
             -L"$FASTDDS_HOME/lib" -lfastcdr -lfastdds \
             -Wl,-rpath,"$FASTDDS_HOME/lib" > cc.log 2>&1 ) \
      || { echo "keyhash: the Fast DDS probe did not build" >&2
           tail -3 "$w/gen.log" >&2; grep -m5 "error:" "$w/cc.log" >&2; return 1; }
    "$w/probe"
}

run_rti() {
    local w="$work/rti"; rm -rf "$w"; mkdir -p "$w"
    cp "$HERE/keyhash.idl" "$HERE/rti/probe2.c" "$w/"
    ( cd "$w" \
      && "$NDDSHOME/bin/rtiddsgen" -language C -platform "$RTI_TARGET" -replace keyhash.idl > gen.log 2>&1 \
      && gcc -O0 -g -o probe2 probe2.c keyhash.c keyhashPlugin.c keyhashSupport.c \
             -I. -I"$NDDSHOME/include" -I"$NDDSHOME/include/ndds" -L"$RTI_LIB" \
             -lnddsc -lnddscore -ldl -lm -lpthread -lrt -Wl,-rpath,"$RTI_LIB" \
             -DRTI_UNIX -DRTI_LINUX -DRTI_64BIT > cc.log 2>&1 ) \
      || { echo "keyhash: the RTI probe did not build" >&2
           grep -m5 -i error "$w"/*.log >&2; return 1; }
    "$w/probe2"
}

# OpenSplice computes the KeyHash inside its DDSI service and has no call
# that returns it, so its column is measured on the wire: `node pub` and
# `node sub` on a domain of their own (OSPL_KEYHASH_DOMAIN, default 77),
# a capture of the traffic, and `keyhashes.py` reading PID_KEY_HASH per
# topic out of it. Needs tcpdump and tshark, and the rights to capture.
# `set +u` in both subshells: release.com reads LD_LIBRARY_PATH unset.
run_opensplice() {
    local w="$work/opensplice"; rm -rf "$w"; mkdir -p "$w"
    local domain="${OSPL_KEYHASH_DOMAIN:-77}"
    cp "$HERE/opensplice/keyhash.idl" "$HERE/opensplice/node.c" \
       "$HERE/opensplice/keyhashes.py" "$w/"
    # shellcheck disable=SC1091
    ( set +u; cd "$w" \
      && . "$OSPL_HOME/release.com" >/dev/null 2>&1 \
      && sed "s#<Id>0</Id>#<Id>$domain</Id>#" \
             "$OSPL_HOME/etc/config/ospl_sp_ddsi.xml" > ospl.xml \
      && "$OSPL_HOME/bin/idlpp" -S -l c keyhash.idl > idlpp.log 2>&1 \
      && gcc -O1 -o node node.c keyhashSacDcps.c keyhashSplDcps.c -I. \
             -I"$OSPL_HOME/include/dcps/C/SAC" -I"$OSPL_HOME/include" \
             -I"$OSPL_HOME/include/sys" -L"$OSPL_HOME/lib" -ldcpssac -lddskernel \
             -Wl,-rpath,"$OSPL_HOME/lib" > cc.log 2>&1 ) \
      || { echo "keyhash: the OpenSplice probe did not build" >&2
           grep -m5 -i error "$w"/*.log >&2; return 1; }
    local lo=$((7400 + 250 * domain)) hi=$((7400 + 250 * domain + 249))
    # shellcheck disable=SC1091
    ( set +u; cd "$w" \
      && . "$OSPL_HOME/release.com" >/dev/null 2>&1 \
      && export OSPL_URI="file://$w/ospl.xml" \
      && { tcpdump -i any -s 0 -w capture.pcap "udp portrange $lo-$hi" > tcpdump.log 2>&1 & } \
      && sleep 1 \
      && { ./node sub 10 > sub.log 2>&1 & } \
      && sleep 2 \
      && ./node pub 6 > pub.log 2>&1 \
      && sleep 3 \
      && pkill -INT -f "tcpdump -i any -s 0 -w capture.pcap" \
      && sleep 1 \
      && python3 keyhashes.py capture.pcap \
         | grep -E "^(PadKey|WideKey|CharKey|StrKey|PathKey|BigKey) " | sort )
}

for vendor in $want; do
    if ! vendor_probe "$vendor"; then
        # 60-odd types, one hash each: the number a missing vendor costs.
        vendor_skip "$vendor" "$(grep -c '^ *@\?[a-z]* *struct\|^struct' "$HERE/keyhash.idl" 2>/dev/null || echo 0)"
        continue
    fi
    printf '===================== %s =====================\n' "$vendor"
    "run_$vendor" || true
done

vendor_report || true
