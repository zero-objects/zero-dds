#!/usr/bin/env bash
# tests/perf/llvm_bench_runner.sh
#
# Native Bench-Runner für den `llvm@llvm` Bare-Metal-Host (CI-3 Welle).
# Wird via SSH vom GitLab-CI-Job `bench-llvm` aufgerufen oder lokal vom
# Entwickler.
#
# Voraussetzungen auf dem Host (siehe Bootstrap unten):
#   - rustup + cargo via ~/.cargo/bin (User-Install, kein root)
#   - ddsperf (Cyclone DDS), libfastrtps-dev, libddsc-dev
#   - git
#
# Was der Skript macht:
#   1. Workspace ins ~/zerodds-bench/<commit>/ klonen / aktualisieren
#   2. Criterion-Bench-Suite voll laufen mit `--save-baseline llvm-<sha>`
#   3. ddsperf-Cross-Vendor-Latenz-Test (ZeroDDS spdp_demo + ddsperf
#      ping/pong); 60 s, RTT-Histogramm in result.json
#   4. Throughput-Test mit ddsperf (1 KB samples, 10000 max throughput)
#   5. Sammelt:
#        - target/criterion/        (criterion-Daten)
#        - bench-output.log         (criterion stdout)
#        - latency_<vendor>.json    (RTT-Histogram per Vendor)
#        - throughput_<vendor>.json (msgs/sec, MB/sec, lost)
#        - bench-summary.md         (Markdown-Zusammenfassung)
#
# Exit:
#   0 — Bench-Run erfolgreich, alle Artefakte geschrieben
#   1 — Fehler beim Bench-Run / fehlende Artefakte
#   2 — Voraussetzungen nicht erfüllt

set -euo pipefail

GITREF="${GITREF:-main}"
COMMIT="${COMMIT:-${GITREF}}"
REPO_URL="${REPO_URL:-https://gitlab.sandra-kessler.eu/fishermen21/zerodds.git}"
WORKDIR="${WORKDIR:-$HOME/zerodds-bench/$COMMIT}"
OUTDIR="${OUTDIR:-$WORKDIR/bench-output}"
RUNTIME_SECS="${RUNTIME_SECS:-60}"

# --- Voraussetzungen ---
need() {
    if ! command -v "$1" >/dev/null 2>&1; then
        echo "[llvm-bench] missing tool: $1 ($2)" >&2
        return 2
    fi
}
export PATH="$HOME/.cargo/bin:$PATH"
need cargo "rustup install" || exit 2
need git "apt install git" || exit 2
need ddsperf "apt install cyclonedds-tools" || exit 2

mkdir -p "$WORKDIR" "$OUTDIR"

# --- Step 1: Workspace bereitstellen ---
if [ ! -d "$WORKDIR/.git" ]; then
    git clone --depth 1 --branch "$GITREF" "$REPO_URL" "$WORKDIR"
fi
cd "$WORKDIR"
git fetch --depth 1 origin "$GITREF"
git checkout -q "$COMMIT" 2>/dev/null || git checkout -q FETCH_HEAD

# --- Step 2: Criterion-Suite ---
echo "[llvm-bench] cargo bench --workspace -- --save-baseline llvm-$COMMIT"
cargo bench --workspace -- --save-baseline "llvm-$COMMIT" \
    2>&1 | tee "$OUTDIR/bench-output.log"

# Criterion-Daten kopieren (target/criterion/ kann gross sein, nur estimates).
mkdir -p "$OUTDIR/criterion"
find target/criterion -name "estimates.json" -path "*/llvm-$COMMIT/*" \
    | while read -r f; do
        rel="${f#target/criterion/}"
        dest="$OUTDIR/criterion/$rel"
        mkdir -p "$(dirname "$dest")"
        cp "$f" "$dest"
    done

