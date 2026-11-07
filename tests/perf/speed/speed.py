#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0
"""The speed setup of rc8-plan.md D1: every stack measured the same way.

One host, every stack's self pair over UDPv4 (SHM separately, `--transport
shm`), the same type, the same sample sizes and the same QoS:

* the vendors' apps are the roundtrip apps of tests/perf/dds-roundtrip-bench,
  one per stack, generated from the one roundtrip.idl by the stack's own code
  generator (HDDS: a Rust app over its Rust API, hddsgen's Rust backend;
  Dust DDS: a Rust app over its Rust API, dust_dds_gen)
  (`RoundtripBench::Roundtrip`, @final, a uint32 sequence number, a
  uint64 time stamp and a `sequence<octet, 65536>`); ZeroDDS runs as the same
  C++ app over its C API (row `zerodds`) and as `zerodds_perf`, the Rust API
  (row `zerodds-rust`, crates/dcps/examples/zerodds_perf.rs);
* `tput`: one writer writes one sample over and over for WRITE_SECS, one
  reader takes in its listener; written and received samples, rate, CPU per
  sample of each process over its own measurement window;
* `lat`: ping/pong with one sample in flight, the echo taken in the
  listener; quantiles of the round trip;
* `ref` (row `zerodds-rust` only): the conditions of the RC7 values in
  internal/release/rc8-zero-copy.md, so the baseline connects to them.

Every cell runs on a domain of its own (150-209, outside the test leases).
Each invocation appends JSON lines to --out: a header (commit, host, kernel,
CPU, governor, vendor versions) and one line per cell and run. `baseline`
folds runs into tests/perf/baseline/speed.json, `compare` reads a new
measurement against it and reports; it never fails on a value.

Usage on the speed host (build.sh builds what it measures; pillar.sh and
ci/jobs/speed-pillar.yml run it weekly and on a release candidate):

  speed.py run --rows zerodds,zerodds-rust --build B --perf-bin P \\
      --label main --commit SHA --runs 1 --out results.jsonl
  speed.py baseline --in results.jsonl ... --out tests/perf/baseline/speed.json
  speed.py compare --baseline tests/perf/baseline/speed.json --in new.jsonl
"""

import argparse
import datetime
import json
import os
import platform
import re
import signal
import statistics
import subprocess
import sys
import time
from pathlib import Path

HERE = Path(__file__).resolve().parent
BENCH_DIR = HERE.parent / "dds-roundtrip-bench"

# The measurement points. Changing one makes old baselines incomparable.
SIZES = (64, 1024, 65536)
WRITE_SECS = 5
LAT_SAMPLES = 10000
LAT_WARMUP = 200
LAT_MAX_SECONDS = 20
SUB_IDLE_EXIT_MS = 2000
DOMAIN_FIRST = 150
DOMAIN_COUNT = 60


