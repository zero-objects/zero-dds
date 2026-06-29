#!/usr/bin/env bash
# tests/perf/soak_runner.sh
#
# 24h-Soak-Runner für ZeroDDS — pub+sub gleichzeitig auf demselben Host,
# misst RSS/heap/sample-counter über die Laufzeit, fail bei
# Memory-Leak (RSS-Wachstum > Threshold) oder Sample-Stillstand.
#
# Wird normalerweise vom GitLab-CI-Job `soak-pivot` per SSH aufgerufen,
# kann aber auch lokal getestet werden.
#
# Voraussetzungen auf dem Soak-Host:
#   - rustup + cargo via $PATH
#   - python3 + ps (procps)
#   - git
#   - optional: heaptrack (apt install heaptrack) — wenn da, wird ein
#     heap-snapshot vor und nach dem Soak gefahren
#
# Tunable Env-Variablen:
#   GITREF        — git ref to checkout (default: main)
#   COMMIT        — sha (default: $GITREF)
#   REPO_URL      — git URL
#   WORKDIR       — wo die checkout/build hingeht
#   OUTDIR        — wohin das output / logs / csv
#   RUNTIME_SECS  — Soak-Dauer (default: 86400 = 24h)
#                   Für lokalen smoke-test: 300 = 5 min
#   SAMPLE_INTERVAL_SECS — RSS-Capture-Intervall (default: 60)
#   RSS_GROWTH_THRESHOLD_PCT — Fail-Threshold für RSS-Wachstum vom
#                              Steady-State zum Ende (default: 25)
#
# Steady-State-Definition: RSS-Median nach den ersten 10 Minuten der
# Laufzeit (Startup-Phase ausgeschlossen). Falls die Soak-Dauer < 12 min
# ist, wird Steady-State = max(RSS in erster Hälfte) — nur für smoke.
#
# Exit:
#   0 — Soak erfolgreich, RSS-Wachstum unter Threshold, Samples flossen
#   1 — Memory-Leak detektiert (RSS-Wachstum > Threshold)
#   2 — Sample-Stillstand detektiert (Subscriber zählte > 60 s lang
#       keine neuen Samples)
#   3 — Setup-Fehler

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
# --- Multi-Endpoint-Mode (CI-4c Welle) ---
# Mit `MULTI_ENDPOINTS=N` (N>=2) wird statt der Single-Pair shapes_demo
# das `multi_endpoint_perf`-Binary genutzt: ein Prozess mit N Writers
# auf N Topics + ein Prozess mit N Readers. Stress-Test fuer
# Endpoint-Skalierung (Discovery, Cache-Management, SEDP).
MULTI_ENDPOINTS="${MULTI_ENDPOINTS:-1}"

# Cross-Vendor-Mode (CI-4c Welle): Pub-Seite via Cyclones `ddsperf`
# statt ZeroDDS-shapes_demo. Sub-Seite bleibt ZeroDDS — testet
# 24 h Wire-Interop-Stabilitaet unter Cyclone-Pub-Last.
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
    # Cyclone-ddsperf publish auf "Square"-Topic mit ShapeType-kompatibler
    # Wire-Form. Andere DDS-Implementierungen lesen das via standard
    # DDS-Interop. Domain default 0.
    PUB_BIN="$(command -v ddsperf)"
    SUB_BIN="$WORKDIR/target/release/examples/shapes_demo_subscriber"
    # ddsperf pub mit 1Hz-Rate, 1 KB Samples
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

# --- HEAPTRACK-Mode (CI-4b Welle) ---
# Mit `HEAPTRACK=1` werden pub + sub durch heaptrack instrumentiert,
# was ~20-30 % Overhead bringt. Output sind zwei `.heaptrack.zst`-
# Dateien (heaptrack-3.0+) bzw `.heaptrack.gz` (aeltere Versionen) in
# $OUTDIR, die mit `heaptrack_print <file>` oder der heaptrack-GUI
# analysierbar sind.
HEAPTRACK_PREFIX_PUB=""
HEAPTRACK_PREFIX_SUB=""
if [ "${HEAPTRACK:-0}" = "1" ]; then
    if ! command -v heaptrack >/dev/null 2>&1; then
        echo "[soak] HEAPTRACK=1 set but `heaptrack` not in PATH — install with: apt install heaptrack" >&2
        exit 3
    fi
    echo "[soak] heaptrack-mode aktiv — pub+sub werden instrumentiert"
    HEAPTRACK_PREFIX_PUB="heaptrack -o $OUTDIR/heaptrack-pub"
    HEAPTRACK_PREFIX_SUB="heaptrack -o $OUTDIR/heaptrack-sub"
fi

# --- Sub starten zuerst (sonst joiner-late Verlust am Anfang) ---
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