# --- Step 3: ddsperf Latenz (Cyclone Self-Test als Sanity) ---
echo "[llvm-bench] ddsperf latency-test (Cyclone self, $RUNTIME_SECS s)"
ddsperf -1 ping 1Hz size 1024 \
    >"$OUTDIR/cyclone_ping.log" 2>&1 &
PING_PID=$!
ddsperf pong \
    >"$OUTDIR/cyclone_pong.log" 2>&1 &
PONG_PID=$!
sleep "$RUNTIME_SECS"
kill -INT "$PING_PID" "$PONG_PID" 2>/dev/null || true
wait "$PING_PID" "$PONG_PID" 2>/dev/null || true

# Aus dem ping-Log RTT-Histogramm extrahieren. Real ddsperf-2.x format:
#   "size 1024 mean 145.409us min 145.409us 50% 145.409us 90% 145.409us
#    99% 145.409us max 145.409us cnt 1"
# Beachte: floats mit `us`-Suffix, kein 99.9%, "mean" vor "min".
python3 - <<PY
import json, re, pathlib, statistics
out = pathlib.Path("$OUTDIR/latency_cyclone.json")
log = pathlib.Path("$OUTDIR/cyclone_ping.log").read_text(errors="replace")
pat = re.compile(
    r"mean\s+([\d.]+)us\s+min\s+([\d.]+)us\s+"
    r"50%\s+([\d.]+)us\s+90%\s+([\d.]+)us\s+99%\s+([\d.]+)us\s+max\s+([\d.]+)us"
)
hits = pat.findall(log)
result = {"vendor": "cyclone", "samples_count_lines": len(hits)}
if hits:
    # Aggregat über alle Lines: median pro Statistik (robust gegen Warm-Up).
    means = [float(h[0]) for h in hits]
    mins  = [float(h[1]) for h in hits]
    p50s  = [float(h[2]) for h in hits]
    p90s  = [float(h[3]) for h in hits]
    p99s  = [float(h[4]) for h in hits]
    maxs  = [float(h[5]) for h in hits]
    result.update({
        "mean_us":   statistics.median(means),
        "min_us":    min(mins),
        "p50_us":    statistics.median(p50s),
        "p90_us":    statistics.median(p90s),
        "p99_us":    statistics.median(p99s),
        "max_us":    max(maxs),
    })
else:
    result["error"] = "no histogram lines parsed (ddsperf format change?)"
out.write_text(json.dumps(result, indent=2))
print("[llvm-bench] wrote", out, ":", json.dumps(result))
PY

# --- Step 4: ddsperf Throughput ---
echo "[llvm-bench] ddsperf throughput-test (Cyclone self, $RUNTIME_SECS s, 1 KB)"
ddsperf -1 pub size 1024 \
    >"$OUTDIR/cyclone_pub.log" 2>&1 &
PUB_PID=$!
ddsperf sub \
    >"$OUTDIR/cyclone_sub.log" 2>&1 &
SUB_PID=$!
sleep "$RUNTIME_SECS"
kill -INT "$PUB_PID" "$SUB_PID" 2>/dev/null || true
wait "$PUB_PID" "$SUB_PID" 2>/dev/null || true

# --- Step 4b: ZeroDDS-Self Throughput + Ping/Pong (CI-3b Welle) ---
echo "[llvm-bench] zerodds_perf build --release"
cargo build --release --example zerodds_perf -p dds-dcps \
    >"$OUTDIR/zerodds_perf_build.log" 2>&1
ZD_PERF="$WORKDIR/target/release/examples/zerodds_perf"

