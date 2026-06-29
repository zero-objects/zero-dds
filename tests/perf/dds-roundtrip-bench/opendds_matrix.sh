#!/usr/bin/env bash
# Non-secure ZeroDDS <-> OpenDDS cross-vendor roundtrip matrix (DDSI-RTPS).
#
# OpenDDS is excluded from quick_matrix.sh/iso_matrix.sh's default vendor list,
# so this focused runner closes the OpenDDS interop axis: both cross-vendor
# directions + both self-baselines, full payload sweep (within the
# roundtrip.idl `sequence<octet, 8192>` bound). See
# internal/interop/opendds-interop-closeout.md.
#
# Prereq: zerodds-roundtrip + opendds-roundtrip built in ./build (see the
# closeout doc's Reproduction section). OpenDDS needs RTPS discovery +
# rtps_udp transport via `-DCPSConfigFile ../opendds_rtps.ini`.
#
# Usage: opendds_matrix.sh [OUT_CSV] [PAYLOADS="0 1638 4096 8192"] [N=2]
set -u

BENCH_DIR="$(cd "$(dirname "$0")" && pwd)"
BUILD_DIR="$BENCH_DIR/build"
ZD_REPO="$(cd "$BENCH_DIR/../../.." && pwd)"
CSV="${1:-$BENCH_DIR/opendds-out/opendds_matrix.csv}"
PAYLOADS_STR="${2:-0 1638 4096 8192}"
N_RUNS="${3:-2}"
read -r -a PAYLOADS <<<"$PAYLOADS_STR"
mkdir -p "$(dirname "$CSV")"

: "${OPENDDS_ROOT:=/opt/opendds}"
: "${CYCLONE_LIB:=/opt/cyclone/lib}"
export LD_LIBRARY_PATH="$OPENDDS_ROOT/lib:$CYCLONE_LIB:$ZD_REPO/target/release:${LD_LIBRARY_PATH:-}"
export PATH="$OPENDDS_ROOT/bin:$PATH"
INI="-DCPSConfigFile $BENCH_DIR/opendds_rtps.ini"

cd "$BUILD_DIR" || { echo "build dir fehlt: $BUILD_DIR (siehe closeout-Doc)" >&2; exit 1; }
for v in zerodds opendds; do
    [ -x "./${v}-roundtrip" ] || { echo "Binary fehlt: ${v}-roundtrip" >&2; exit 1; }
done

echo "ping,pong,payload,run,p50,p99,status" > "$CSV"
DOM=140
cell() { # ping pong payload run
    local ping=$1 pong=$2 pl=$3 run=$4
    pkill -9 -x zerodds-roundtrip 2>/dev/null
    pkill -9 -x opendds-roundtrip 2>/dev/null
    sleep 3
    local pc="" gc=""
    [ "$pong" = opendds ] && pc="$INI"
    [ "$ping" = opendds ] && gc="$INI"
    ZERODDS_BENCH_DOMAIN=$DOM ./"${pong}-roundtrip" pong 40 $pc >/tmp/odm_pong.log 2>&1 &
    local pp=$!
    [ "$pong" = opendds ] && sleep 8 || sleep 4
    local L
    L=$(ZERODDS_BENCH_DOMAIN=$DOM timeout 70 ./"${ping}-roundtrip" ping \
        --payload "$pl" --samples 2000 --warmup 200 $gc 2>&1 | grep payload=)
    kill -9 "$pp" 2>/dev/null
    wait "$pp" 2>/dev/null
    DOM=$((DOM + 1))
    local p50 p99
    p50=$(echo "$L" | grep -oE 'p50=[0-9.]+' | cut -d= -f2)
    p99=$(echo "$L" | grep -oE 'p99=[0-9.]+' | cut -d= -f2)
    if [ -z "$p50" ]; then
        echo "$ping,$pong,$pl,$run,,,RED" >>"$CSV"
        printf "%-9s -> %-9s p=%-5s r%s : RED\n" "$ping" "$pong" "$pl" "$run"
    else
        echo "$ping,$pong,$pl,$run,$p50,$p99,ok" >>"$CSV"
        printf "%-9s -> %-9s p=%-5s r%s : p50=%s p99=%s\n" "$ping" "$pong" "$pl" "$run" "$p50" "$p99"
    fi
}

for pl in "${PAYLOADS[@]}"; do
    for run in $(seq 1 "$N_RUNS"); do
        cell zerodds opendds "$pl" "$run"
        cell opendds zerodds "$pl" "$run"
    done
done
echo
echo "=== rote Zellen ==="
grep RED "$CSV" || echo "KEINE — ZeroDDS<->OpenDDS interop voll gruen (0..8192)"
echo "CSV: $CSV"
