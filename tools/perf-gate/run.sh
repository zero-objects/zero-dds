#!/usr/bin/env bash
# RELIABLE throughput gate.
#
# zerodds_perf (crates/dcps/examples/zerodds_perf.rs), RELIABLE, KEEP_ALL,
# RESOURCE_LIMITS max_samples 100000, 1 kB samples, publisher and
# subscriber as two processes on loopback, three runs of 10 s on domains of
# their own. The gate fails when a run loses a sample (RELIABLE KEEP_ALL
# delivers every one) or when the median rate is below
# ZERODDS_PERF_GATE_MIN_KS (kS/s, written samples per second / 1000).
#
# Why it exists: 809d976aecb made every write of such a writer walk its
# whole history and took this cell from 72 to 42 kS/s on the Linux test
# host; every test stayed green for a week, because none measures a rate.
#
# Usage: ZERODDS_PERF_GATE_MIN_KS=<floor> tools/perf-gate/run.sh
set -euo pipefail
cd "$(dirname "$0")/../.."

MIN=${ZERODDS_PERF_GATE_MIN_KS:?set ZERODDS_PERF_GATE_MIN_KS to the floor in kS/s}
SECS=${ZERODDS_PERF_GATE_SECS:-10}
cargo build -q --release -p zerodds-dcps --example zerodds_perf
BIN="${CARGO_TARGET_DIR:-target}/release/examples/zerodds_perf"
LOGS=$(mktemp -d)

export ZERODDS_PERF_RELIABILITY=reliable ZERODDS_PERF_HISTORY=all \
       ZERODDS_PERF_MAX_SAMPLES=100000 ZERODDS_PERF_SUB_POLL_US=1000

# The reader answers a HEARTBEAT at once (reader.heartbeat_response_delay_ms
# = 0) and not after the 500 ms of DDSI-RTPS 2.5 §8.4.10.1, the default. With
# the default a RELIABLE writer is acknowledged twice a second: its 512 KiB
# window (writer.unacked_window_bytes) lets 1.0 kS/s of 1 kB samples through,
# and without the window the repairs come at the same pace (Linux test host: 330 984
# written in 5 s, 271 152 received in 25 s). The gate measures the cost of the
# write and receive path, which the acknowledgement pace would hide.
export ZERODDS_READER_HEARTBEAT_RESPONSE_DELAY_MS=0

rates=()
for run in 1 2 3; do
    export ZERODDS_PERF_DOMAIN=$((210 + run))
    # The reader outlives the writer by 30 s: a reader on a loaded runner is
    # up to a whole history behind when the writer stops, and the writer
    # waits for its acknowledgments (zerodds_perf), so the tail arrives after
    # the loop. A reader that stopped at SECS + 8 cut that tail off and the
    # gate counted it lost. The rate is written / SECS and does not move.
    "$BIN" sub $((SECS + 30)) > "$LOGS/sub$run.log" 2>&1 &
    sub=$!
    sleep 3
    "$BIN" pub 1024 "$SECS" > "$LOGS/pub$run.log" 2>&1
    wait "$sub"
    written=$(sed -nE 's/.*pub-done: total=([0-9]+).*/\1/p' "$LOGS/pub$run.log")
    received=$(sed -nE 's/.*sub-done: total=([0-9]+) .*/\1/p' "$LOGS/sub$run.log")
    if [ -z "$written" ] || [ -z "$received" ] || [ "$received" -eq 0 ]; then
        echo "perf-gate: run $run produced no result" >&2
        tail -5 "$LOGS/pub$run.log" "$LOGS/sub$run.log" >&2
        exit 1
    fi
    rate=$(awk -v w="$written" -v s="$SECS" 'BEGIN{printf "%.1f", w/s/1000}')
    echo "perf-gate: run $run written=$written received=$received rate=${rate} kS/s"
    if [ "$received" -ne "$written" ]; then
        echo "perf-gate: run $run lost $((written - received)) of $written samples under RELIABLE KEEP_ALL" >&2
        exit 1
    fi
    rates+=("$rate")
done

median=$(printf '%s\n' "${rates[@]}" | sort -n | sed -n 2p)
echo "perf-gate: median ${median} kS/s, floor ${MIN} kS/s"
awk -v m="$median" -v f="$MIN" 'BEGIN{exit !(m >= f)}' || {
    echo "perf-gate: the median is below the floor" >&2
    exit 1
}
