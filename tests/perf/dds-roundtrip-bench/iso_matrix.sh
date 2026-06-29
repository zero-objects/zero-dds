#!/usr/bin/env bash
# Isolated Cross-Vendor-Matrix mit Process+Domain-Isolation pro Cell.
#
# Architektur (inspiriert von Apex.AI performance_test):
# - Pro Cell ein FRISCHER bash-Sub-Prozess via `( ... )` Sub-Shell
# - Pro Cell eigene Domain-ID (unique pro Cell, kein Discovery-Leak)
# - Raw-CSV pro Cell mit allen Samples (Quantile offline)
# - 8s settle nach jedem Cell (TIME_WAIT-Cleanup)
# - Pre-Cell-Verify: keine $vendor-roundtrip Prozesse mehr offen
#
# Erwartet: <5% timeout-Rate, <5% CV im steady-state.
#
# Aufruf:
#   iso_matrix.sh <out_dir> [N=3] [PAYLOADS="0 1638 4096 8192"] [VENDORS="zerodds cyclone fastdds rti"]
set -u

BENCH_DIR="$(cd "$(dirname "$0")" && pwd)"
BUILD_DIR="$BENCH_DIR/build"
ZD_REPO="$(cd "$BENCH_DIR/../../.." && pwd)"
OUT_DIR="${1:-$BENCH_DIR/iso-out}"
N_RUNS="${2:-3}"
PAYLOADS_STR="${3:-0 1638 4096 8192}"
VENDORS_STR="${4:-zerodds cyclone fastdds rti}"
read -r -a PAYLOADS <<<"$PAYLOADS_STR"
read -r -a VENDORS <<<"$VENDORS_STR"

SAMPLES=2000
WARMUP=200
SETTLE_BETWEEN_CELLS=8
# Domain wird mod 100 vergeben (Range 100-199). RTPS port-mapping
# limitiert domain_id auf <232. 192 cells passt in 100 wenn wir
# modulo nehmen — Discovery-Isolation reicht trotzdem weil je
# 100 cells dazwischen liegen und Settle die alten sockets cleant.
DOMAIN_BASE=100
DOMAIN_RANGE=100

mkdir -p "$OUT_DIR"
CSV="$OUT_DIR/iso_matrix.csv"
echo "ping,pong,payload,run,domain,p50,p99,p999,max,status" > "$CSV"

# ------ Env-Setup ------
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

# Cleanup ALLEN vendor-binaries vor jedem cell + Verify (max 5s wait)
verify_clean() {
    local tries=0
    while [ $tries -lt 10 ]; do
        local count=0
        for bin in "${VENDORS[@]}"; do
            local c
            c=$(pgrep -x "${bin}-roundtrip" 2>/dev/null | wc -l | tr -d ' ')
            count=$((count + c))
        done
        if [ "$count" -eq 0 ]; then return 0; fi
        for bin in "${VENDORS[@]}"; do
            pkill -9 -x "${bin}-roundtrip" 2>/dev/null
        done
        sleep 0.5
        tries=$((tries + 1))
    done
    echo "verify_clean: still $count processes alive" >&2
    return 1
}