if [ -x "$ZD_PERF" ]; then
    # ZeroDDS-Self Throughput (1 KB samples)
    echo "[llvm-bench] zerodds_perf throughput (Self, $RUNTIME_SECS s, 1 KB)"
    "$ZD_PERF" pub 1024 "$RUNTIME_SECS" >"$OUTDIR/zerodds_pub.log" 2>&1 &
    ZD_PUB_PID=$!
    "$ZD_PERF" sub "$RUNTIME_SECS" >"$OUTDIR/zerodds_sub.log" 2>&1 &
    ZD_SUB_PID=$!
    wait "$ZD_PUB_PID" "$ZD_SUB_PID" 2>/dev/null || true

    # ZeroDDS-Self Ping/Pong RTT
    echo "[llvm-bench] zerodds_perf pingpong (Self, 30s)"
    "$ZD_PERF" pong 30 >"$OUTDIR/zerodds_pong.log" 2>&1 &
    ZD_PONG_PID=$!
    sleep 1
    "$ZD_PERF" pingpong 30 >"$OUTDIR/zerodds_pingpong.log" 2>&1 &
    ZD_PING_PID=$!
    wait "$ZD_PING_PID" "$ZD_PONG_PID" 2>/dev/null || true

    # Parser
    python3 - <<PY
import json, re, pathlib, statistics
out = pathlib.Path("$OUTDIR/zerodds_perf.json")
result = {"vendor": "zerodds"}

# Throughput aus zerodds_sub.log
sub_log = pathlib.Path("$OUTDIR/zerodds_sub.log").read_text(errors="replace")
sub_pat = re.compile(r"size N total (\d+) delta (\d+) rate ([\d.]+) kS/s")
sub_hits = sub_pat.findall(sub_log)
if sub_hits:
    rates = [float(h[2]) for h in sub_hits]
    result["throughput_kS_per_s_median"] = statistics.median(rates)
    result["samples_total"] = int(sub_hits[-1][0])

# RTT aus zerodds_pingpong.log
pp_log = pathlib.Path("$OUTDIR/zerodds_pingpong.log").read_text(errors="replace")
rtt_pat = re.compile(
    r"rtt mean (\d+)us min (\d+) 50% (\d+) 90% (\d+) 99% (\d+) max (\d+) cnt (\d+)"
)
rtt_hits = rtt_pat.findall(pp_log)
if rtt_hits:
    last = rtt_hits[-1]
    result.update({
        "rtt_mean_us": int(last[0]),
        "rtt_min_us": int(last[1]),
        "rtt_p50_us": int(last[2]),
        "rtt_p90_us": int(last[3]),
        "rtt_p99_us": int(last[4]),
        "rtt_max_us": int(last[5]),
        "rtt_count": int(last[6]),
    })
out.write_text(json.dumps(result, indent=2))
print("[llvm-bench] wrote", out, ":", json.dumps(result))
PY
else
    echo "[llvm-bench] zerodds_perf binary not built — skipping ZeroDDS-Self bench"
fi

# --- Step 4c: Cross-Vendor-Throughput (CI-3c v1) ---
# Throughput-Numbers fuer ZeroDDS<->Cyclone via die bestehenden interop-
# scripts. Nutzt cyclonedds-python und ZeroDDS shapes_demo. Latenz-Test
# kommt durch CI-3c v2 (Apex.AI Step 4d).
XV_RUNTIME=20
XV_PUB="$WORKDIR/target/release/examples/shapes_demo_publisher"
XV_SUB="$WORKDIR/target/release/examples/shapes_demo_subscriber"
XV_CY_PUB_PY="$WORKDIR/tests/interop/cyclone_shapes_pub.py"
XV_CY_SUB_PY="$WORKDIR/tests/interop/cyclone_shapes_sub.py"