def max_samples(size):
    """RESOURCE_LIMITS max_samples (= max_samples_per_instance) of a cell:
    100 000 samples, at most 128 MiB of payload per history."""
    return min(100000, (128 << 20) // size)


QOS_NAMES = {"be": "besteffort", "rel": "reliable"}

# The rows: how each stack's app is started. `settle` is the time between
# starting the reader (or pong) and the writer (or ping).
ROWS = {
    "zerodds": {"app": "zerodds-roundtrip", "settle": 1.0},
    "zerodds-rust": {"perf": True, "settle": 1.0},
    "cyclone": {"app": "cyclone-roundtrip", "settle": 1.0},
    "fastdds": {"app": "fastdds-roundtrip", "settle": 1.0},
    "rti": {"app": "rti-roundtrip", "settle": 1.0},
    "opendds": {"app": "opendds-roundtrip", "settle": 3.0, "opendds": True},
    "opensplice": {"app": "opensplice-roundtrip", "settle": 1.0},
    # A Rust app over the HDDS API (dds-roundtrip-bench/hdds, its main.rs
    # says why), the same modes and output lines as the C++ apps.
    "hdds": {"app": "hdds-roundtrip", "settle": 1.0},
    # A Rust app over the dust_dds API (dds-roundtrip-bench/dust, its
    # main.rs says what differs), the same modes and output lines; Dust DDS
    # 0.16.0 has no shared-memory transport and its app refuses
    # ZERODDS_BENCH_TRANSPORT=SHM.
    "dust": {"app": "dust-roundtrip", "settle": 1.0},
    # ZeroDDS with the reliability timing HDDS 1.1.5 uses instead of the
    # specification's defaults, which stay ZeroDDS' own (rc8-plan.md, arc 3,
    # D3): its reader answers a HEARTBEAT at once (crates/hdds/src/core/
    # reader/proxy.rs), its writer answers a NACK at once (crates/hdds/src/
    # dds/writer/nack.rs), heartbeat period 100 ms on both sides. HDDS
    # sends at most one ACKNACK per 10 ms (`ACKNACK_RATE_LIMIT_MS`); ZeroDDS
    # has no such limit, and its `reader.heartbeat_suppression_duration_ms`
    # is another rule: it ignores a HEARTBEAT within the duration, and the
    # one a full flow-control window asks for was ignored, so the writer
    # waited for the periodic one (pipeline 9289: 4.85 kS/s RELIABLE at
    # 1 KiB with 10 ms, 126 kS/s without it on the Linux test host, 2026-11-05).
    "zerodds-hdds-timing": {"app": "zerodds-roundtrip", "settle": 1.0, "env": {
        "ZERODDS_READER_HEARTBEAT_RESPONSE_DELAY_MS": "0",
        "ZERODDS_NACK_RESPONSE_DELAY_MS": "0",
    }},
}

# The row the targets are set against (rc8-plan.md D3: equal to or better
# than HDDS in every cell, HDDS measured in the same run).
TARGET_ROW = "hdds"

NDDSHOME = os.environ.get("NDDSHOME", "/opt/rti.com/rti_connext_dds-7.7.0")
OSPL_HOME = os.environ.get("OSPL_HOME", "/opt/opensplice-6.9")


def cells_of(row, kinds):
    """The cells a row runs, in order: (kind, qos, size)."""
    out = []
    if "tput" in kinds:
        for size in SIZES:
            for qos in ("be", "rel"):
                out.append(("tput", qos, size))
    if "lat" in kinds and not ROWS[row].get("perf"):
        for size in SIZES:
            out.append(("lat", "rel", size))
    if ROWS[row].get("perf"):
        # `ref-be` separately: under these conditions the rc.7 and main
        # reader stalls in one take of the full cache (100 000 samples,
        # 47 s on the Linux test host) and the cell ends in sub-timeout.
        if "ref-be" in kinds:
            out.append(("ref", "be", 1024))
        if "ref" in kinds:
            out.append(("ref", "rel", 1024))
    return out


# ---------------------------------------------------------------- host facts


def read(path, default=""):
    try:
        return Path(path).read_text().strip()
    except OSError:
        return default


def host_facts():
    cpu = ""
    for line in read("/proc/cpuinfo").splitlines():
        if line.startswith("model name"):
            cpu = line.split(":", 1)[1].strip()
            break
    try:
        ncpu = len(os.sched_getaffinity(0))
    except AttributeError:
        ncpu = os.cpu_count()
    return {
        "host": platform.node(),
        "kernel": platform.release(),
        "cpu_model": cpu,
        "cpus": ncpu,
        "governor": read("/sys/devices/system/cpu/cpu0/cpufreq/scaling_governor", "n/a"),
        "mem_total_kb": next(
            (int(l.split()[1]) for l in read("/proc/meminfo").splitlines() if l.startswith("MemTotal")),
            0,
        ),
    }


def vendor_versions():
    def grep(path, pattern):
        m = re.search(pattern, read(path), re.M)
        return m.group(1) if m else "unknown"

    fastdds = "unknown"
    try:
        libs = sorted(p.name for p in Path("/opt/fastdds/lib").glob("libfastdds.so.*.*.*"))
        if libs:
            fastdds = libs[-1].split(".so.", 1)[1]
    except OSError:
        pass
    ospl = "unknown"
    info = read(f"{OSPL_HOME}/etc/RELEASEINFO")
    if info:
        f = dict(re.findall(r'^(\w+)="([^"]*)"', info, re.M))
        ospl = f"{f.get('OSPL_MAJOR')}.{f.get('OSPL_MINOR')}.{f.get('OSPL_MAINT')}{f.get('OSPL_PATCH', '')}"
    rti = re.search(r"rti_connext_dds-([\d.]+)", NDDSHOME)
    # HDDS is built from source by build.sh at the tag its app pins.
    hdds = grep(str(BENCH_DIR / "hdds" / "Cargo.toml"), r'^hdds = .*tag = "v([^"]+)"')
    # Dust DDS likewise, from crates.io at the version its app pins.
    dust = grep(str(BENCH_DIR / "dust" / "Cargo.toml"), r'^dust_dds = "=([^"]+)"')
    return {
        "cyclone": grep("/opt/cyclone/lib/pkgconfig/CycloneDDS.pc", r"^Version:\s*(\S+)"),
        "fastdds": fastdds,
        "rti": rti.group(1) if rti else "unknown",
        "opendds": grep("/opt/opendds/include/dds/Version.h", r'#define OPENDDS_VERSION "([^"]+)"'),
        "opensplice": ospl,
        "hdds": hdds,
        "dust": dust,
    }


def load1():
    try:
        return float(read("/proc/loadavg").split()[0])
    except (ValueError, IndexError):
        return -1.0


def udp_rcvbuf_errors():
    """RcvbufErrors of /proc/net/snmp: datagrams the kernel dropped for a
    full socket receive buffer (this network namespace)."""
    lines = [l.split() for l in read("/proc/net/snmp").splitlines() if l.startswith("Udp:")]
    if len(lines) >= 2 and "RcvbufErrors" in lines[0]:
        return int(lines[1][lines[0].index("RcvbufErrors")])
    return -1


def busy_processes():
    """Processes that would load the host during a measurement: builds and
    matrix cells (the matrix runner on the Linux test host)."""
    try:
        out = subprocess.run(["pgrep", "-a", "-f", r"(^|/)(cargo|rustc|cc1plus|run\.sh) "],
                             capture_output=True, text=True, timeout=10).stdout
    except (OSError, subprocess.SubprocessError):
        return []
    return [l for l in out.splitlines() if "speed.py" not in l][:10]


# ---------------------------------------------------------------- processes


def base_env(args):
    env = dict(os.environ)
    # OpenDDS is not on the path: its app finds its libraries through the
    # RUNPATH its CMake package sets (the security-enabled build of the
    # same version on the Linux test host); /opt/opendds/lib on the path
    # loads the other build and the app fails with an undefined symbol.
    libs = [
        "/opt/cyclone/lib",
        "/opt/fastdds/lib",
        f"{NDDSHOME}/lib/x64Linux4gcc8.5.0",
    ]
    if env.get("LD_LIBRARY_PATH"):
        libs.append(env["LD_LIBRARY_PATH"])
    env["LD_LIBRARY_PATH"] = ":".join(libs)
    env["NDDSHOME"] = NDDSHOME
    env.setdefault("RTI_LICENSE_FILE", f"{NDDSHOME}/rti_license.dat")
    env["OSPL_HOME"] = OSPL_HOME
    if args.transport == "shm":
        env["ZERODDS_BENCH_TRANSPORT"] = "SHM"
        env["ZERODDS_SAME_HOST_SHM"] = "1"
    else:
        env.pop("ZERODDS_BENCH_TRANSPORT", None)
        # ZeroDDS would take its same-host shared-memory channel for a
        # ZeroDDS peer; every other stack runs UDPv4 here.
        env["ZERODDS_SAME_HOST_SHM"] = "0"
    for k in [k for k in env if k.startswith(("ZERODDS_BENCH_", "ZERODDS_PERF_"))]:
        if k != "ZERODDS_BENCH_TRANSPORT":
            del env[k]
    return env


def cell_env(args, row, kind, qos, size, domain):
    env = base_env(args)
    limit = str(max_samples(size))
    if ROWS[row].get("perf"):
        env["ZERODDS_PERF_DOMAIN"] = str(domain)
        env["ZERODDS_PERF_RELIABILITY"] = QOS_NAMES[qos]
        env["ZERODDS_PERF_HISTORY"] = "all"
        env["ZERODDS_PERF_MAX_SAMPLES"] = limit
        env["ZERODDS_PERF_SUB_IDLE_EXIT_MS"] = str(SUB_IDLE_EXIT_MS)
        if kind == "ref":
            # The conditions of the RC7 values (rc8-zero-copy.md, the
            # cell scripts of 2026-10-08): 1 kB, KEEP_ALL, max_samples
            # 100 000, LATENCY_BUDGET 1 ms, receive batch 32, and the reader
            # answering a HEARTBEAT at once, which was the default then
            # (reader.heartbeat_response_delay_ms, 500 ms since 2026-10-19).
            env["ZERODDS_PERF_MAX_SAMPLES"] = "100000"
            env["ZERODDS_PERF_LATENCY_BUDGET_NS"] = "1000000"
            env["ZERODDS_RECV_BATCH"] = "32"
            env["ZERODDS_BATCH_AT_ZERO_BUDGET"] = "0"
            env["ZERODDS_READER_HEARTBEAT_RESPONSE_DELAY_MS"] = "0"
    else:
        env["ZERODDS_BENCH_DOMAIN"] = str(domain)
        env["ZERODDS_BENCH_RELIABILITY"] = QOS_NAMES[qos]
        env["ZERODDS_BENCH_HISTORY"] = "all"
        env["ZERODDS_BENCH_MAX_SAMPLES"] = limit
    env.update(ROWS[row].get("env", {}))
    return env


def app_cmd(args, row, *mode):
    spec = ROWS[row]
    if spec.get("perf"):
        return [args.perf_bin, *mode]
    cmd = [str(Path(args.build) / spec["app"]), *mode]
    if spec.get("opendds"):
        ini = BENCH_DIR / ("opendds_rtps_shm.ini" if args.transport == "shm" else "opendds_rtps.ini")
        cmd += ["-DCPSConfigFile", str(ini)]
    return cmd


def spawn(cmd, env, log):
    f = open(log, "w")
    return subprocess.Popen(cmd, env=env, stdout=f, stderr=subprocess.STDOUT,
                            start_new_session=True, cwd=log.parent)


def stop(p, grace=3.0):
    if p is None or p.poll() is not None:
        return
    try:
        os.killpg(p.pid, signal.SIGTERM)
        p.wait(timeout=grace)
    except subprocess.TimeoutExpired:
        os.killpg(p.pid, signal.SIGKILL)
        p.wait(timeout=10)
    except ProcessLookupError:
        pass


def wait_or_stop(p, timeout):
    try:
        p.wait(timeout=timeout)
        return True
    except subprocess.TimeoutExpired:
        stop(p)
        return False


# ---------------------------------------------------------------- parsing

RE_PUB = re.compile(r"tput-pub: payload=(\d+) written=(\d+) failed=(\d+) window_ns=(\d+) "
                    r"cpu_ns=(\d+) acked=(-?\d+) ack_ns=(\d+)")
RE_SUB = re.compile(r"tput-sub: received=(\d+) bytes=(\d+) first_seq=(\d+) last_seq=(\d+) "
                    r"gaps=(\d+) reordered=(\d+) rx_window_ns=(\d+) cpu_ns=(\d+) cpu_samples=(\d+)")
RE_PERF_PUB_CPU = re.compile(r"# pub-cpu: cpu_ns=(\d+) window_ns=(\d+) samples=(\d+)")
RE_PERF_PUB = re.compile(r"# pub-done: total=(\d+)")
RE_PERF_SUB_CPU = re.compile(r"# sub-cpu: cpu_ns=(\d+) cpu_samples=(\d+)")
RE_PERF_SUB = re.compile(r"# sub-done: total=(\d+) runtime=\S+ first_seq=(-?\d+) last_seq=(-?\d+) "
                         r"span=(-?\d+) missing=(-?\d+)")
RE_SENT = re.compile(r"ping-sent: (\d+)")
RE_LAT = re.compile(r"payload=(\d+)\s+n=(\d+)\s+min=([\d.e+-]+)us\s+p50=([\d.e+-]+)us\s+"
                    r"p90=([\d.e+-]+)us\s+p99=([\d.e+-]+)us\s+p999=([\d.e+-]+)us\s+max=([\d.e+-]+)us")


def last_match(regex, text):
    m = None
    for m in regex.finditer(text):
        pass
    return m


def parse_tput(row, size, pub_log, sub_log):
    pub, sub = read(pub_log), read(sub_log)
    r = {}
    if ROWS[row].get("perf"):
        pc, pd = last_match(RE_PERF_PUB_CPU, pub), last_match(RE_PERF_PUB, pub)
        sc, sd = last_match(RE_PERF_SUB_CPU, sub), last_match(RE_PERF_SUB, sub)
        if not (pc and pd and sd):
            return None
        r.update(written=int(pd.group(1)), failed=0, window_ns=int(pc.group(2)),
                 pub_cpu_ns=int(pc.group(1)), received=int(sd.group(1)),
                 gaps=max(0, int(sd.group(5))), sub_cpu_ns=int(sc.group(1)) if sc else 0,
                 sub_cpu_samples=int(sc.group(2)) if sc else 0, rx_window_ns=0)
    else:
        p, s = last_match(RE_PUB, pub), last_match(RE_SUB, sub)
        if not (p and s):
            return None
        r.update(written=int(p.group(2)), failed=int(p.group(3)), window_ns=int(p.group(4)),
                 pub_cpu_ns=int(p.group(5)), acked=int(p.group(6)), ack_ns=int(p.group(7)),
                 received=int(s.group(1)), gaps=int(s.group(5)), reordered=int(s.group(6)),
                 rx_window_ns=int(s.group(7)), sub_cpu_ns=int(s.group(8)),
                 sub_cpu_samples=int(s.group(9)))
    w, secs = r["written"], r["window_ns"] / 1e9
    r["ks_per_s"] = round(w / secs / 1000, 3) if secs > 0 else 0.0
    r["mb_per_s"] = round(w * size / secs / 1e6, 3) if secs > 0 else 0.0
    r["received_pct"] = round(100.0 * r["received"] / w, 4) if w else 0.0
    if r.get("rx_window_ns"):
        r["rx_ks_per_s"] = round(r["received"] / (r["rx_window_ns"] / 1e9) / 1000, 3)
    r["pub_cpu_us"] = round(r["pub_cpu_ns"] / w / 1000, 4) if w else None
    r["sub_cpu_us"] = (round(r["sub_cpu_ns"] / r["sub_cpu_samples"] / 1000, 4)
                       if r["sub_cpu_samples"] else None)
    return r


def parse_lat(ping_log):
    text = read(ping_log)
    m = last_match(RE_LAT, text)
    if not m:
        return None
    n = int(m.group(2))
    s = last_match(RE_SENT, text)
    sent = int(s.group(1)) if s else LAT_WARMUP + LAT_SAMPLES
    return {
        "n": n,
        "sent": sent,
        # Echoes not back within the 50 ms the ping waits for each, after
        # the warm-up; a run stopped at LAT_MAX_SECONDS sent fewer.
        "lost": max(0, sent - LAT_WARMUP - n),
        "min_us": float(m.group(3)),
        "p50_us": float(m.group(4)),
        "p90_us": float(m.group(5)),
        "p99_us": float(m.group(6)),
        "p999_us": float(m.group(7)),
        "max_us": float(m.group(8)),
    }


# ---------------------------------------------------------------- cells


def run_cell(args, row, kind, qos, size, domain, logs):
    env = cell_env(args, row, kind, qos, size, domain)
    settle = ROWS[row]["settle"]
    result = {"status": "ok"}
    if kind in ("tput", "ref"):
        pub_log, sub_log = logs / "pub.log", logs / "sub.log"
        if ROWS[row].get("perf"):
            sub_cmd = app_cmd(args, row, "sub", str(WRITE_SECS + 45))
            pub_cmd = app_cmd(args, row, "pub", str(size), str(WRITE_SECS))
        else:
            sub_cmd = app_cmd(args, row, "tput-sub", "--seconds", str(WRITE_SECS + 45))
            pub_cmd = app_cmd(args, row, "tput-pub", "--payload", str(size),
                              "--seconds", str(WRITE_SECS))
        sub = spawn(sub_cmd, env, sub_log)
        time.sleep(settle)
        pub = spawn(pub_cmd, env, pub_log)
        if not wait_or_stop(pub, WRITE_SECS + 60):
            result["status"] = "pub-timeout"
        if not wait_or_stop(sub, 45):
            result["status"] = "sub-timeout"
        stop(pub)
        stop(sub)
        parsed = parse_tput(row, size, pub_log, sub_log)
    else:
        ping_log, pong_log = logs / "ping.log", logs / "pong.log"
        pong = spawn(app_cmd(args, row, "pong", "120"), env, pong_log)
        time.sleep(settle + 1.0)
        ping = spawn(app_cmd(args, row, "ping", "--payload", str(size), "--samples",
                             str(LAT_SAMPLES), "--warmup", str(LAT_WARMUP),
                             "--max-seconds", str(LAT_MAX_SECONDS)), env, ping_log)
        if not wait_or_stop(ping, LAT_MAX_SECONDS + 40):
            result["status"] = "ping-timeout"
        stop(pong)
        parsed = parse_lat(ping_log)
    if parsed is None:
        result["status"] = result["status"] if result["status"] != "ok" else "no-result"
        tails = {}
        for f in sorted(logs.glob("*.log")):
            tails[f.stem] = read(f).splitlines()[-6:]
        result["log_tail"] = tails
    else:
        result.update(parsed)
    return result


def cmd_run(args):
    rows = [r for r in args.rows.split(",") if r]
    for r in rows:
        if r not in ROWS:
            sys.exit(f"unknown row {r}; rows: {', '.join(ROWS)}")
        if ROWS[r].get("perf") and not args.perf_bin:
            sys.exit(f"row {r} needs --perf-bin")
        if not ROWS[r].get("perf") and not args.build:
            sys.exit(f"row {r} needs --build")
    # The apps run in their log directories: absolute paths.
    if args.build:
        args.build = str(Path(args.build).resolve())
    if args.perf_bin:
        args.perf_bin = str(Path(args.perf_bin).resolve())
    kinds = set(args.kinds.split(","))
    out = open(args.out, "a")
    logs_root = Path(args.logs)
    logs_root.mkdir(parents=True, exist_ok=True)
    header = {
        "type": "header",
        "label": args.label,
        "commit": args.commit,
        "transport": args.transport,
        "started": datetime.datetime.now(datetime.timezone.utc).isoformat(timespec="seconds"),
        "write_secs": WRITE_SECS,
        "lat_samples": LAT_SAMPLES,
        "lat_max_seconds": LAT_MAX_SECONDS,
        "rows": rows,
        "busy_processes": busy_processes(),
        **host_facts(),
        "vendors": vendor_versions(),
    }
    out.write(json.dumps(header) + "\n")
    out.flush()
    domain_seq = int(time.time()) % DOMAIN_COUNT
    for run in range(1, args.runs + 1):
        for row in rows:
            for kind, qos, size in cells_of(row, kinds):
                domain = DOMAIN_FIRST + domain_seq % DOMAIN_COUNT
                domain_seq += 1
                logs = logs_root / f"{row}-{kind}-{qos}-{size}-r{run}"
                logs.mkdir(parents=True, exist_ok=True)
                before = {"load1": load1(), "udp": udp_rcvbuf_errors(), "t": time.time()}
                res = run_cell(args, row, kind, qos, size, domain, logs)
                line = {
                    "type": "cell",
                    "label": args.label,
                    "commit": args.commit,
                    "row": row,
                    "kind": kind,
                    "qos": qos,
                    "size": size,
                    "run": args.run_offset + run,
                    "domain": domain,
                    "load1": before["load1"],
                    "seconds": round(time.time() - before["t"], 1),
                    "udp_rcvbuf_errors": udp_rcvbuf_errors() - before["udp"],
                    **res,
                }
                out.write(json.dumps(line) + "\n")
                out.flush()
                brief = {k: line.get(k) for k in ("ks_per_s", "received_pct", "pub_cpu_us",
                                                  "sub_cpu_us", "p50_us", "p99_us")
                         if line.get(k) is not None}
                print(f"{row:13} {kind:4} {qos:3} {size:6} r{line['run']} {res['status']} "
                      f"{json.dumps(brief)}", flush=True)
                time.sleep(1.0)
    out.close()


# ---------------------------------------------------------------- baseline

# The values of a cell and which way is better.
METRICS = {
    "tput": [("ks_per_s", "kS/s", +1), ("mb_per_s", "MB/s", +1), ("received_pct", "%", +1),
             ("pub_cpu_us", "us/sample", -1), ("sub_cpu_us", "us/sample", -1)],
    "ref": [("ks_per_s", "kS/s", +1), ("received_pct", "%", +1),
            ("pub_cpu_us", "us/sample", -1), ("sub_cpu_us", "us/sample", -1)],
    "lat": [("p50_us", "us", -1), ("p99_us", "us", -1), ("p999_us", "us", -1), ("lost", "samples", -1)],
}


def load_lines(paths):
    headers, cells = [], []
    for p in paths:
        for raw in Path(p).read_text().splitlines():
            if not raw.strip():
                continue
            d = json.loads(raw)
            (headers if d.get("type") == "header" else cells).append(d)
    return headers, cells


def series_key(c):
    return (c["row"], c["label"], c["kind"], c["qos"], c["size"])


def fold(cells):
    groups = {}
    for c in cells:
        groups.setdefault(series_key(c), []).append(c)
    values = []
    for (row, label, kind, qos, size), cs in sorted(groups.items()):
        cs.sort(key=lambda c: c["run"])
        # A run whose writer or reader had to be stopped still has values
        # when it printed them (e.g. a RELIABLE writer still waiting for its
        # acknowledgments): they count, and the status says what happened.
        statuses = {}
        for c in cs:
            statuses[c.get("status", "?")] = statuses.get(c.get("status", "?"), 0) + 1
        for metric, unit, better in METRICS[kind]:
            runs = [c.get(metric) for c in cs if c.get(metric) is not None]
            entry = {"row": row, "label": label, "kind": kind, "qos": qos, "size": size,
                     "metric": metric, "unit": unit, "better": "higher" if better > 0 else "lower",
                     "n": len(runs), "statuses": statuses, "runs": runs}
            if runs:
                med = statistics.median(runs)
                entry.update(median=round(med, 4), min=min(runs), max=max(runs),
                             spread_pct=round(100.0 * (max(runs) - min(runs)) / med, 2) if med else None)
            values.append(entry)
    return values


def cmd_baseline(args):
    headers, cells = load_lines(args.inputs)
    commits = {}
    for h in headers:
        commits.setdefault(h["label"], set()).add(h.get("commit"))
    first = headers[0] if headers else {}
    starts = sorted(h["started"] for h in headers)
    doc = {
        "setup": "tests/perf/speed/speed.py",
        "commits": {k: sorted(v) for k, v in commits.items()},
        "reference": args.reference,
        "host": first.get("host"),
        "kernel": first.get("kernel"),
        "cpu_model": first.get("cpu_model"),
        "cpus": first.get("cpus"),
        "governor": first.get("governor"),
        "transport": first.get("transport"),
        "vendors": first.get("vendors"),
        "measured_from": starts[0] if starts else None,
        "measured_to": starts[-1] if starts else None,
        "write_secs": first.get("write_secs"),
        "lat_samples": first.get("lat_samples"),
        "sizes": list(SIZES),
        "max_samples": {str(s): max_samples(s) for s in SIZES},
        "busy_processes": sorted({p for h in headers for p in h.get("busy_processes", [])}),
        "load1_range": [min(c["load1"] for c in cells), max(c["load1"] for c in cells)] if cells else None,
        "notes": args.note,
        "values": fold(cells),
    }
    Path(args.out).write_text(json.dumps(doc, indent=1) + "\n")
    print(f"{args.out}: {len(doc['values'])} values from {len(cells)} cell runs")


def against_target(values):
    """Every ZeroDDS row of this run against TARGET_ROW in the same run,
    cell by cell: the targets of rc8-plan.md D3. Report only."""
    target = {(v["kind"], v["qos"], v["size"], v["metric"]): v
              for v in values if v["row"] == TARGET_ROW and v.get("n")}
    if not target:
        return [f"speed: no {TARGET_ROW} values in this run, no comparison with the target"]
    lines = [f"speed: against {TARGET_ROW} of this run (rc8-plan.md D3, equal or better)",
             f"{'row':20} {'cell':16} {'metric':13} {'zerodds':>10} {TARGET_ROW:>10}  verdict"]
    behind = 0
    for v in values:
        if not v["row"].startswith("zerodds") or not v.get("n"):
            continue
        t = target.get((v["kind"], v["qos"], v["size"], v["metric"]))
        if not t:
            continue
        ok = v["median"] >= t["median"] if t["better"] == "higher" else v["median"] <= t["median"]
        behind += not ok
        cell = f"{v['kind']}/{v['qos']}/{v['size']}"
        lines.append(f"{v['row']:20} {cell:16} {v['metric']:13} {v['median']:>10g} {t['median']:>10g}  "
                     f"{'equal-or-better' if ok else 'behind'}")
    lines.append(f"speed: {behind} value(s) behind {TARGET_ROW}")
    return lines


def cmd_compare(args):
    base = json.loads(Path(args.baseline).read_text())
    _, cells = load_lines(args.inputs)
    ref = args.reference or base.get("reference")
    # ZeroDDS rows against the reference label; the vendors' rows (label
    # `vendors` in the baseline, the same build whatever ZeroDDS commit)
    # against their own runs, so a host that moved shows in them too.
    ref_values = {(v["row"], v["kind"], v["qos"], v["size"], v["metric"]): v
                  for v in base["values"]
                  if v.get("n") and (v["label"] == ref
                                     or (not v["row"].startswith("zerodds") and v["label"] == "vendors"))}
    new_values = fold(cells)
    lines = [f"speed: against baseline label {ref} ({args.baseline}, measured "
             f"{base.get('measured_from')} on {base.get('host')}); outside = beyond the range of "
             f"the baseline runs; report only",
             f"{'row':13} {'cell':16} {'metric':13} {'baseline median [min..max]':32} {'now':>10} "
             f"{'delta':>8}  flag"]
    report = []
    outside = 0
    for v in new_values:
        if not v.get("n"):
            lines.append(f"{v['row']:13} {v['kind']}/{v['qos']}/{v['size']:<8} {v['metric']:13} "
                         f"no result ({v['statuses']})")
            report.append({**v, "flag": "no-result"})
            continue
        b = ref_values.get((v["row"], v["kind"], v["qos"], v["size"], v["metric"]))
        if not b:
            continue
        now = v["median"]
        delta = 100.0 * (now - b["median"]) / b["median"] if b["median"] else 0.0
        worse = now < b["min"] if b["better"] == "higher" else now > b["max"]
        better = now > b["max"] if b["better"] == "higher" else now < b["min"]
        flag = "outside-worse" if worse else ("outside-better" if better else "")
        outside += bool(worse)
        cell = f"{v['kind']}/{v['qos']}/{v['size']}"
        rng = f"{b['median']:g} [{b['min']:g}..{b['max']:g}]"
        lines.append(f"{v['row']:13} {cell:16} {v['metric']:13} {rng:32} {now:>10g} {delta:>+7.1f}%  {flag}")
        report.append({"row": v["row"], "kind": v["kind"], "qos": v["qos"], "size": v["size"],
                       "metric": v["metric"], "baseline": b["median"], "baseline_min": b["min"],
                       "baseline_max": b["max"], "now": now, "now_runs": v["runs"],
                       "delta_pct": round(delta, 2), "flag": flag})
    lines.append(f"speed: {outside} value(s) worse than every baseline run")
    lines.extend(against_target(new_values))
    print("\n".join(lines))
    if args.report:
        Path(args.report).write_text(json.dumps({"reference": ref, "values": report}, indent=1) + "\n")
    return 0


def main():
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    sub = ap.add_subparsers(dest="cmd", required=True)
    r = sub.add_parser("run", help="measure")
    r.add_argument("--rows", required=True, help=f"comma list of {', '.join(ROWS)}")
    r.add_argument("--kinds", default="tput,lat,ref", help="tput,lat,ref,ref-be")
    r.add_argument("--build", help="build directory of tests/perf/dds-roundtrip-bench")
    r.add_argument("--perf-bin", help="zerodds_perf binary (row zerodds-rust)")
    r.add_argument("--label", required=True, help="what the ZeroDDS rows were built from, e.g. rc7, main")
    r.add_argument("--commit", default="unknown")
    r.add_argument("--runs", type=int, default=1)
    r.add_argument("--run-offset", type=int, default=0, help="number of the first run minus one")
    r.add_argument("--transport", choices=["udp", "shm"], default="udp")
    r.add_argument("--out", required=True)
    r.add_argument("--logs", default="speed-logs")
    b = sub.add_parser("baseline", help="fold runs into a baseline file")
    b.add_argument("--in", dest="inputs", nargs="+", required=True)
    b.add_argument("--out", required=True)
    b.add_argument("--reference", default="rc7", help="label the speed pillar compares against")
    b.add_argument("--note", action="append", default=[], help="a line for the file's header")
    c = sub.add_parser("compare", help="report a measurement against the baseline")
    c.add_argument("--baseline", required=True)
    c.add_argument("--in", dest="inputs", nargs="+", required=True)
    c.add_argument("--reference", help="label in the baseline (default: its `reference`)")
    c.add_argument("--report", help="write the comparison as JSON")
    args = ap.parse_args()
    if args.cmd == "run":
        cmd_run(args)
    elif args.cmd == "baseline":
        cmd_baseline(args)
    else:
        sys.exit(cmd_compare(args))


if __name__ == "__main__":
    main()
