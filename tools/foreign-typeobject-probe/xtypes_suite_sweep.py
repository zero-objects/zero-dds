#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 ZeroDDS Contributors
"""Each publisher variant of the OMG dds-xtypes v2 suite, per vendor test_main
(for Fast DDS, fastdds_pub):
the probe reads one sample through the TypeObject the vendor announced and
compares it with the suite data file.

  xtypes_suite_sweep.py <probe> <vendor,...> <xcdr,...>

Run from anywhere; the suite is expected at /root/dds-xtypes-suite and the
vendor binaries at /root/omg-vendors/xtypes, as on the Linux test host."""
import json, math, os, re, shlex, subprocess, sys, time
sys.path.insert(0, "/root/dds-xtypes-suite")
os.chdir("/root/dds-xtypes-suite")
import test_suite
probe = sys.argv[1]
vendors = sys.argv[2].split(",")
xs = sys.argv[3].split(",")
variants = {}
for name, case in test_suite.__dict__.items():
    if not isinstance(case, dict):
        continue
    for key, c in case.items():
        if not isinstance(c, dict) or "apps" not in c:
            continue
        common = " ".join(c.get("common_args", []))
        for app in c["apps"]:
            if app.startswith("pub-exe"):
                variants[(common, app[len("pub-exe"):].strip())] = key
def number(v):
    """A JSON value as a number when it is one: the codec writes 64-bit
    integers and long doubles as strings, the suite writes a float128 as
    its sixteen octets in hex."""
    if isinstance(v, bool):
        return None
    if isinstance(v, (int, float)):
        return float(v)
    if isinstance(v, str):
        if v.startswith("0x") and len(v) == 34:
            raw = bytes.fromhex(v[2:])[::-1]
            sign = -1.0 if raw[0] & 0x80 else 1.0
            exp = ((raw[0] & 0x7F) << 8) | raw[1]
            frac = int.from_bytes(raw[2:], "big")
            if exp == 0 and frac == 0:
                return 0.0
            return sign * (1 + frac / (1 << 112)) * 2.0 ** (exp - 16383)
        try:
            return float(v)
        except ValueError:
            return None
    return None
def flat(v):
    return [y for x in v for y in flat(x)] if isinstance(v, list) else [v]
def same(a, b):
    if isinstance(a, dict):
        return isinstance(b, dict) and set(a) <= set(b) and all(same(a[k], b[k]) for k in a)
    if isinstance(a, list):
        if not isinstance(b, list):
            return False
        # The suite writes a multi-dimensional array flat.
        if len(a) != len(b) and any(isinstance(x, list) for x in b):
            b = flat(b)
        return len(a) == len(b) and all(same(x, y) for x, y in zip(a, b))
    na, nb = number(a), number(b)
    if na is not None and nb is not None:
        return math.isclose(na, nb, rel_tol=1e-6, abs_tol=1e-9)
    return a == b
# The suite has no Fast DDS test_main; fastdds_pub.cpp stands in for one
# (README.md).
fastdds_pub = os.environ.get("FASTDDS_PUB", "/root/fto-fastdds/fastdds_pub")
def publisher(vendor, argv, ty, topic, data, x):
    if vendor == "fastdds":
        type_file = argv[argv.index("--type-file") + 1]
        return [fastdds_pub, "--idl", f"types/idl/{type_file}.idl", "--type", ty, "--topic", topic,
                "--data", f"data/json/{data}.json", "--domain", "49", "--xcdr", x,
                "--seconds", "20"]
    return [f"/root/omg-vendors/xtypes/{vendor}_test_main_linux"] + argv + ["-r", "-x", x, "-d", "49"]
for vendor in vendors:
    for x in xs:
        tally = {}
        for (common, args), case in sorted(variants.items(), key=lambda kv: kv[1]):
            argv = shlex.split(common) + shlex.split(args)
            ty = argv[argv.index("-y") + 1]
            topic = argv[argv.index("-t") + 1]
            data = argv[argv.index("--data-file") + 1] if "--data-file" in argv else None
            dialect = {"connext_dds-7.7.0": "rti", "opendds-3.35.0-dev": "opendds",
                       "eclipse_cyclone-11.0.1": "cyclone",
                       "fastdds": "fastdds"}.get(vendor, "specification")
            if vendor == "fastdds" and (data is None or "--disable-type-info" in argv):
                verdict = "no-data-file" if data is None else "no-type-info"
                tally[verdict] = tally.get(verdict, 0) + 1
                print(f"{vendor} xcdr{x} {case} {ty}: {verdict}", flush=True)
                continue
            p = subprocess.Popen([probe, "--domain", "49", "--topic", topic, "--type-name", ty,
                                  "--count", "1", "--timeout-s", "12", "--dialect", dialect],
                                 stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
            time.sleep(0.5)
            pub = subprocess.Popen(publisher(vendor, argv, ty, topic, data, x),
                                   stdout=subprocess.DEVNULL, text=True,
                                   # Only fastdds_pub's few lines are read back; a
                                   # vendor test_main could fill a pipe and stall.
                                   stderr=subprocess.PIPE if vendor == "fastdds"
                                   else subprocess.DEVNULL)
            try:
                out, err = p.communicate(timeout=20)
            except subprocess.TimeoutExpired:
                p.kill()
                out, err = p.communicate()
            refused = pub.poll() not in (None, 0)
            pub.kill()
            pub_err = pub.communicate()[1] or ""
            lines = out.splitlines()
            if refused:
                # The publisher left before the probe read anything: Fast
                # DDS's own IDL parser or JSON reader refused the case.
                # Fast DDS logs in colour and without a newline before ours.
                plain = re.sub(r"\x1b\[[0-9;]*m", "", pub_err)
                said = re.findall(r"fastdds_pub: (.*)", plain)
                verdict = "pub-refused: " + (said or ["?"])[-1][:300]
            elif "--disable-type-info" in argv:
                verdict = "no-type-info"
            elif p.returncode != 0 or len(lines) < 2:
                verdict = "probe-failed: " + (err.strip().splitlines() or ["?"])[-1][:400]
            elif data is None:
                verdict = "no-data-file"
            else:
                expected = json.load(open(f"data/json/{data}.json"))
                got = json.loads(lines[1])
                verdict = "same" if same(expected, got) else \
                    "DIFFERENT got " + json.dumps(got) + " expected " + json.dumps(expected)
            kind = verdict.split(":")[0].split(" ")[0]
            tally[kind] = tally.get(kind, 0) + 1
            print(f"{vendor} xcdr{x} {case} {ty}: {verdict}", flush=True)
        print(f"== {vendor} xcdr{x}: {tally}", flush=True)
