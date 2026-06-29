#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
#
# Spread-Diagnose: kombiniert phase-timing + perf-record + tcpdump
# fuer einen einzelnen zerodds-self-Roundtrip-Run, plus Vergleichs-
# Runs mit verschiedenen Konfigurationen.
#
# Erzeugt im OUT_DIR:
#   - phase_<mode>.log   — phase-timing-output pro Mode
#   - sample_p50_p99_<mode>.csv — pro run: min/p50/p90/p99/p999/max
#   - perf_<mode>.data   — perf record sample data (wenn perf da)
#   - pcap_<mode>.pcap   — tcpdump loopback capture (wenn tcpdump da)
#   - summary.md         — Zusammenfassung
#
# Aufruf:
#   spread_diagnose.sh <out_dir> [N=10] [PAYLOADS="0 8192"]

set -u

BENCH_DIR="$(cd "$(dirname "$0")" && pwd)"
BUILD_DIR="$BENCH_DIR/build"
ZD_REPO="$(cd "$BENCH_DIR/../../.." && pwd)"
OUT_DIR="${1:-$BENCH_DIR/spread-diag}"
N_RUNS="${2:-10}"
PAYLOADS_STR="${3:-0 8192}"
read -r -a PAYLOADS <<<"$PAYLOADS_STR"
SAMPLES=2000
WARMUP=200

mkdir -p "$OUT_DIR"
CSV="$OUT_DIR/samples.csv"
echo "mode,payload_bytes,run_idx,n,min_us,p50_us,p90_us,p99_us,p999_us,max_us,status" >"$CSV"

if [ "$(uname)" = Linux ]; then
    : "${LD_LIBRARY_PATH:=/opt/cyclone/lib:/opt/fastdds/lib:/opt/rti.com/rti_connext_dds-7.7.0/lib/x64Linux4gcc8.5.0:$ZD_REPO/target/release}"
    export LD_LIBRARY_PATH
fi
cd "$BUILD_DIR" || exit 1

HAVE_PERF=0
HAVE_TCPDUMP=0
command -v perf >/dev/null 2>&1 && HAVE_PERF=1
command -v tcpdump >/dev/null 2>&1 && HAVE_TCPDUMP=1
echo "perf available: $HAVE_PERF, tcpdump available: $HAVE_TCPDUMP"

run_cell() {
    local mode=$1 payload=$2 idx=$3 capture=$4
    local ping_log pong_log pcap_path perf_data
    pong_log=$(mktemp); ping_log=$(mktemp)
    pcap_path="$OUT_DIR/pcap_${mode}_p${payload}_r${idx}.pcap"
    perf_data="$OUT_DIR/perf_${mode}_p${payload}_r${idx}.data"

    # Start pong
    ./zerodds-roundtrip pong 60 >"$pong_log" 2>&1 &
    local pong_pid=$!
    sleep 3

    local tcpdump_pid=0
    local perf_pid=0
    if [ "$capture" = "1" ]; then
        if [ "$HAVE_TCPDUMP" = "1" ]; then
            tcpdump -i lo -w "$pcap_path" -B 4096 'udp and (port 7400 or portrange 7410-7800)' \
                >/dev/null 2>&1 &
            tcpdump_pid=$!
        fi
        if [ "$HAVE_PERF" = "1" ]; then
            # Sample pong-process at 999 Hz fuer ~2.5s (covers warmup+samples)
            perf record -F 999 -p "$pong_pid" -o "$perf_data" -g \
                >/dev/null 2>&1 &
            perf_pid=$!
        fi
    fi

    # Ping mit phase-timing
    ZERODDS_PHASE_TIMING=1 ZERODDS_PHASE_DUMP=1 \
        timeout 90 ./zerodds-roundtrip ping --payload "$payload" \
            --samples "$SAMPLES" --warmup "$WARMUP" >"$ping_log" 2>&1

    if [ "$tcpdump_pid" -gt 0 ]; then
        kill "$tcpdump_pid" 2>/dev/null
        wait "$tcpdump_pid" 2>/dev/null
    fi
    if [ "$perf_pid" -gt 0 ]; then
        kill -INT "$perf_pid" 2>/dev/null
        wait "$perf_pid" 2>/dev/null
    fi
    kill "$pong_pid" 2>/dev/null
    sleep 0.3
    kill -9 "$pong_pid" 2>/dev/null
    wait "$pong_pid" 2>/dev/null

    # Parse + dump
    local line phase_log
    line=$(grep "payload=" "$ping_log" | head -1)
    phase_log="$OUT_DIR/phase_${mode}_p${payload}_r${idx}.log"
    grep "ZERODDS_PHASE" "$ping_log" >"$phase_log"
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
    local mode=$1
    shift
    local env_str="$*"
    echo "===== Mode: $mode ($env_str) ====="
    for payload in "${PAYLOADS[@]}"; do
        for idx in $(seq 1 "$N_RUNS"); do
            # Capture nur fuer ersten run pro payload+mode
            local cap=0
            [ "$idx" = "1" ] && cap=1
            printf '[%-30s] payload=%-5d run=%2d ... ' "$mode" "$payload" "$idx"
            local rc
            env -i HOME="$HOME" PATH="$PATH" LD_LIBRARY_PATH="$LD_LIBRARY_PATH" $env_str \
                bash -c "$(declare -f run_cell); HAVE_TCPDUMP=$HAVE_TCPDUMP HAVE_PERF=$HAVE_PERF \
                    OUT_DIR='$OUT_DIR' SAMPLES=$SAMPLES WARMUP=$WARMUP CSV='$CSV' \
                    run_cell '$mode' '$payload' '$idx' '$cap'"
            rc=$?
            if [ "$rc" = "0" ]; then
                local p50 p99
                p50=$(tail -1 "$CSV" | cut -d, -f6)
                p99=$(tail -1 "$CSV" | cut -d, -f8)
                echo "p50=${p50}us p99=${p99}us"
            else
                echo TIMEOUT
            fi
            sleep 0.5
        done
    done
}

started=$(date +%s)

# Variante A: baseline (cache an, sndbuf 256K, tick 5ms = default)
run_mode "baseline"

# Variante B: cache aus (klassisches send_to)
run_mode "cache_off" ZERODDS_UDP_CACHE_ENABLE=0

# Variante C: cache aus + längerer tick-period (1 sec, drastisch)
run_mode "cache_off_tick_1s" ZERODDS_UDP_CACHE_ENABLE=0 ZERODDS_TICK_PERIOD_MS=1000

# Variante D: cache an + längerer tick (sehen ob tick die spikes macht)
run_mode "baseline_tick_1s" ZERODDS_TICK_PERIOD_MS=1000

elapsed=$(( $(date +%s) - started ))
echo
echo "Spread-Diagnose fertig in $((elapsed / 60)) min $((elapsed % 60)) s."
echo "CSV: $CSV"
echo "Per-Mode Files in: $OUT_DIR"
