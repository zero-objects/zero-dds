#!/usr/bin/env bash
# Bench-Runner für codepit (LXC 111 auf pve).
#
# Ersetzt tests/perf/llvm_bench_runner.sh fuer den codepit-Host. Die
# llvm-Variante machte git-clone + war auf /home/llvm/-Paths verdrahtet
# und brauchte ssh; auf codepit liegt das Repo bereits unter
# /root/projects/zerodds und wir laufen lokal.
#
# Vendoren installiert in:
#   /opt/cyclone                       — Cyclone DDS 0.10.5 (from source)
#   /opt/rti.com/rti_connext_dds-7.3.0 — RTI Connext 7.3.0 (apt + license)
#   /opt/fastdds                       — Fast-DDS 2.14 (from source)
#
# Aufruf:
#   ./tests/perf/codepit_bench_runner.sh
#   RUNTIME_SECS=30 ./tests/perf/codepit_bench_runner.sh
#
# Output: $OUTDIR/{cyclone,zerodds,rti,fastdds}_perf.json + cross_vendor_*.json
#         + bench-summary.md

set -uo pipefail

REPO="${REPO:-/root/projects/zerodds}"
OUTDIR="${OUTDIR:-$REPO/bench-output-$(date +%Y%m%d-%H%M%S)}"
RUNTIME_SECS="${RUNTIME_SECS:-30}"
# Export damit die Python-heredocs OUTDIR + RUNTIME_SECS sehen.
export OUTDIR RUNTIME_SECS

mkdir -p "$OUTDIR"
cd "$REPO"

# ============================================================
# Stage 0: Builds
# ============================================================
echo "[bench] cargo build --release zerodds_perf + shapes_demo_{pub,sub} + spdp_demo"
cargo build --release --example zerodds_perf -p zerodds-dcps 2>&1 | tail -3
cargo build --release --example shapes_demo_publisher -p zerodds-dcps 2>&1 | tail -3
cargo build --release --example shapes_demo_subscriber -p zerodds-dcps 2>&1 | tail -3

ZD_PERF="$REPO/target/release/examples/zerodds_perf"
SHAPES_PUB="$REPO/target/release/examples/shapes_demo_publisher"
SHAPES_SUB="$REPO/target/release/examples/shapes_demo_subscriber"

for f in "$ZD_PERF" "$SHAPES_PUB" "$SHAPES_SUB"; do
    [ -x "$f" ] || { echo "MISSING: $f" >&2; exit 2; }
done

# ============================================================
# Stage 1: Cyclone-Self (ddsperf)
# ============================================================
if [ -x /opt/cyclone/bin/ddsperf ]; then
    echo "[bench] ===== Cyclone-Self ddsperf latency 1KB ($RUNTIME_SECS s) ====="
    export LD_LIBRARY_PATH=/opt/cyclone/lib
    # `timeout --kill-after=3s` sendet erst SIGTERM, dann SIGKILL —
    # ddsperf/rtiddsping/DDSHelloWorld ignorieren SIGINT, kill -INT
    # + wait hing fuer immer. timeout garantiert Termination.
    timeout --kill-after=3s "$RUNTIME_SECS"s /opt/cyclone/bin/ddsperf -D 100 -1 ping 1Hz size 1024 \
        >"$OUTDIR/cyclone_ping.log" 2>&1 &
    P=$!
    timeout --kill-after=3s "$RUNTIME_SECS"s /opt/cyclone/bin/ddsperf -D 100 pong \
        >"$OUTDIR/cyclone_pong.log" 2>&1 &
    Q=$!
    wait $P $Q 2>/dev/null || true

    python3 - <<'PY'
import json, re, pathlib, statistics, os
out = pathlib.Path(os.environ["OUTDIR"]) / "cyclone_perf.json"
log = (pathlib.Path(os.environ["OUTDIR"]) / "cyclone_ping.log").read_text(errors="replace")
pat = re.compile(
    r"mean\s+([\d.]+)us\s+min\s+([\d.]+)us\s+"
    r"50%\s+([\d.]+)us\s+90%\s+([\d.]+)us\s+99%\s+([\d.]+)us\s+max\s+([\d.]+)us"
)
hits = pat.findall(log)
result = {"vendor": "cyclone", "lines": len(hits)}
if hits:
    means=[float(h[0]) for h in hits]; mins=[float(h[1]) for h in hits]
    p50s =[float(h[2]) for h in hits]; p90s=[float(h[3]) for h in hits]
    p99s =[float(h[4]) for h in hits]; maxs=[float(h[5]) for h in hits]
    result.update({"mean_us":statistics.median(means), "min_us":min(mins),
                   "p50_us":statistics.median(p50s), "p90_us":statistics.median(p90s),
                   "p99_us":statistics.median(p99s), "max_us":max(maxs)})
else:
    result["error"] = "no histogram lines parsed"
out.write_text(json.dumps(result, indent=2))
print("wrote", out, ":", json.dumps(result))
PY
else
    echo "[bench] cyclone ddsperf not installed — skipping"
fi