# Pro Cell: fresh sub-shell + eigene Domain + complete process tree teardown.
run_cell() {
    local ping=$1 pong=$2 payload=$3 run=$4 cell_idx=$5
    local domain=$((DOMAIN_BASE + (cell_idx % DOMAIN_RANGE)))
    local cell_out="$OUT_DIR/cells"
    mkdir -p "$cell_out"
    local ping_log="$cell_out/${cell_idx}-${ping}-${pong}-${payload}-${run}.ping.log"
    local pong_log="$cell_out/${cell_idx}-${ping}-${pong}-${payload}-${run}.pong.log"

    verify_clean || true

    # Vendor-spezifische CLI-Args (OpenDDS braucht -DCPSConfigFile).
    local pong_args=""
    local ping_args=""
    if [ "$pong" = "opendds" ]; then
        pong_args="-DCPSConfigFile $BENCH_DIR/opendds_rtps.ini"
    fi
    if [ "$ping" = "opendds" ]; then
        ping_args="-DCPSConfigFile $BENCH_DIR/opendds_rtps.ini"
    fi

    # === Sub-Shell: alles passiert in eigenem Sub-Process ===
    (
        export ZERODDS_BENCH_DOMAIN="$domain"
        # shellcheck disable=SC2086
        ./"${pong}-roundtrip" pong 60 $pong_args >"$pong_log" 2>&1 &
        local pp=$!
        # Vendor-spezifische discovery delays — pong-side
        case "$pong" in
            fastdds) sleep 8 ;;
            opendds) sleep 10 ;;
            rti)     sleep 6 ;;
            *)       sleep 4 ;;
        esac
        # Vendor-spezifischer ping timeout. RTI-Init braucht auf arm-host
        # ~10-15s + Discovery vom pong; mit 90s total reicht das knapp
        # nicht. OpenDDS auch. Andere bleiben bei 90s.
        local ping_timeout=90
        case "$ping" in
            rti)     ping_timeout=120 ;;
            opendds) ping_timeout=120 ;;
        esac
        # shellcheck disable=SC2086
        timeout "$ping_timeout" ./"${ping}-roundtrip" ping --payload "$payload" \
            --samples "$SAMPLES" --warmup "$WARMUP" $ping_args >"$ping_log" 2>&1
        # Graceful pong stop
        kill -TERM "$pp" 2>/dev/null
        sleep 1
        kill -9 "$pp" 2>/dev/null
        wait "$pp" 2>/dev/null
    )

    local LINE
    LINE=$(grep "payload=" "$ping_log" 2>/dev/null | head -1)

    # Post-cell: warten bis Sockets cleaned
    sleep "$SETTLE_BETWEEN_CELLS"

    if [ -z "$LINE" ]; then
        echo "[cell $cell_idx $ping → $pong p=$payload run=$run dom=$domain] TIMEOUT"
        echo "$ping,$pong,$payload,$run,$domain,0,,,,timeout" >> "$CSV"
        return 1
    fi
    local p50 p99 p999 mx
    p50=$(echo "$LINE" | grep -oE 'p50=[0-9.]+' | cut -d= -f2)
    p99=$(echo "$LINE" | grep -oE 'p99=[0-9.]+' | cut -d= -f2)
    p999=$(echo "$LINE" | grep -oE 'p999=[0-9.]+' | cut -d= -f2)
    mx=$(echo "$LINE" | grep -oE 'max=[0-9.]+' | cut -d= -f2)
    # Ghost-Filter: p50 < 5 µs = bench-race-artifact (Loopback-Floor ist ~25µs)
    if awk "BEGIN{exit !(${p50:-0} < 5)}"; then
        echo "[cell $cell_idx $ping → $pong p=$payload run=$run dom=$domain] GHOST (p50=$p50)"
        echo "$ping,$pong,$payload,$run,$domain,0,,,,ghost" >> "$CSV"
        return 1
    fi
    echo "[cell $cell_idx $ping → $pong p=$payload run=$run dom=$domain] p50=$p50 p99=$p99"
    echo "$ping,$pong,$payload,$run,$domain,$p50,$p99,$p999,$mx,ok" >> "$CSV"
    return 0
}

started=$(date +%s)
cell_idx=0
for payload in "${PAYLOADS[@]}"; do
  for ping in "${VENDORS[@]}"; do
    for pong in "${VENDORS[@]}"; do
      for run in $(seq 1 "$N_RUNS"); do
        cell_idx=$((cell_idx + 1))
        run_cell "$ping" "$pong" "$payload" "$run" "$cell_idx"
      done
    done
  done
done
elapsed=$(( $(date +%s) - started ))
echo
echo "ISO-Matrix fertig in $((elapsed/60)) min $((elapsed%60)) s, $cell_idx cells."
echo "CSV: $CSV"
echo "Per-Cell-Logs: $OUT_DIR/cells/"
