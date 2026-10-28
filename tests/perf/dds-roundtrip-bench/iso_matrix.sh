#!/usr/bin/env bash
# Isolated cross-vendor matrix with process+domain isolation per cell.
#
# Architecture (inspired by Apex.AI performance_test):
# - Per cell a FRESH bash subprocess via a `( ... )` subshell
# - Per cell its own domain ID (unique per cell, no discovery leak)
# - Raw CSV per cell with all samples (quantiles offline)
# - 8s settle after each cell (TIME_WAIT cleanup)
# - Pre-cell verify: no $vendor-roundtrip processes left open
#
# Expected: <5% timeout rate, <5% CV in the steady state.
#
# Usage:
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
# The domain is assigned mod 100 (range 100-199). RTPS port mapping
# limits domain_id to <232. 192 cells fit into 100 if we
# take the modulo — discovery isolation is still sufficient because
# 100 cells lie in between each time and the settle cleans up the old sockets.
DOMAIN_BASE=100
DOMAIN_RANGE=100

mkdir -p "$OUT_DIR"
CSV="$OUT_DIR/iso_matrix.csv"
echo "ping,pong,payload,run,domain,p50,p99,p999,max,status" > "$CSV"

# ------ Env setup ------
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

cd "$BUILD_DIR" || { echo "build dir missing: $BUILD_DIR" >&2; exit 1; }
for v in "${VENDORS[@]}"; do
    [ -x "./${v}-roundtrip" ] || { echo "Binary missing: ${v}-roundtrip" >&2; exit 1; }
done

# Count running instances of one vendor's roundtrip binary.
#
# NOT `pgrep -x`: that matches against `comm`, which the kernel truncates
# to 15 characters (TASK_COMM_LEN - 1). `zerodds-roundtrip`,
# `cyclone-roundtrip`, `fastdds-roundtrip` and `opendds-roundtrip` are all
# 17, so `-x` matched none of them, ever — `verify_clean` reported a clean
# host while a previous cell's processes were still competing for the CPU
# it was about to measure, and the `pkill -x` beside it killed nothing
# either. Only `rti-roundtrip` (13) was ever seen.
#
# The fix uses the very property that caused the bug: match against `comm`
# as `-x` does, but with the name already truncated the way the kernel
# truncates it. `-f` was the obvious alternative and is worse here — it
# matches the full command line, so any shell whose arguments happen to
# contain the pattern counts as a hit, including the one asking. This
# form cannot find its own caller.
COMM_MAX=15
count_roundtrip() {
    local name="$1-roundtrip"
    pgrep -x "${name:0:$COMM_MAX}" 2>/dev/null | wc -l | tr -d ' '
}

# Clean up ALL vendor binaries before each cell + verify (max 5s wait)
verify_clean() {
    local tries=0
    while [ $tries -lt 10 ]; do
        local count=0
        for bin in "${VENDORS[@]}"; do
            local c
            c=$(count_roundtrip "$bin")
            count=$((count + c))
        done
        if [ "$count" -eq 0 ]; then return 0; fi
        for bin in "${VENDORS[@]}"; do
            local name="${bin}-roundtrip"
            pkill -9 -x "${name:0:$COMM_MAX}" 2>/dev/null
        done
        sleep 0.5
        tries=$((tries + 1))
    done
    echo "verify_clean: still $count processes alive" >&2
    return 1
}

# Per cell: fresh subshell + own domain + complete process tree teardown.
run_cell() {
    local ping=$1 pong=$2 payload=$3 run=$4 cell_idx=$5
    local domain=$((DOMAIN_BASE + (cell_idx % DOMAIN_RANGE)))
    local cell_out="$OUT_DIR/cells"
    mkdir -p "$cell_out"
    local ping_log="$cell_out/${cell_idx}-${ping}-${pong}-${payload}-${run}.ping.log"
    local pong_log="$cell_out/${cell_idx}-${ping}-${pong}-${payload}-${run}.pong.log"

    verify_clean || true

    # Vendor-specific CLI args (OpenDDS needs -DCPSConfigFile).
    local pong_args=""
    local ping_args=""
    if [ "$pong" = "opendds" ]; then
        pong_args="-DCPSConfigFile $BENCH_DIR/opendds_rtps.ini"
    fi
    if [ "$ping" = "opendds" ]; then
        ping_args="-DCPSConfigFile $BENCH_DIR/opendds_rtps.ini"
    fi

    # === Subshell: everything happens in its own subprocess ===
    (
        export ZERODDS_BENCH_DOMAIN="$domain"
        # shellcheck disable=SC2086
        ./"${pong}-roundtrip" pong 60 $pong_args >"$pong_log" 2>&1 &
        local pp=$!
        # Vendor-specific discovery delays — pong side
        case "$pong" in
            fastdds) sleep 8 ;;
            opendds) sleep 10 ;;
            rti)     sleep 6 ;;
            *)       sleep 4 ;;
        esac
        # Vendor-specific ping timeout. RTI init needs ~10-15s on arm-host
        # + discovery from the pong; with 90s in total that is just
        # not enough. OpenDDS too. Others stay at 90s.
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

    # Post-cell: wait until sockets are cleaned up
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
    # Ghost filter: p50 < 5 µs = bench race artifact (loopback floor is ~25µs)
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
echo "ISO matrix done in $((elapsed/60)) min $((elapsed%60)) s, $cell_idx cells."
echo "CSV: $CSV"
echo "Per-cell logs: $OUT_DIR/cells/"