# ============================================================
# Stage 2: ZeroDDS-Self
# ============================================================
echo "[bench] ===== ZeroDDS-Self zerodds_perf throughput 1KB ($RUNTIME_SECS s) ====="
"$ZD_PERF" pub 1024 "$RUNTIME_SECS" >"$OUTDIR/zerodds_pub.log" 2>&1 &
P=$!
"$ZD_PERF" sub "$RUNTIME_SECS" >"$OUTDIR/zerodds_sub.log" 2>&1 &
Q=$!
wait $P $Q 2>/dev/null || true

echo "[bench] ===== ZeroDDS-Self pingpong 30s ====="
"$ZD_PERF" pong 30 >"$OUTDIR/zerodds_pong.log" 2>&1 &
P=$!
sleep 1
"$ZD_PERF" pingpong 30 >"$OUTDIR/zerodds_pingpong.log" 2>&1 &
Q=$!
wait $P $Q 2>/dev/null || true

python3 - <<'PY'
import json, re, pathlib, statistics, os
out_dir = pathlib.Path(os.environ["OUTDIR"])
out = out_dir / "zerodds_perf.json"
result = {"vendor": "zerodds"}

sub = (out_dir / "zerodds_sub.log").read_text(errors="replace")
hits = re.findall(r"size N total (\d+) delta (\d+) rate ([\d.]+) kS/s", sub)
if hits:
    rates = [float(h[2]) for h in hits]
    result["throughput_kS_per_s_median"] = statistics.median(rates)
    result["samples_total"] = int(hits[-1][0])

pp = (out_dir / "zerodds_pingpong.log").read_text(errors="replace")
rtt = re.findall(r"rtt mean (\d+)us min (\d+) 50% (\d+) 90% (\d+) 99% (\d+) max (\d+) cnt (\d+)", pp)
if rtt:
    last = rtt[-1]
    result.update({"rtt_mean_us":int(last[0]), "rtt_min_us":int(last[1]),
                   "rtt_p50_us":int(last[2]), "rtt_p90_us":int(last[3]),
                   "rtt_p99_us":int(last[4]), "rtt_max_us":int(last[5]),
                   "rtt_count":int(last[6])})

out.write_text(json.dumps(result, indent=2))
print("wrote", out, ":", json.dumps(result))
PY

# ============================================================
# Stage 3: RTI-Self (rtiddsping)
# ============================================================
NDDSHOME=/opt/rti.com/rti_connext_dds-7.3.0
if [ -x "$NDDSHOME/bin/rtiddsping" ]; then
    echo "[bench] ===== RTI-Self rtiddsping 1KB ($RUNTIME_SECS s) ====="
    export NDDSHOME
    export RTI_LICENSE_FILE="${RTI_LICENSE_FILE:-$NDDSHOME/rti_license.dat}"
    export LD_LIBRARY_PATH="$NDDSHOME/lib/x64Linux4gcc7.3.0:${LD_LIBRARY_PATH:-}"
    # Subscriber zuerst (gibt SPDP-Discovery Vorlauf), dann Publisher.
    timeout --kill-after=3s "$RUNTIME_SECS"s "$NDDSHOME/bin/rtiddsping" -domainId 101 -subscriber \
        >"$OUTDIR/rti_sub.log" 2>&1 &
    Q=$!
    sleep 1
    timeout --kill-after=3s "$RUNTIME_SECS"s "$NDDSHOME/bin/rtiddsping" -domainId 101 -publisher \
        >"$OUTDIR/rti_pub.log" 2>&1 &
    P=$!
    wait $P $Q 2>/dev/null || true

    python3 - <<'PY'
import json, re, pathlib, os
out_dir = pathlib.Path(os.environ["OUTDIR"])
out = out_dir / "rti_perf.json"
sub = (out_dir / "rti_sub.log").read_text(errors="replace")
# rtiddsping 7.3 prints "rtiddsping, issue received: NNNNNNN" pro Sample
# (in aelteren Versionen war es "<- value: NNNNNNN").
samples = len(re.findall(r"issue received:|<-\s*value:", sub))
result = {"vendor": "rti", "samples_received": samples,
          "throughput_S_per_s": round(samples / float(os.environ["RUNTIME_SECS"]), 2)}
out.write_text(json.dumps(result, indent=2))
print("wrote", out, ":", json.dumps(result))
PY
else
    echo "[bench] RTI rtiddsping not installed — skipping"
fi

# ============================================================
# Stage 4: FastDDS-Self (DDSHelloWorldExample)
# ============================================================
FASTDDS_BC=/opt/fastdds/examples/cpp/dds/BasicConfigurationExample/bin/BasicConfigurationExample
if [ -x "$FASTDDS_BC" ]; then
    echo "[bench] ===== FastDDS-Self BasicConfigurationExample ($RUNTIME_SECS s) ====="
    export LD_LIBRARY_PATH="/opt/fastdds/lib:${LD_LIBRARY_PATH:-}"
    # HelloWorldExample subscriber printed nichts beim Receive — wir
    # nutzen BasicConfigurationExample der "Message HelloWorld N
    # RECEIVED" pro Sample loggt. -s 0 infinite, -i 5 ms = 200 S/s.
    timeout --kill-after=3s "$RUNTIME_SECS"s "$FASTDDS_BC" subscriber -s 0 \
        >"$OUTDIR/fastdds_sub.log" 2>&1 &
    Q=$!
    sleep 1
    timeout --kill-after=3s "$RUNTIME_SECS"s "$FASTDDS_BC" publisher -s 0 -i 5 \
        >"$OUTDIR/fastdds_pub.log" 2>&1 &
    P=$!
    wait $P $Q 2>/dev/null || true

    python3 - <<'PY'