# --- RSS/Sample-Tracking-Loop ---
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
    # RSS via ps; ps gibt KB aus (auf Linux RSS = resident set size in KB)
    PUB_RSS=$(ps -o rss= -p "$PUB_PID" 2>/dev/null | tr -d ' ' || echo 0)
    SUB_RSS=$(ps -o rss= -p "$SUB_PID" 2>/dev/null | tr -d ' ' || echo 0)
    if [ -z "$PUB_RSS" ] || [ -z "$SUB_RSS" ] || [ "$PUB_RSS" = "0" ] || [ "$SUB_RSS" = "0" ]; then
        echo "[soak] FAIL: pub or sub process died (PUB_RSS=$PUB_RSS SUB_RSS=$SUB_RSS)" >&2
        exit 3
    fi
    # Sample-Count aus sub.log: zaehle Zeilen mit "received" / "<-".
    SAMPLE_COUNT=$(grep -c -E "(received|<-)" "$OUTDIR/sub.log" 2>/dev/null || echo 0)
    echo "$ELAPSED,$PUB_RSS,$SUB_RSS,$SAMPLE_COUNT" >>"$RSS_CSV"

    if [ "$SAMPLE_COUNT" -gt "$LAST_SAMPLE_COUNT" ]; then
        LAST_SAMPLE_COUNT=$SAMPLE_COUNT
        LAST_PROGRESS_EPOCH=$NOW
    fi

    # Sample-Stillstand-Detektion: > 5x interval ohne neue Samples = Fail
    SINCE_PROGRESS=$((NOW - LAST_PROGRESS_EPOCH))
    STILLSTAND_LIMIT=$((SAMPLE_INTERVAL_SECS * 5))
    if [ "$ELAPSED" -gt "$((SAMPLE_INTERVAL_SECS * 2))" ] && [ "$SINCE_PROGRESS" -gt "$STILLSTAND_LIMIT" ]; then
        echo "[soak] FAIL: sample-stillstand — no new samples in ${SINCE_PROGRESS}s (limit $STILLSTAND_LIMIT)" >&2
        exit 2
    fi

    sleep "$SAMPLE_INTERVAL_SECS"
done

# --- Soak ist fertig — Cleanup vor Auswertung ---
cleanup
trap - EXIT INT TERM

# --- Heaptrack-Analyse (CI-4b Welle, nur wenn HEAPTRACK=1) ---
if [ "${HEAPTRACK:-0}" = "1" ]; then
    if command -v heaptrack_print >/dev/null 2>&1; then
        echo "[soak] heaptrack_print analysis"
        # Heaptrack erzeugt eine .zst (heaptrack-3.0+) oder .gz (aelter)
        # Datei. Suche beide Varianten.
        for prefix in pub sub; do
            for ext in zst gz; do
                f="$OUTDIR/heaptrack-${prefix}.${ext}"
                if [ -f "$f" ]; then
                    heaptrack_print --print-leaks --print-overall-allocators \
                        "$f" >"$OUTDIR/heaptrack-${prefix}.txt" 2>&1 || true
                    # Top-Zeilen extrahieren fuer summary
                    head -200 "$OUTDIR/heaptrack-${prefix}.txt" \
                        >"$OUTDIR/heaptrack-${prefix}.summary.txt" 2>&1 || true
                fi
            done
        done
    else
        echo "[soak] heaptrack_print not found — raw .heaptrack-Files in $OUTDIR/"
    fi
fi

# --- Auswertung ---
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

# Leak-Detektion: Vergleiche median über fruehe Steady-State-Phase
# gegen median über späte Steady-State-Phase. Naive Vergleiche mit
# "letzter Sample vs. Median über ganze Steady-State" maskieren leaks,
# weil ein langsam wachsender RSS den Median mitzieht.
#
# Steady-state startet nach Startup (10min, oder runtime/2 für smoke).
# Davon: erste Hälfte = early-window, letzte Hälfte = late-window.
# Leak = late_median signifikant höher als early_median.
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

md = f"""# Soak-Run-Zusammenfassung

* runtime: {total} s ({total/3600:.2f} h)
* samples received: {samples_total}
* startup-cutoff: {startup_cutoff} s, steady-state samples: {len(steady_pub)}

## RSS-Wachstum (early vs late steady-state median)

| Prozess | Early median | Late median | End | Wachstum |
|---|--:|--:|--:|--:|
| Publisher | {pub_early_median} kB | {pub_late_median} kB | {pub_end} kB | {pub_growth_pct:+.2f}% |
| Subscriber | {sub_early_median} kB | {sub_late_median} kB | {sub_end} kB | {sub_growth_pct:+.2f}% |

Threshold: {threshold}% — verdict **{result['verdict']}**.
"""
if result["fail_reasons"]:
    md += "\n## Fail-Gründe\n\n"
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
