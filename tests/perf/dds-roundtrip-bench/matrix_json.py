#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0
#
# matrix_json.py — wandelt matrix_results.csv in eine strukturierte
# JSON-Datei, die der Website-Doc-Builder (website/_tools/) zu Claims
# verarbeitet. Maschinenlesbares Gegenstueck zum Markdown-Report.
#
# Aufruf:  python3 matrix_json.py <matrix_results.csv> [out.json]
#          (ohne out.json -> stdout)
#
# Schema (stabil — der Doc-Builder verlaesst sich darauf):
#   benchmark, date, host, config{}, vendors[], vendor_versions{},
#   payloads[], cells{total,ok,fail}, zerodds_fail_cells,
#   matrix{ "<payload>": { "<ping>": { "<pong>": {p50,p90,p99,status} } } },
#   headline{}, interop{}

import csv
import datetime
import json
import sys

VENDOR_VERSIONS = {
    "zerodds": "1.0.0-rc.2",
    "cyclone": "Cyclone DDS 0.10.5",
    "fastdds": "eProsima Fast-DDS 2.14",
    "rti": "RTI Connext 7.7.0",
    "opendds": "OpenDDS 3.31",
}


def num(s):
    """CSV-Feld -> float oder None."""
    try:
        return float(s)
    except (TypeError, ValueError):
        return None


def main() -> int:
    if len(sys.argv) < 2:
        print("usage: matrix_json.py <matrix_results.csv> [out.json]",
              file=sys.stderr)
        return 2

    with open(sys.argv[1], newline="") as fh:
        rows = list(csv.DictReader(fh))
    if not rows:
        print("error: leere CSV", file=sys.stderr)
        return 1

    vendors: list[str] = []
    payloads: list[int] = []
    for r in rows:
        if r["ping_vendor"] not in vendors:
            vendors.append(r["ping_vendor"])
        p = int(r["payload_bytes"])
        if p not in payloads:
            payloads.append(p)
    payloads.sort()

    # matrix[payload][ping][pong] = {p50,p90,p99,status}
    matrix: dict[str, dict] = {str(p): {pi: {} for pi in vendors}
                               for p in payloads}
    ok = fail = 0
    for r in rows:
        p, pi, po = r["payload_bytes"], r["ping_vendor"], r["pong_vendor"]
        cell = {"status": r["status"]}
        if r["status"] == "ok":
            ok += 1
            cell["p50_us"] = num(r["p50_us"])
            cell["p90_us"] = num(r["p90_us"])
            cell["p99_us"] = num(r["p99_us"])
        else:
            fail += 1
        matrix[p][pi][po] = cell

    zd_fail = sum(1 for r in rows
                  if r["status"] != "ok"
                  and "zerodds" in (r["ping_vendor"], r["pong_vendor"]))

    def self_p50(vendor: str, payload: int):
        c = matrix[str(payload)].get(vendor, {}).get(vendor)
        return c["p50_us"] if c and c.get("status") == "ok" else None

    pmin, pmax = payloads[0], payloads[-1]
    headline = {
        "self_p50_us": {
            v: {str(pmin): self_p50(v, pmin), str(pmax): self_p50(v, pmax)}
            for v in vendors
        },
    }

    out = {
        "benchmark": "cross-vendor-roundtrip-matrix",
        "date": datetime.date.today().isoformat(),
        "host": "x86-host (LXC, x86_64, Linux loopback)",
        "config": {
            "samples_per_cell": 2000,
            "warmup_per_cell": 200,
            "qos": "RELIABLE, KEEP_LAST(64)",
            "data_representation": "XCDR1",
            "transport": "UDPv4 loopback",
            "pong": "event-driven (Listener/Callback, kein Busy-Poll)",
        },
        "vendors": vendors,
        "vendor_versions": {v: VENDOR_VERSIONS.get(v, v) for v in vendors},
        "payloads_bytes": payloads,
        "cells": {"total": ok + fail, "ok": ok, "fail": fail},
        "zerodds_fail_cells": zd_fail,
        "interop": {
            "zerodds": "full" if zd_fail == 0 else "partial",
            "note": ("ZeroDDS interoperiert in beide Richtungen mit allen "
                     "Fremd-Stacks; 0 Fehlzellen."
                     if zd_fail == 0 else
                     f"{zd_fail} ZeroDDS-Zellen ohne Ergebnis."),
        },
        "headline": headline,
        "matrix": matrix,
    }

    text = json.dumps(out, indent=2, ensure_ascii=False)
    if len(sys.argv) >= 3:
        with open(sys.argv[2], "w") as fh:
            fh.write(text + "\n")
    else:
        print(text)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
