#!/usr/bin/env bash
# tests/perf/soak_runner.sh
#
# 24h soak runner for ZeroDDS — pub+sub at the same time on the same host,
# measures RSS/heap/sample counter over the runtime, fails on a
# memory leak (RSS growth > threshold) or a sample stall.
#
# Normally called by a CI soak job via SSH,
# but can also be tested locally.
#
# Prerequisites on the soak host:
#   - rustup + cargo via $PATH
#   - python3 + ps (procps)
#   - git
#   - optional: heaptrack (apt install heaptrack) — if present, a
#     heap snapshot is taken before and after the soak
#
# Tunable env variables:
#   GITREF        — git ref to checkout (default: main)
#   COMMIT        — sha (default: $GITREF)
#   REPO_URL      — git URL
#   WORKDIR       — where the checkout/build goes
#   OUTDIR        — where the output / logs / csv go
#   RUNTIME_SECS  — soak duration (default: 86400 = 24h)
#                   For a local smoke test: 300 = 5 min
#   SAMPLE_INTERVAL_SECS — RSS capture interval (default: 60)
#   RSS_GROWTH_THRESHOLD_PCT — fail threshold for RSS growth from the
#                              steady state to the end (default: 25)
#
# Steady-state definition: RSS median after the first 10 minutes of
# runtime (startup phase excluded). If the soak duration is < 12 min,
# steady state = max(RSS in the first half) — only for smoke.
#
# Exit:
#   0 — soak successful, RSS growth below threshold, samples flowed
#   1 — memory leak detected (RSS growth > threshold)
#   2 — sample stall detected (subscriber counted no new samples
#       for > 60 s)
#   3 — setup error

set -euo pipefail

GITREF="${GITREF:-main}"
COMMIT="${COMMIT:-$GITREF}"
REPO_URL="${REPO_URL:-https://github.com/zero-objects/zero-dds.git}"
WORKDIR="${WORKDIR:-$HOME/zerodds-soak/$COMMIT}"
OUTDIR="${OUTDIR:-$WORKDIR/soak-output}"
RUNTIME_SECS="${RUNTIME_SECS:-86400}"
SAMPLE_INTERVAL_SECS="${SAMPLE_INTERVAL_SECS:-60}"
RSS_GROWTH_THRESHOLD_PCT="${RSS_GROWTH_THRESHOLD_PCT:-25}"

need() {
    if ! command -v "$1" >/dev/null 2>&1; then
        echo "[soak] missing tool: $1 ($2)" >&2
        return 3
    fi
}
export PATH="$HOME/.cargo/bin:$PATH"
need cargo "rustup install" || exit 3
need git "apt install git" || exit 3
need python3 "apt install python3" || exit 3
need ps "apt install procps" || exit 3

mkdir -p "$WORKDIR" "$OUTDIR"

# --- Workspace ---
if [ ! -d "$WORKDIR/.git" ]; then
    git clone --depth 1 --branch "$GITREF" "$REPO_URL" "$WORKDIR"
fi
cd "$WORKDIR"
git fetch --depth 1 origin "$GITREF"
git checkout -q "$COMMIT" 2>/dev/null || git checkout -q FETCH_HEAD

# --- Build (release) ---
# --- Multi-endpoint mode (CI-4c wave) ---
# With `MULTI_ENDPOINTS=N` (N>=2), the `multi_endpoint_perf` binary is used
# instead of the single-pair shapes_demo: one process with N writers
# on N topics + one process with N readers. Stress test for
# endpoint scaling (discovery, cache management, SEDP).
MULTI_ENDPOINTS="${MULTI_ENDPOINTS:-1}"

# Cross-vendor mode (CI-4c wave): pub side via Cyclone's `ddsperf`
# instead of ZeroDDS shapes_demo. The sub side stays ZeroDDS — tests
# 24 h wire interop stability under Cyclone pub load.
CROSS_VENDOR="${CROSS_VENDOR:-0}"

if [ "$MULTI_ENDPOINTS" -gt 1 ]; then
    echo "[soak] cargo build --release --example multi_endpoint_perf (N=$MULTI_ENDPOINTS)"
    cargo build --release --example multi_endpoint_perf -p dds-dcps \
        >"$OUTDIR/build.log" 2>&1
    PUB_BIN="$WORKDIR/target/release/examples/multi_endpoint_perf"
    SUB_BIN="$PUB_BIN"
    PUB_ARGS=("pub_n" "$MULTI_ENDPOINTS" "$RUNTIME_SECS")
    SUB_ARGS=("sub_n" "$MULTI_ENDPOINTS" "$RUNTIME_SECS")