if [ -x "$XV_PUB" ] && [ -x "$XV_SUB" ] && [ -f "$XV_CY_SUB_PY" ]; then
    echo "[llvm-bench] cross-vendor throughput dir-1 (ZeroDDS-Pub -> Cyclone-Sub)"
    "$XV_PUB" Square BLUE 0 >"$OUTDIR/xv_zd_pub.log" 2>&1 &
    XV_ZD_PID=$!
    sleep 1
    timeout "${XV_RUNTIME}s" python3 "$XV_CY_SUB_PY" Square 0 \
        >"$OUTDIR/xv_cy_sub.log" 2>&1 || true
    kill -TERM "$XV_ZD_PID" 2>/dev/null || true
    wait "$XV_ZD_PID" 2>/dev/null || true

    echo "[llvm-bench] cross-vendor throughput dir-2 (Cyclone-Pub -> ZeroDDS-Sub)"
    python3 "$XV_CY_PUB_PY" Square GREEN 0 \
        >"$OUTDIR/xv_cy_pub.log" 2>&1 &
    XV_CY_PID=$!
    sleep 1
    timeout "${XV_RUNTIME}s" "$XV_SUB" Square 0 \
        >"$OUTDIR/xv_zd_sub.log" 2>&1 || true
    kill -TERM "$XV_CY_PID" 2>/dev/null || true
    wait "$XV_CY_PID" 2>/dev/null || true

    python3 - <<PY
import json, re, pathlib

out = pathlib.Path("$OUTDIR/cross_vendor_throughput.json")

def count(path, pat):
    try:
        return len(re.findall(pat, pathlib.Path(path).read_text(errors="replace")))
    except Exception:
        return 0

cy_received = count("$OUTDIR/xv_cy_sub.log", r"<-\s*color=")
zd_received = count("$OUTDIR/xv_zd_sub.log", r"received|<-")
runtime_s = $XV_RUNTIME

result = {
    "direction_1_zerodds_pub_cyclone_sub": {
        "samples_received": cy_received,
        "throughput_S_per_s": round(cy_received / runtime_s, 2),
        "runtime_s": runtime_s,
    },
    "direction_2_cyclone_pub_zerodds_sub": {
        "samples_received": zd_received,
        "throughput_S_per_s": round(zd_received / runtime_s, 2),
        "runtime_s": runtime_s,
    },
}
out.write_text(json.dumps(result, indent=2))
print("[llvm-bench] wrote", out, ":", json.dumps(result))
PY
else
    echo "[llvm-bench] missing prerequisites for cross-vendor throughput — skipping"
fi

# --- Step 4d: Apex.AI Cross-Vendor Latency (CI-3c v2) ---
# Verwendet das `apex-perf:cyclonedds` Docker-Image (build via
# tests/perf/apex/Dockerfile) fuer Cyclone-Self-Latency mit echten
# percentiles aus Apex.AIs JSON-Output.
#
# Cross-Vendor (ZeroDDS↔Cyclone) braucht ein ZeroDDS-Apex-Plugin
# (CI-3d, nach C-API-Welle). Vorher: nur Cyclone-Self als Reference.
APEX_RUNTIME=15
if docker image inspect apex-perf:cyclonedds >/dev/null 2>&1; then
    echo "[llvm-bench] Apex.AI Cyclone-Self ($APEX_RUNTIME s, Array1k @ 1000 Hz)"
    mkdir -p "$OUTDIR/apex"
    chmod 777 "$OUTDIR/apex"
    docker run --rm --network=host -v "$OUTDIR/apex":/out apex-perf:cyclonedds \
        -c CycloneDDS -m Array1k -p 0 -s 1 --max-runtime "$APEX_RUNTIME" \
        -l /out/apex_sub.json >"$OUTDIR/apex_sub.log" 2>&1 &
    APEX_SUB_PID=$!
    sleep 1
    docker run --rm --network=host -v "$OUTDIR/apex":/out apex-perf:cyclonedds \
        -c CycloneDDS -m Array1k -p 1 -s 0 -r 1000 --max-runtime "$APEX_RUNTIME" \
        -l /out/apex_pub.json >"$OUTDIR/apex_pub.log" 2>&1
    wait "$APEX_SUB_PID" 2>/dev/null || true

    python3 - <<PY
import json, pathlib, statistics

out = pathlib.Path("$OUTDIR/apex_summary.json")
sub_json = pathlib.Path("$OUTDIR/apex/apex_sub.json")

