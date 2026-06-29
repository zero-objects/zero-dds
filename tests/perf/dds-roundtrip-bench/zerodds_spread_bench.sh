#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
#
# Gezielter Spread-Bench: nur zerodds-self ueber wenige Payloads, viele
# Runs pro Cell. Vergleicht verschiedene UDP-Cache-Konfigurationen
# (via ZERODDS_UDP_CACHE_*-Env-Vars) ohne rebuild.
#
# Aufruf:
#   zerodds_spread_bench.sh <out_dir> [N=20] [PAYLOADS="0 1638 4096 6554 8192"]
#
# CSV-Output: cache_mode,payload_bytes,run_idx,p50_us,min_us,max_us
#
# Varianten:
#   cache_on   = Default (Cache an, SNDBUF=256K, max=16)
#   cache_off  = ZERODDS_UDP_CACHE_ENABLE=0 (klassisches send_to)
#   cache_default_sndbuf = ZERODDS_UDP_CACHE_SNDBUF=0 (Kernel-Default)

set -u

BENCH_DIR="$(cd "$(dirname "$0")" && pwd)"
BUILD_DIR="$BENCH_DIR/build"
ZD_REPO="$(cd "$BENCH_DIR/../../.." && pwd)"
OUT_DIR="${1:-$BENCH_DIR/spread-out}"
N_RUNS="${2:-20}"
PAYLOADS_STR="${3:-0 1638 4096 6554 8192}"
read -r -a PAYLOADS <<<"$PAYLOADS_STR"
SAMPLES=2000
WARMUP=200

mkdir -p "$OUT_DIR"
CSV="$OUT_DIR/zerodds_spread.csv"
echo "cache_mode,payload_bytes,run_idx,n,min_us,p50_us,p90_us,p99_us,p999_us,max_us,status" >"$CSV"

if [ "$(uname)" = Linux ]; then
    : "${LD_LIBRARY_PATH:=/opt/cyclone/lib:/opt/fastdds/lib:/opt/rti.com/rti_connext_dds-7.7.0/lib/x64Linux4gcc8.5.0:$ZD_REPO/target/release}"
    export LD_LIBRARY_PATH
fi

cd "$BUILD_DIR" || exit 1
[ -x "./zerodds-roundtrip" ] || { echo "binary missing"; exit 1; }

run_one() {
    local mode=$1
    local payload=$2
    local idx=$3
    local pong_log ping_log
    pong_log=$(mktemp); ping_log=$(mktemp)
    ./zerodds-roundtrip pong 30 >"$pong_log" 2>&1 &
    local pong_pid=$!
    sleep 3
    timeout 90 ./zerodds-roundtrip ping --payload "$payload" \
        --samples "$SAMPLES" --warmup "$WARMUP" >"$ping_log" 2>&1
    kill "$pong_pid" 2>/dev/null
    sleep 0.3
    kill -9 "$pong_pid" 2>/dev/null
    wait "$pong_pid" 2>/dev/null
    local line
    line=$(grep "payload=" "$ping_log" | head -1)
    rm -f "$pong_log" "$ping_log"
    if [ -z "$line" ]; then
        echo "$mode,$payload,$idx,0,,,,,,,timeout" >>"$CSV"
        return 1
    fi
    local n mn p50 p90 p99 p999 mx
    n=$(printf '%s' "$line" | grep -oE 'n=[0-9.]+' | head -1 | cut -d= -f2)
    mn=$(printf '%s' "$line" | grep -oE 'min=[0-9.]+' | head -1 | cut -d= -f2)
    p50=$(printf '%s' "$line" | grep -oE 'p50=[0-9.]+' | head -1 | cut -d= -f2)
    p90=$(printf '%s' "$line" | grep -oE 'p90=[0-9.]+' | head -1 | cut -d= -f2)
    p99=$(printf '%s' "$line" | grep -oE 'p99=[0-9.]+' | head -1 | cut -d= -f2)
    p999=$(printf '%s' "$line" | grep -oE 'p999=[0-9.]+' | head -1 | cut -d= -f2)
    mx=$(printf '%s' "$line" | grep -oE 'max=[0-9.]+' | head -1 | cut -d= -f2)
    echo "$mode,$payload,$idx,$n,$mn,$p50,$p90,$p99,$p999,$mx,ok" >>"$CSV"
    return 0
}

run_mode() {
    local mode_name=$1
    shift
    echo "===== Mode: $mode_name ($* set) ====="
    "$@" env -i HOME="$HOME" PATH="$PATH" LD_LIBRARY_PATH="$LD_LIBRARY_PATH" \
        bash -c "
        $(declare -f run_one)
        SAMPLES=$SAMPLES; WARMUP=$WARMUP
        for payload in ${PAYLOADS[@]}; do
            for idx in \$(seq 1 $N_RUNS); do
                printf '[%s] payload=%-5d run=%2d ... ' '$mode_name' \"\$payload\" \"\$idx\"
                if run_one '$mode_name' \"\$payload\" \"\$idx\"; then
                    p50=\$(tail -1 '$CSV' | cut -d, -f6)
                    echo \"p50=\${p50} us\"
                else
                    echo TIMEOUT
                fi
                sleep 0.5
            done
        done
        " 2>&1
}

started=$(date +%s)

# Variante 1: Cache aus
ZERODDS_UDP_CACHE_ENABLE=0 \
run_mode "cache_off" env ZERODDS_UDP_CACHE_ENABLE=0

# Variante 2: Cache an + SNDBUF Default 256K (jetziger Default)
run_mode "cache_on_sndbuf_256k" env ZERODDS_UDP_CACHE_ENABLE=1 ZERODDS_UDP_CACHE_SNDBUF=262144

# Variante 3: Cache an + SNDBUF Kernel-Default (wie pre-refit)
run_mode "cache_on_sndbuf_default" env ZERODDS_UDP_CACHE_ENABLE=1 ZERODDS_UDP_CACHE_SNDBUF=0

elapsed=$(( $(date +%s) - started ))
echo
echo "Spread-Bench fertig in $((elapsed / 60)) min $((elapsed % 60)) s."
echo "CSV: $CSV"