elif [ "$CROSS_VENDOR" = "1" ]; then
    if ! command -v ddsperf >/dev/null 2>&1; then
        echo "[soak] CROSS_VENDOR=1 set but ddsperf not in PATH — install cyclonedds-tools" >&2
        exit 3
    fi
    echo "[soak] cargo build --release --example shapes_demo_subscriber (cross-vendor: Cyclone-Pub via ddsperf)"
    cargo build --release --example shapes_demo_subscriber -p dds-dcps \
        >"$OUTDIR/build.log" 2>&1
    # Cyclone ddsperf publishes on the "Square" topic with a ShapeType-compatible
    # wire form. Other DDS implementations read it via standard
    # DDS interop. Domain default 0.
    PUB_BIN="$(command -v ddsperf)"
    SUB_BIN="$WORKDIR/target/release/examples/shapes_demo_subscriber"
    # ddsperf pub with 1Hz rate, 1 KB samples
    PUB_ARGS=("-1" "pub" "1Hz" "size" "1024")
    SUB_ARGS=("Square" "0")
else
    echo "[soak] cargo build --release --example shapes_demo_publisher --example shapes_demo_subscriber"
    cargo build --release \
        --example shapes_demo_publisher \
        --example shapes_demo_subscriber \
        -p dds-dcps \
        >"$OUTDIR/build.log" 2>&1
    PUB_BIN="$WORKDIR/target/release/examples/shapes_demo_publisher"
    SUB_BIN="$WORKDIR/target/release/examples/shapes_demo_subscriber"
    PUB_ARGS=("Square" "BLUE" "0")
    SUB_ARGS=("Square" "0")
fi

# --- HEAPTRACK mode (CI-4b wave) ---
# With `HEAPTRACK=1`, pub + sub are instrumented by heaptrack,
# which adds ~20-30 % overhead. The output is two `.heaptrack.zst`
# files (heaptrack-3.0+) or `.heaptrack.gz` (older versions) in
# $OUTDIR, which can be analyzed with `heaptrack_print <file>` or the
# heaptrack GUI.
HEAPTRACK_PREFIX_PUB=""
HEAPTRACK_PREFIX_SUB=""
if [ "${HEAPTRACK:-0}" = "1" ]; then
    if ! command -v heaptrack >/dev/null 2>&1; then
        echo "[soak] HEAPTRACK=1 set but 'heaptrack' not in PATH — install with: apt install heaptrack" >&2
        exit 3
    fi
    echo "[soak] heaptrack mode active — pub+sub are instrumented"
    HEAPTRACK_PREFIX_PUB="heaptrack -o $OUTDIR/heaptrack-pub"
    HEAPTRACK_PREFIX_SUB="heaptrack -o $OUTDIR/heaptrack-sub"
fi

# --- Start sub first (otherwise late-joiner loss at the start) ---
echo "[soak] starting subscriber: $SUB_BIN ${SUB_ARGS[*]}"
$HEAPTRACK_PREFIX_SUB "$SUB_BIN" "${SUB_ARGS[@]}" >"$OUTDIR/sub.log" 2>&1 &
SUB_PID=$!
sleep 2
if ! kill -0 "$SUB_PID" 2>/dev/null; then
    echo "[soak] FAIL: subscriber died at startup" >&2
    tail -20 "$OUTDIR/sub.log" >&2
    exit 3
fi

echo "[soak] starting publisher: $PUB_BIN ${PUB_ARGS[*]}"
$HEAPTRACK_PREFIX_PUB "$PUB_BIN" "${PUB_ARGS[@]}" >"$OUTDIR/pub.log" 2>&1 &
PUB_PID=$!
sleep 2
if ! kill -0 "$PUB_PID" 2>/dev/null; then
    echo "[soak] FAIL: publisher died at startup" >&2
    kill "$SUB_PID" 2>/dev/null || true
    tail -20 "$OUTDIR/pub.log" >&2
    exit 3
fi

cleanup() {
    kill -TERM "$PUB_PID" "$SUB_PID" 2>/dev/null || true
    sleep 1
    kill -KILL "$PUB_PID" "$SUB_PID" 2>/dev/null || true
    wait "$PUB_PID" "$SUB_PID" 2>/dev/null || true
}
trap cleanup EXIT INT TERM