result = {"vendor": "cyclone-via-apex"}
if sub_json.exists():
    data = json.loads(sub_json.read_text())
    rows = data.get("analysis_results", [])
    # Drop ersten Eintrag (cold-start + Discovery), aggregiere rest.
    steady = [r for r in rows[1:] if r.get("num_samples_received", 0) > 0]
    if steady:
        latencies_mean_us = [r["latency_mean"] * 1_000_000 for r in steady]
        latencies_min_us = [r["latency_min"] * 1_000_000 for r in steady]
        latencies_max_us = [r["latency_max"] * 1_000_000 for r in steady]
        samples_per_sec = [r["num_samples_received"] for r in steady]
        result.update({
            "rate_S_per_s_median": statistics.median(samples_per_sec),
            "latency_mean_us_median": round(statistics.median(latencies_mean_us), 2),
            "latency_min_us_min": round(min(latencies_min_us), 2),
            "latency_max_us_max": round(max(latencies_max_us), 2),
            "samples_total": sum(r.get("num_samples_received", 0) for r in steady),
            "samples_lost_total": sum(r.get("num_samples_lost", 0) for r in steady),
            "steady_window_s": len(steady),
        })
out.write_text(json.dumps(result, indent=2))
print("[llvm-bench] wrote", out, ":", json.dumps(result))
PY
else
    echo "[llvm-bench] apex-perf:cyclonedds image not found — skipping Apex.AI bench"
    echo "    Build: cd ~/performance_test && docker build -f /path/to/zerodds/tests/perf/apex/Dockerfile -t apex-perf:cyclonedds ."
fi

python3 - <<PY
import json, re, pathlib, statistics
out = pathlib.Path("$OUTDIR/throughput_cyclone.json")
log = pathlib.Path("$OUTDIR/cyclone_sub.log").read_text(errors="replace")
# Real ddsperf-2.x sub format:
#   "size 1024 total 172466 lost 0 delta 57713 lost 0 rate 57.71 kS/s 472.78 Mb/s ..."
pat = re.compile(
    r"size\s+(\d+)\s+total\s+(\d+)\s+lost\s+(\d+)\s+"
    r"delta\s+\d+\s+lost\s+\d+\s+rate\s+([\d.]+)\s+kS/s\s+([\d.]+)\s+Mb/s"
)
hits = pat.findall(log)
result = {"vendor": "cyclone", "samples_count_lines": len(hits)}
if hits:
    last = hits[-1]
    rates_kS = [float(h[3]) for h in hits]
    rates_Mb = [float(h[4]) for h in hits]
    result.update({
        "size_bytes":      int(last[0]),
        "samples_total":   int(last[1]),
        "samples_lost":    int(last[2]),
        "rate_kS_per_s_median":  statistics.median(rates_kS),
        "rate_Mb_per_s_median":  statistics.median(rates_Mb),
        "rate_kS_per_s_max":     max(rates_kS),
        "rate_Mb_per_s_max":     max(rates_Mb),
    })
else:
    result["error"] = "no rate lines parsed"
out.write_text(json.dumps(result, indent=2))
print("[llvm-bench] wrote", out, ":", json.dumps(result))
PY

# --- Step 5: Markdown-Zusammenfassung ---
python3 - <<PY
import json, pathlib, datetime, subprocess
out = pathlib.Path("$OUTDIR/bench-summary.md")
def load(p):
    try: return json.loads(pathlib.Path(p).read_text())
    except Exception: return {}
lat = load("$OUTDIR/latency_cyclone.json")
thr = load("$OUTDIR/throughput_cyclone.json")
sha = subprocess.check_output(["git","-C","$WORKDIR","rev-parse","HEAD"]).decode().strip()
host = subprocess.check_output(["uname","-n"]).decode().strip()
ts = datetime.datetime.now().isoformat(timespec="seconds")

bench_count = sum(1 for _ in pathlib.Path("$OUTDIR/criterion").rglob("estimates.json"))

