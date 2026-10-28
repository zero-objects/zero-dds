#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
#
# Cross-vendor self-cell latency comparison, without CI.
#
# Answers one question: over the five stacks installed on this host, whose
# own end-to-end ping/pong round trip is fastest, at which payload. Each
# vendor talks to itself — that is the cell every stack is tuned for, and
# the only one where a ranking is about the stack rather than about which
# two stacks had to agree on a wire format.
#
# Difference from `iso_matrix.sh`, which stays the tool for the full
# ping x pong interop matrix:
#
#   * self-cells only, so the run is O(vendors) instead of O(vendors^2)
#     and finishes in minutes rather than an hour
#   * a load gate. The Linux test host runs several agents at once; a latency cell
#     next to a `cargo build --workspace` measures the compiler. The
#     script reads /proc/loadavg before every cell, waits for the host to
#     come down, and records the load it measured under in the CSV, so a
#     number can never be read without the load that produced it
#   * a ranking table written next to the CSV
#
# Usage:
#   ./self_compare.sh                          # defaults below
#   OUT_DIR=/tmp/x N_RUNS=5 ./self_compare.sh
#   VENDORS="zerodds cyclone" PAYLOADS="0 8192" ./self_compare.sh
#
# Environment:
#   OUT_DIR    where CSV + report land   (default: ./self-compare-<stamp>)
#   N_RUNS     repetitions per cell      (default: 3)
#   PAYLOADS   octets of `sequence<octet,8192>` (default: "0 1638 4096 8192")
#   VENDORS    subset to measure         (default: all five)
#   SAMPLES    round trips per cell      (default: 2000)
#   WARMUP     discarded round trips     (default: 200)
#   LOAD_MAX   refuse to measure above this 1-minute load (default: 6.0)
#   LOAD_WAIT  seconds to wait for the host to come down  (default: 600)
#   BENCH      "basis" or "rich"         (default: basis)
#
# Prerequisites (both are one-time, and neither is done here — building
# under the measurement would be the very skew this script exists to avoid):
#   cargo build --release -p zerodds-c-api --features security
#   cd tests/perf/dds-roundtrip-bench && mkdir -p build && cd build \
#       && cmake .. && cmake --build . -j 6

set -uo pipefail

BENCH_DIR="$(cd "$(dirname "$0")" && pwd)"
BUILD_DIR="$BENCH_DIR/build"
ZD_REPO="$(cd "$BENCH_DIR/../../.." && pwd)"

OUT_DIR="${OUT_DIR:-$BENCH_DIR/self-compare-$(date +%Y%m%d-%H%M%S)}"
N_RUNS="${N_RUNS:-3}"
PAYLOADS_STR="${PAYLOADS:-0 1638 4096 8192}"
VENDORS_STR="${VENDORS:-zerodds cyclone fastdds rti opendds}"
SAMPLES="${SAMPLES:-2000}"
WARMUP="${WARMUP:-200}"
LOAD_MAX="${LOAD_MAX:-6.0}"
LOAD_WAIT="${LOAD_WAIT:-600}"
BENCH="${BENCH:-basis}"
SETTLE=8
DOMAIN_BASE=140

read -r -a PAYLOADS <<<"$PAYLOADS_STR"
read -r -a VENDORS <<<"$VENDORS_STR"

case "$BENCH" in
    basis) SUFFIX="-roundtrip" ;;
    rich)  SUFFIX="-roundtrip-rich" ;;
    *) echo "BENCH must be 'basis' or 'rich', not '$BENCH'" >&2; exit 2 ;;
esac

mkdir -p "$OUT_DIR/cells"
CSV="$OUT_DIR/self_compare.csv"
echo "vendor,bench,payload,run,domain,load1,p50_us,p90_us,p99_us,p999_us,max_us,status" >"$CSV"

# ---- vendor environment ------------------------------------------------
: "${NDDSHOME:=/opt/rti.com/rti_connext_dds-7.7.0}"
[ -d "$NDDSHOME" ] || NDDSHOME="$HOME/y/rti_connext_dds-7.7.0"
: "${RTI_LICENSE_FILE:=$NDDSHOME/rti_license.dat}"
export NDDSHOME RTI_LICENSE_FILE
RTI_LIB_DIR="$(find "$NDDSHOME/lib" -maxdepth 1 -name 'libnddscpp2.so' -printf '%h\n' 2>/dev/null | head -1)"
if [ "$(uname)" = Linux ]; then
    LD_LIBRARY_PATH="/opt/cyclone/lib:/opt/fastdds/lib:${RTI_LIB_DIR:-}:$ZD_REPO/target/release${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
    export LD_LIBRARY_PATH
fi

cd "$BUILD_DIR" || { echo "no build dir: $BUILD_DIR — see Prerequisites" >&2; exit 1; }
missing=0
for v in "${VENDORS[@]}"; do
    [ -x "./${v}${SUFFIX}" ] || { echo "missing binary: $BUILD_DIR/${v}${SUFFIX}" >&2; missing=1; }