import json, re, pathlib, os
out_dir = pathlib.Path(os.environ["OUTDIR"])
out = out_dir / "fastdds_perf.json"
sub = (out_dir / "fastdds_sub.log").read_text(errors="replace")
# BasicConfigurationExample subscriber prints "Message HelloWorld N RECEIVED".
samples = len(re.findall(r"RECEIVED", sub))
result = {"vendor": "fastdds", "samples_received": samples,
          "throughput_S_per_s": round(samples / float(os.environ["RUNTIME_SECS"]), 2)}
out.write_text(json.dumps(result, indent=2))
print("wrote", out, ":", json.dumps(result))
PY
else
    echo "[bench] Fast-DDS HelloWorldExample not installed — skipping"
fi

# ============================================================
# Stage 5: Cross-Vendor ZeroDDS↔Cyclone Throughput
# ============================================================
# Verwendet shapes_demo (Square-Topic, ShapeType). Cyclone-Seite:
# direkt ddsperf statt cyclonedds-python (Python-Variante existiert
# als tests/interop/cyclone_shapes_pub.py, braucht aber cyclonedds-
# python Install).
XV_RUNTIME=15
if [ -x /opt/cyclone/bin/ddsperf ]; then
    echo "[bench] ===== Cross-Vendor ZeroDDS↔Cyclone (SPDP-Discovery, $XV_RUNTIME s) ====="
    export LD_LIBRARY_PATH=/opt/cyclone/lib
    timeout --kill-after=3s $((XV_RUNTIME+2))s /opt/cyclone/bin/ddsperf -D 102 pub 10Hz >"$OUTDIR/xv_cy_pub.log" 2>&1 &
    P=$!
    sleep 1
    cargo build --release --example spdp_demo -p zerodds-discovery >/dev/null 2>&1
    DOMAIN_ID=102 timeout "$XV_RUNTIME"s "$REPO/target/release/examples/spdp_demo" \
        >"$OUTDIR/xv_zd_spdp.log" 2>&1 || true
    wait $P 2>/dev/null || true

    python3 - <<'PY'
import json, pathlib, os, re
out_dir = pathlib.Path(os.environ["OUTDIR"])
out = out_dir / "cross_vendor_spdp.json"
log = (out_dir / "xv_zd_spdp.log").read_text(errors="replace")
# Count Cyclone discoveries.
cyclone_seen = len(re.findall(r"vendor=VendorId\(\[1, 240\]\)", log))
result = {"zd_discovered_cyclone_participants": cyclone_seen}
out.write_text(json.dumps(result, indent=2))
print("wrote", out, ":", json.dumps(result))
PY
fi

# ============================================================
# Stage 6: Summary
# ============================================================
echo "[bench] ===== Summary ====="
python3 - <<'PY'
import json, pathlib, os
out_dir = pathlib.Path(os.environ["OUTDIR"])
def load(name):
    p = out_dir / name
    return json.loads(p.read_text()) if p.exists() else None

cy = load("cyclone_perf.json")
zd = load("zerodds_perf.json")
rti = load("rti_perf.json")
fd = load("fastdds_perf.json")
xv = load("cross_vendor_spdp.json")

lines = ["# Codepit Bench Summary\n"]
lines.append(f"* Runtime per stage: {os.environ['RUNTIME_SECS']} s\n")

lines.append("\n## Self-Tests\n")
lines.append("| Vendor | Throughput (kS/s) | RTT p50 (µs) | RTT p99 (µs) | Samples |\n|---|--:|--:|--:|--:|\n")
def fmt(v, k, default="-"):
    return v.get(k, default) if v else default
for name, d in [("cyclone", cy), ("zerodds", zd), ("rti", rti), ("fastdds", fd)]:
    if not d: continue
    thr = d.get("throughput_kS_per_s_median") or d.get("throughput_S_per_s", "-")
    rtt_p50 = d.get("rtt_p50_us") or d.get("p50_us", "-")
    rtt_p99 = d.get("rtt_p99_us") or d.get("p99_us", "-")
    samples = d.get("samples_total") or d.get("samples_received", "-")
    lines.append(f"| {name} | {thr} | {rtt_p50} | {rtt_p99} | {samples} |\n")

if xv:
    lines.append(f"\n## Cross-Vendor SPDP\n\n* ZeroDDS discovered Cyclone-Participants: {xv['zd_discovered_cyclone_participants']}\n")

(out_dir / "bench-summary.md").write_text("".join(lines))
print(open(out_dir / "bench-summary.md").read())
PY

echo "[bench] DONE — output in $OUTDIR/"
ls -la "$OUTDIR/"