# --- RSS/sample tracking loop ---
RSS_CSV="$OUTDIR/rss-timeline.csv"
echo "elapsed_s,pub_rss_kb,sub_rss_kb,sub_sample_count" >"$RSS_CSV"

START_EPOCH=$(date +%s)
LAST_SAMPLE_COUNT=0
LAST_PROGRESS_EPOCH=$START_EPOCH

echo "[soak] entering monitoring loop, RUNTIME_SECS=$RUNTIME_SECS, interval=${SAMPLE_INTERVAL_SECS}s"
while true; do
    NOW=$(date +%s)
    ELAPSED=$((NOW - START_EPOCH))
    if [ "$ELAPSED" -ge "$RUNTIME_SECS" ]; then
        break
    fi
    # RSS via ps; ps prints KB (on Linux RSS = resident set size in KB)
    PUB_RSS=$(ps -o rss= -p "$PUB_PID" 2>/dev/null | tr -d ' ' || echo 0)
    SUB_RSS=$(ps -o rss= -p "$SUB_PID" 2>/dev/null | tr -d ' ' || echo 0)
    if [ -z "$PUB_RSS" ] || [ -z "$SUB_RSS" ] || [ "$PUB_RSS" = "0" ] || [ "$SUB_RSS" = "0" ]; then
        echo "[soak] FAIL: pub or sub process died (PUB_RSS=$PUB_RSS SUB_RSS=$SUB_RSS)" >&2
        exit 3
    fi
    # Sample count from sub.log: count lines with "received" / "<-".
    SAMPLE_COUNT=$(grep -c -E "(received|<-)" "$OUTDIR/sub.log" 2>/dev/null || echo 0)
    echo "$ELAPSED,$PUB_RSS,$SUB_RSS,$SAMPLE_COUNT" >>"$RSS_CSV"

    if [ "$SAMPLE_COUNT" -gt "$LAST_SAMPLE_COUNT" ]; then
        LAST_SAMPLE_COUNT=$SAMPLE_COUNT
        LAST_PROGRESS_EPOCH=$NOW
    fi

    # Sample stall detection: > 5x interval without new samples = fail
    SINCE_PROGRESS=$((NOW - LAST_PROGRESS_EPOCH))
    STILLSTAND_LIMIT=$((SAMPLE_INTERVAL_SECS * 5))
    if [ "$ELAPSED" -gt "$((SAMPLE_INTERVAL_SECS * 2))" ] && [ "$SINCE_PROGRESS" -gt "$STILLSTAND_LIMIT" ]; then
        echo "[soak] FAIL: sample stall — no new samples in ${SINCE_PROGRESS}s (limit $STILLSTAND_LIMIT)" >&2
        exit 2
    fi

    sleep "$SAMPLE_INTERVAL_SECS"
done

# --- Soak is done — cleanup before evaluation ---
cleanup
trap - EXIT INT TERM

# --- Heaptrack analysis (CI-4b wave, only if HEAPTRACK=1) ---
if [ "${HEAPTRACK:-0}" = "1" ]; then
    if command -v heaptrack_print >/dev/null 2>&1; then
        echo "[soak] heaptrack_print analysis"
        # Heaptrack produces a .zst (heaptrack-3.0+) or .gz (older)
        # file. Look for both variants.
        for prefix in pub sub; do
            for ext in zst gz; do
                f="$OUTDIR/heaptrack-${prefix}.${ext}"
                if [ -f "$f" ]; then
                    heaptrack_print --print-leaks --print-overall-allocators \
                        "$f" >"$OUTDIR/heaptrack-${prefix}.txt" 2>&1 || true
                    # Extract the top lines for the summary
                    head -200 "$OUTDIR/heaptrack-${prefix}.txt" \
                        >"$OUTDIR/heaptrack-${prefix}.summary.txt" 2>&1 || true
                fi
            done
        done
    else
        echo "[soak] heaptrack_print not found — raw .heaptrack files in $OUTDIR/"
    fi
fi

# --- Evaluation ---
python3 - <<PY
import csv, json, statistics, pathlib, datetime, subprocess
csv_path = pathlib.Path("$RSS_CSV")
rows = list(csv.DictReader(csv_path.open()))
if not rows:
    print("[soak] no rows captured", file=__import__("sys").stderr)
    raise SystemExit(3)

elapsed = [int(r["elapsed_s"]) for r in rows]
pub_rss = [int(r["pub_rss_kb"]) for r in rows]
sub_rss = [int(r["sub_rss_kb"]) for r in rows]
samples = [int(r["sub_sample_count"]) for r in rows]

