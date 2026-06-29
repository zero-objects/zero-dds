#!/usr/bin/env bash
# Quick Interop-Matrix: 4 vendoren × 4 payloads × N=3, mit korrekten
# env-vars und retry-on-timeout. Liefert eine kompakte Baseline ohne
# 8h x86-host-Lauf.
#
# 4×4 × 4 × 3 = 192 cells × ~30s ≈ 90 min
#
# Aufruf: quick_matrix.sh <out_dir> [N=3] [PAYLOADS="0 1638 4096 8192"]
# Analyse separat via quick_matrix_report.py
set -u

BENCH_DIR="$(cd "$(dirname "$0")" && pwd)"
BUILD_DIR="$BENCH_DIR/build"
ZD_REPO="$(cd "$BENCH_DIR/../../.." && pwd)"
OUT_DIR="${1:-$BENCH_DIR/quick-out}"
N_RUNS="${2:-3}"
PAYLOADS_STR="${3:-0 1638 4096 8192}"
read -r -a PAYLOADS <<<"$PAYLOADS_STR"

SAMPLES=2000
WARMUP=200
VENDORS=(zerodds cyclone fastdds rti)

mkdir -p "$OUT_DIR"
CSV="$OUT_DIR/quick_matrix.csv"
echo "ping,pong,payload,run,p50,p99,p999,max,status" > "$CSV"

# Env: caller-overridable. arm-host hat $HOME/y/rti..., x86-host /opt/rti...
: "${NDDSHOME:=$HOME/y/rti_connext_dds-7.7.0}"
[ -d "$NDDSHOME" ] || NDDSHOME=/opt/rti.com/rti_connext_dds-7.7.0
: "${RTI_LICENSE_FILE:=$NDDSHOME/rti_license.dat}"
export NDDSHOME RTI_LICENSE_FILE
if [ "$(uname)" = Linux ]; then
    : "${LD_LIBRARY_PATH:=/opt/cyclone/lib:/opt/fastdds/lib:$NDDSHOME/lib/x64Linux4gcc8.5.0:$ZD_REPO/target/release}"
    export LD_LIBRARY_PATH
elif [ "$(uname)" = Darwin ]; then
    : "${DYLD_LIBRARY_PATH:=$ZD_REPO/target/release:$HOME/dds-prefix/lib:$NDDSHOME/lib/arm64Darwin23clang16.0}"
    export DYLD_LIBRARY_PATH
fi

cd "$BUILD_DIR" || { echo "build dir fehlt: $BUILD_DIR" >&2; exit 1; }
for v in "${VENDORS[@]}"; do
    [ -x "./${v}-roundtrip" ] || { echo "Binary fehlt: ${v}-roundtrip" >&2; exit 1; }
done

run_one() {
    local ping=$1 pong=$2 payload=$3 run=$4
    # Kill alle *-roundtrip executables (exact match auf Process-Name).
    # `-f roundtrip` matched auch unseren eigenen
    # `dds-roundtrip-bench/quick_matrix.sh`-Pfad und killt sich selbst.
    for bin in "${VENDORS[@]}"; do
      pkill -9 -x "${bin}-roundtrip" 2>/dev/null
    done
    # Settle: TIME_WAIT-cleanup + Multicast-state. FastDDS-pong braucht
    # >5s damit der Listener-Thread sauber abgemeldet ist; sonst leakt
    # ein zombie callback in die naechste cell.
    sleep 5
    # OpenDDS needs RTPS discovery + rtps_udp via -DCPSConfigFile; the other
    # vendors auto-configure RTPS. (OpenDDS is not in the default VENDORS list
    # here — see opendds_matrix.sh for the dedicated ZeroDDS<->OpenDDS axis.)
    local pong_cfg="" ping_cfg=""
    [ "$pong" = opendds ] && pong_cfg="-DCPSConfigFile $BENCH_DIR/opendds_rtps.ini"
    [ "$ping" = opendds ] && ping_cfg="-DCPSConfigFile $BENCH_DIR/opendds_rtps.ini"
    local pong_log
    pong_log=$(mktemp)
    ./"${pong}-roundtrip" pong 60 $pong_cfg >"$pong_log" 2>&1 &
    local pp=$!
    # Vendor-spezifische Discovery-Delays.
    case "$pong" in
      fastdds) sleep 8 ;;
      opendds) sleep 8 ;;
      rti)     sleep 6 ;;
      *)       sleep 4 ;;
    esac
    local LINE
    LINE=$(timeout 90 ./"${ping}-roundtrip" ping --payload "$payload" \
            --samples "$SAMPLES" --warmup "$WARMUP" $ping_cfg 2>&1 | grep payload=)
    # Graceful pong shutdown: SIGTERM first, then wait, then SIGKILL.
    # Verhindert dass FastDDS-callback-thread im halb-destroyed state
    # in den naechsten run leakt.
    kill -TERM "$pp" 2>/dev/null
    sleep 1
    kill -9 "$pp" 2>/dev/null
    wait "$pp" 2>/dev/null
    rm -f "$pong_log"
    if [ -z "$LINE" ]; then
        echo "[$ping → $pong p=$payload run=$run] TIMEOUT"
        echo "$ping,$pong,$payload,$run,0,,,,timeout" >> "$CSV"
        return 1
    fi
    # Geistwerte (p50 < 5 µs sind Bench-Race-Artefakte, nicht echte
    # Latenz): als invalid markieren statt verfaelschte Daten zu
    # speichern. Cyclone schnellster ist ~25 µs auf Loopback.
    local p50_raw
    p50_raw=$(echo "$LINE" | grep -oE 'p50=[0-9.]+' | cut -d= -f2)
    if awk "BEGIN{exit !(${p50_raw:-0} < 5)}"; then
        echo "[$ping → $pong p=$payload run=$run] GHOST (p50=$p50_raw)"
        echo "$ping,$pong,$payload,$run,0,,,,ghost" >> "$CSV"
        return 1
    fi
    local p50 p99 p999 mx
    p50=$(echo "$LINE" | grep -oE 'p50=[0-9.]+' | cut -d= -f2)
    p99=$(echo "$LINE" | grep -oE 'p99=[0-9.]+' | cut -d= -f2)
    p999=$(echo "$LINE" | grep -oE 'p999=[0-9.]+' | cut -d= -f2)
    mx=$(echo "$LINE" | grep -oE 'max=[0-9.]+' | cut -d= -f2)
    echo "[$ping → $pong p=$payload run=$run] p50=$p50 p99=$p99"
    echo "$ping,$pong,$payload,$run,$p50,$p99,$p999,$mx,ok" >> "$CSV"
    return 0
}

started=$(date +%s)
for payload in "${PAYLOADS[@]}"; do
  for ping in "${VENDORS[@]}"; do
    for pong in "${VENDORS[@]}"; do
      for run in $(seq 1 "$N_RUNS"); do
        run_one "$ping" "$pong" "$payload" "$run" || run_one "$ping" "$pong" "$payload" "$run"
      done
    done
  done
done
elapsed=$(( $(date +%s) - started ))
echo
echo "Quick-Matrix fertig in $((elapsed/60)) min $((elapsed%60)) s."
echo "CSV: $CSV"
