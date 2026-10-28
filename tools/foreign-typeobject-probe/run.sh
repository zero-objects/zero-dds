#!/bin/bash
# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 ZeroDDS Contributors
#
# run.sh — Cyclone DDS publishes probe::Sample with its own TypeInformation;
# zerodds-foreign-typeobject-probe fetches that TypeObject over TypeLookup,
# decodes Cyclone's samples with it, and the values are compared with what
# Cyclone was given.
#
#   tools/foreign-typeobject-probe/run.sh [domain]
#
# Needs a Python with the cyclonedds binding (PYTHON, default python3) and a
# built probe (cargo build -p zerodds-foreign-typeobject-probe).
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
root="$(cd "$here/../.." && pwd)"
domain="${1:-47}"
py="${PYTHON:-python3}"
probe="${PROBE:-${CARGO_TARGET_DIR:-$root/target}/debug/zerodds-foreign-typeobject-probe}"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

"$probe" --domain "$domain" --topic ProbeTopic --type-name probe::Sample --count 3 \
    > "$work/decoded" 2> "$work/probe.err" &
probe_pid=$!
sleep 1
"$py" "$here/cyclone_pub.py" --domain "$domain" --count 3 --expect "$work/expected" \
    2> "$work/pub.err"
status=0
wait "$probe_pid" || status=$?
if [ "$status" -ne 0 ]; then
    echo "probe failed ($status):" >&2
    cat "$work/probe.err" >&2
    exit 1
fi
"$py" - "$work/expected" "$work/decoded" <<'PY'
import json, math, sys
expected = [json.loads(l) for l in open(sys.argv[1], encoding="utf-8")]
lines = open(sys.argv[2], encoding="utf-8").read().splitlines()
print(lines[0])
decoded = [json.loads(l) for l in lines[1:]]
def same(a, b, path):
    if isinstance(a, dict):
        if not isinstance(b, dict) or set(a) != set(b):
            return [f"{path}: members {sorted(a)} against {sorted(b) if isinstance(b, dict) else b}"]
        return [m for k in a for m in same(a[k], b[k], f"{path}.{k}")]
    if isinstance(a, list):
        if not isinstance(b, list) or len(a) != len(b):
            return [f"{path}: {a} against {b}"]
        return [m for i, (x, y) in enumerate(zip(a, b)) for m in same(x, y, f"{path}[{i}]")]
    if isinstance(a, float) or isinstance(b, float):
        return [] if math.isclose(float(a), float(b)) else [f"{path}: {a} against {b}"]
    return [] if a == b else [f"{path}: {a!r} against {b!r}"]
misses = [m for e, d in zip(expected, decoded) for m in same(e, d, "sample")]
if len(decoded) != len(expected):
    misses.append(f"{len(decoded)} samples decoded, {len(expected)} written")
for m in misses:
    print("DIFFERENT", m)
print(f"{len(expected)} samples, {len(misses)} differences")
sys.exit(1 if misses else 0)
PY