done
[ "$missing" -eq 0 ] || exit 1

# ---- load gate ---------------------------------------------------------
# The 1-minute average, because that is the window a 2000-sample cell
# actually runs in. Everything longer describes a host that is no longer
# there.
load1() { cut -d' ' -f1 /proc/loadavg; }

wait_for_quiet() {
    local waited=0 l
    while :; do
        l="$(load1)"
        if awk "BEGIN{exit !($l <= $LOAD_MAX)}"; then
            echo "$l"
            return 0
        fi
        if [ "$waited" -ge "$LOAD_WAIT" ]; then
            echo "load $l still above LOAD_MAX=$LOAD_MAX after ${LOAD_WAIT}s" >&2
            echo "$l"
            return 1
        fi
        echo "[load] $l > $LOAD_MAX — waiting (${waited}/${LOAD_WAIT}s)" >&2
        sleep 15
        waited=$((waited + 15))
    done
}

kill_all() {
    for v in "${VENDORS[@]}"; do
        pkill -9 -x "${v}${SUFFIX}" 2>/dev/null
    done
}

trap 'kill_all; exit 130' INT TERM

# ---- one cell ----------------------------------------------------------
cell_idx=0
run_cell() {
    local vendor=$1 payload=$2 run=$3
    cell_idx=$((cell_idx + 1))
    local domain=$((DOMAIN_BASE + (cell_idx % 60)))
    local tag="${vendor}-${payload}-${run}"
    local ping_log="$OUT_DIR/cells/$tag.ping.log"
    local pong_log="$OUT_DIR/cells/$tag.pong.log"

    kill_all
    sleep 1

    local load
    load="$(wait_for_quiet)" || echo "[warn] measuring $tag under load $load" >&2

    local args=""
    [ "$vendor" = "opendds" ] && args="-DCPSConfigFile $BENCH_DIR/opendds_rtps.ini"

    local discovery=4
    case "$vendor" in
        fastdds) discovery=8 ;;
        opendds) discovery=10 ;;
        rti)     discovery=6 ;;
    esac
    local ping_timeout=90
    case "$vendor" in
        rti|opendds) ping_timeout=120 ;;
    esac

    (
        export ZERODDS_BENCH_DOMAIN="$domain"
        # shellcheck disable=SC2086
        ./"${vendor}${SUFFIX}" pong 60 $args >"$pong_log" 2>&1 &
        local pp=$!
        sleep "$discovery"
        # shellcheck disable=SC2086
        timeout "$ping_timeout" ./"${vendor}${SUFFIX}" ping --payload "$payload" \
            --samples "$SAMPLES" --warmup "$WARMUP" $args >"$ping_log" 2>&1
        kill -TERM "$pp" 2>/dev/null
        sleep 1
        kill -9 "$pp" 2>/dev/null
        wait "$pp" 2>/dev/null
    )

    sleep "$SETTLE"

    local line
    line=$(grep "payload=" "$ping_log" 2>/dev/null | head -1)
    if [ -z "$line" ]; then
        echo "[$tag dom=$domain load=$load] TIMEOUT"
        echo "$vendor,$BENCH,$payload,$run,$domain,$load,,,,,,timeout" >>"$CSV"
        return 1
    fi

    local p50 p90 p99 p999 mx
    p50=$(grep -oE 'p50=[0-9.]+' <<<"$line" | cut -d= -f2)
    p90=$(grep -oE 'p90=[0-9.]+' <<<"$line" | cut -d= -f2)
    p99=$(grep -oE 'p99=[0-9.]+' <<<"$line" | cut -d= -f2)
    p999=$(grep -oE 'p999=[0-9.]+' <<<"$line" | cut -d= -f2)
    mx=$(grep -oE 'max=[0-9.]+' <<<"$line" | cut -d= -f2)

    # A round trip under 5 us did not cross a transport. The loopback
    # floor on this class of host is ~15 us; anything below is the ping
    # reading its own warm-up echo, and recording it as a win would be a
    # lie in our favour.
    if awk "BEGIN{exit !(${p50:-0} < 5)}"; then
        echo "[$tag dom=$domain load=$load] GHOST p50=$p50"
        echo "$vendor,$BENCH,$payload,$run,$domain,$load,,,,,,ghost" >>"$CSV"
        return 1
    fi

    echo "[$tag dom=$domain load=$load] p50=$p50 p90=$p90 p99=$p99"
    echo "$vendor,$BENCH,$payload,$run,$domain,$load,$p50,$p90,$p99,$p999,$mx,ok" >>"$CSV"
    return 0
}