content = f"""# Bench-Run-Zusammenfassung

* commit: \`{sha}\`
* host: \`{host}\`
* timestamp: {ts}
* runtime per stage: {$RUNTIME_SECS} s

## Criterion-Suite

* benchmarks erfasst: {bench_count}
* baseline: \`llvm-{sha[:8]}\`
* details: \`bench-output.log\` + \`criterion/\`

## Cyclone-Self ddsperf-Latenz (1 KB ping/pong, median über alle Sekunden)

| Statistik | µs |
|---|--:|
| min | {lat.get('min_us', 'n/a')} |
| mean | {lat.get('mean_us', 'n/a')} |
| p50 | {lat.get('p50_us', 'n/a')} |
| p90 | {lat.get('p90_us', 'n/a')} |
| p99 | {lat.get('p99_us', 'n/a')} |
| max | {lat.get('max_us', 'n/a')} |

## Cyclone-Self ddsperf-Throughput (1 KB samples)

* samples total: {thr.get('samples_total', 'n/a')}, lost: {thr.get('samples_lost', 'n/a')}
* rate (median): {thr.get('rate_kS_per_s_median', 'n/a')} kS/s, {thr.get('rate_Mb_per_s_median', 'n/a')} Mb/s
* rate (max):    {thr.get('rate_kS_per_s_max', 'n/a')} kS/s, {thr.get('rate_Mb_per_s_max', 'n/a')} Mb/s

## ZeroDDS-Self (CI-3b Welle)
"""
zd = load("$OUTDIR/zerodds_perf.json")
if zd:
    content += f"""
| Metrik | Wert |
|---|--:|
| throughput median | {zd.get('throughput_kS_per_s_median', 'n/a')} kS/s |
| samples total | {zd.get('samples_total', 'n/a')} |
| RTT mean | {zd.get('rtt_mean_us', 'n/a')} µs |
| RTT min | {zd.get('rtt_min_us', 'n/a')} µs |
| RTT p50 | {zd.get('rtt_p50_us', 'n/a')} µs |
| RTT p90 | {zd.get('rtt_p90_us', 'n/a')} µs |
| RTT p99 | {zd.get('rtt_p99_us', 'n/a')} µs |
| RTT max | {zd.get('rtt_max_us', 'n/a')} µs |
| RTT samples | {zd.get('rtt_count', 'n/a')} |
"""
else:
    content += "\n_(ZeroDDS-Self bench did not run or produced no output)_\n"

# Cross-Vendor (CI-3c)
xv = load("$OUTDIR/cross_vendor_throughput.json")
content += "\n## Cross-Vendor Throughput (CI-3c, ZeroDDS<->Cyclone)\n"
if xv:
    d1 = xv.get('direction_1_zerodds_pub_cyclone_sub', {})
    d2 = xv.get('direction_2_cyclone_pub_zerodds_sub', {})
    content += f"""
| Direction | Samples | Throughput (S/s) | Runtime |
|---|--:|--:|--:|
| ZeroDDS-Pub → Cyclone-Sub | {d1.get('samples_received', 'n/a')} | {d1.get('throughput_S_per_s', 'n/a')} | {d1.get('runtime_s', 'n/a')} s |
| Cyclone-Pub → ZeroDDS-Sub | {d2.get('samples_received', 'n/a')} | {d2.get('throughput_S_per_s', 'n/a')} | {d2.get('runtime_s', 'n/a')} s |
"""
else:
    content += "\n_(cross-vendor throughput did not run)_\n"

content += """

## Folge-Welle CI-3d

* ZeroDDS-C-API exponieren (cbindgen-Header + extern-C-shim)
* ZeroDDS-Plugin fuer Apex.AI performance_test schreiben
* Echte cross-vendor-Latenz mit timestamped Payloads
* Apex.AI performance_test gegen alle drei Vendoren parallel
"""
out.write_text(content)
print("[llvm-bench] wrote", out)
PY

echo "[llvm-bench] DONE — output in $OUTDIR/"
ls -la "$OUTDIR/"