# Leak detection: compare the median over the early steady-state phase
# against the median over the late steady-state phase. Naive comparisons of
# "last sample vs. median over the whole steady state" mask leaks,
# because a slowly growing RSS pulls the median along.
#
# Steady state starts after startup (10min, or runtime/2 for smoke).
# Of that: first half = early window, last half = late window.
# Leak = late_median significantly higher than early_median.
total = elapsed[-1] if elapsed else 0
startup_cutoff = min(600, max(60, total // 4))
steady_idx = next((i for i, e in enumerate(elapsed) if e >= startup_cutoff), 0)
steady_pub = pub_rss[steady_idx:] or [pub_rss[-1]]
steady_sub = sub_rss[steady_idx:] or [sub_rss[-1]]

if len(steady_pub) >= 2:
    half = len(steady_pub) // 2 or 1
    early_pub = steady_pub[:half]
    late_pub  = steady_pub[half:] or steady_pub[-1:]
    early_sub = steady_sub[:half]
    late_sub  = steady_sub[half:] or steady_sub[-1:]
else:
    early_pub = late_pub = steady_pub
    early_sub = late_sub = steady_sub

pub_early_median = statistics.median(early_pub)
pub_late_median  = statistics.median(late_pub)
sub_early_median = statistics.median(early_sub)
sub_late_median  = statistics.median(late_sub)
pub_end = pub_rss[-1]
sub_end = sub_rss[-1]
pub_growth_pct = (pub_late_median - pub_early_median) / pub_early_median * 100 if pub_early_median else 0
sub_growth_pct = (sub_late_median - sub_early_median) / sub_early_median * 100 if sub_early_median else 0
samples_total = samples[-1] if samples else 0
threshold = float("$RSS_GROWTH_THRESHOLD_PCT")

result = {
    "runtime_s":              total,
    "samples_total":          samples_total,
    "pub_rss_early_kb":       pub_early_median,
    "pub_rss_late_kb":        pub_late_median,
    "pub_rss_end_kb":         pub_end,
    "pub_rss_growth_pct":     round(pub_growth_pct, 2),
    "sub_rss_early_kb":       sub_early_median,
    "sub_rss_late_kb":        sub_late_median,
    "sub_rss_end_kb":         sub_end,
    "sub_rss_growth_pct":     round(sub_growth_pct, 2),
    "threshold_pct":          threshold,
    "verdict":                "PASS",
    "fail_reasons":           [],
}
if pub_growth_pct > threshold:
    result["verdict"] = "FAIL"
    result["fail_reasons"].append(f"pub RSS grew {pub_growth_pct:.1f}% > {threshold}%")
if sub_growth_pct > threshold:
    result["verdict"] = "FAIL"
    result["fail_reasons"].append(f"sub RSS grew {sub_growth_pct:.1f}% > {threshold}%")
if samples_total == 0:
    result["verdict"] = "FAIL"
    result["fail_reasons"].append("no samples captured at all")

pathlib.Path("$OUTDIR/soak-summary.json").write_text(json.dumps(result, indent=2))

md = f"""# Soak run summary

* runtime: {total} s ({total/3600:.2f} h)
* samples received: {samples_total}
* startup-cutoff: {startup_cutoff} s, steady-state samples: {len(steady_pub)}

## RSS growth (early vs late steady-state median)

| Process | Early median | Late median | End | Growth |
|---|--:|--:|--:|--:|
| Publisher | {pub_early_median} kB | {pub_late_median} kB | {pub_end} kB | {pub_growth_pct:+.2f}% |
| Subscriber | {sub_early_median} kB | {sub_late_median} kB | {sub_end} kB | {sub_growth_pct:+.2f}% |

Threshold: {threshold}% — verdict **{result['verdict']}**.
"""
if result["fail_reasons"]:
    md += "\n## Fail reasons\n\n"
    for r in result["fail_reasons"]:
        md += f"- {r}\n"
pathlib.Path("$OUTDIR/soak-summary.md").write_text(md)
print(md)
print("verdict:", result["verdict"])
PY

VERDICT=$(python3 -c "import json; print(json.load(open('$OUTDIR/soak-summary.json'))['verdict'])")
case "$VERDICT" in
    PASS) echo "[soak] DONE: PASS"; exit 0;;
    FAIL) echo "[soak] DONE: FAIL — see $OUTDIR/soak-summary.md" >&2; exit 1;;
    *)    echo "[soak] DONE: unexpected verdict '$VERDICT'" >&2; exit 3;;
esac