# ---- run ---------------------------------------------------------------
echo "host: $(hostname), $(nproc) cpus, load at start: $(cat /proc/loadavg)"
echo "bench=$BENCH vendors=[${VENDORS[*]}] payloads=[${PAYLOADS[*]}] runs=$N_RUNS samples=$SAMPLES"
echo

started=$(date +%s)
for payload in "${PAYLOADS[@]}"; do
    for vendor in "${VENDORS[@]}"; do
        for run in $(seq 1 "$N_RUNS"); do
            run_cell "$vendor" "$payload" "$run"
        done
    done
done
kill_all
elapsed=$(($(date +%s) - started))

echo
echo "done in $((elapsed / 60))m $((elapsed % 60))s, $cell_idx cells -> $CSV"

# ---- report ------------------------------------------------------------
REPORT="$OUT_DIR/self_compare.md"
CSV="$CSV" REPORT="$REPORT" BENCH="$BENCH" python3 - <<'PY'
import csv, os, statistics, subprocess
from collections import defaultdict

csv_path = os.environ["CSV"]
report = os.environ["REPORT"]
bench = os.environ["BENCH"]

rows = [r for r in csv.DictReader(open(csv_path))]
ok = [r for r in rows if r["status"] == "ok"]
if not rows:
    open(report, "w").write("# Self-cell round-trip latency\n\nNo cell ran.\n")
    raise SystemExit("no cells ran — see the per-cell logs")

# median of the per-run p50s: a run that lost its slice to another agent
# moves the mean and not the median, and the median is what a repetition
# count of three can actually support.
by = defaultdict(list)
for r in ok:
    by[(r["vendor"], int(r["payload"]))].append(float(r["p50_us"]))

vendors = sorted({v for v, _ in by})
payloads = sorted({p for _, p in by})
loads = [float(r["load1"]) for r in rows if r["load1"]]

lines = []
lines.append(f"# Self-cell round-trip latency — `{bench}`\n")
lines.append(
    f"Host `{os.uname().nodename}`, {os.cpu_count()} cpus. "
    f"1-minute load across the run: min {min(loads):.2f}, "
    f"median {statistics.median(loads):.2f}, max {max(loads):.2f}.\n"
)
lines.append(
    "Each cell is one vendor talking to itself over its own full "
    "typed pipeline. Numbers are the **median of the per-run p50 RTT** "
    "in microseconds; a cell that timed out or produced a sub-5-us ghost "
    "is absent, not zero.\n"
)

# p50 table
head = "| payload | " + " | ".join(vendors) + " | fastest |"
lines.append(head)
lines.append("|" + "---|" * (len(vendors) + 2))
wins = defaultdict(int)
ranks = defaultdict(list)
for p in payloads:
    cells = {}
    for v in vendors:
        vals = by.get((v, p))
        if vals:
            cells[v] = statistics.median(vals)
    if not cells:
        continue
    best = min(cells, key=cells.get)
    wins[best] += 1
    for rank, v in enumerate(sorted(cells, key=cells.get), start=1):
        ranks[v].append(rank)
    row = [f"{p}"]
    for v in vendors:
        row.append(f"**{cells[v]:.1f}**" if v == best else (f"{cells[v]:.1f}" if v in cells else "—"))
    row.append(best)
    lines.append("| " + " | ".join(row) + " |")
lines.append("")

# ranking
lines.append("## Mean rank across the payload sweep\n")
lines.append("| vendor | cells | mean rank | payloads won |")
lines.append("|---|---|---|---|")
for v in sorted(vendors, key=lambda v: statistics.mean(ranks[v]) if ranks[v] else 99):
    if not ranks[v]:
        continue
    lines.append(
        f"| {v} | {len(ranks[v])} | {statistics.mean(ranks[v]):.2f} | {wins[v]} |"
    )
lines.append("")
lines.append(
    "Mean rank 1.00 is \"first at every payload\". A vendor missing from a "
    "payload row is not ranked there, so compare `cells` before comparing "
    "mean rank.\n"
)

# spread, so nobody reads a median as a precision claim
lines.append("## Spread per cell (per-run p50, min–max)\n")
lines.append("| payload | " + " | ".join(vendors) + " |")
lines.append("|" + "---|" * (len(vendors) + 1))
for p in payloads:
    row = [f"{p}"]
    for v in vendors:
        vals = by.get((v, p))
        row.append(f"{min(vals):.1f}–{max(vals):.1f}" if vals else "—")
    lines.append("| " + " | ".join(row) + " |")
lines.append("")

failed = [r for r in rows if r["status"] != "ok"]
if failed:
    lines.append("## Cells that produced no number\n")
    lines.append("| vendor | payload | run | status |")
    lines.append("|---|---|---|---|")
    for r in failed:
        lines.append(f"| {r['vendor']} | {r['payload']} | {r['run']} | {r['status']} |")
    lines.append("")

open(report, "w").write("\n".join(lines) + "\n")
print("\n".join(lines))
print(f"\nreport: {report}")
PY
