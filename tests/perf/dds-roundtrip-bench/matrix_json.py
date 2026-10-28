#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0
#
# matrix_json.py — converts matrix_results.csv into a structured
# JSON file that the website doc builder (website/_tools/) processes into claims.
# Machine-readable counterpart to the markdown report.
#
# Usage:   python3 matrix_json.py <matrix_results.csv> [out.json]
#                 [--keep <earlier.json> <vendor>]
#          (without out.json -> stdout)
#
# --unmeasurable <a> <b> <reason> marks every cell between a and b, both
# directions, as `not-measured` with the reason instead of failed: a pair the
# host cannot measure (RTI's trial-end licence admits 12 DataReaders, and an
# OpenSplice peer announces more of its own) is not an interop gap.
#
# --keep takes every cell with <vendor> on either side from an earlier
# matrix and marks it with that matrix's date (`measured`), for a vendor
# that cannot be measured again: RTI Connext while its licence is expired.
# `measured` in the output names the date each vendor's cells come from.
#
# Schema (stable — the doc builder relies on it):
#   benchmark, date, host, config{}, vendors[], vendor_versions{},
#   payloads[], cells{total,ok,fail}, zerodds_fail_cells,
#   matrix{ "<payload>": { "<ping>": { "<pong>":
#            {status, n, min, p50, p90, p99, p999, max (µs), measured?} } } },
#   headline{}, interop{}

import csv
import datetime
import json
import sys

VENDOR_VERSIONS = {
    "zerodds": "ZeroDDS 1.0.0-rc.7",
    "cyclone": "Cyclone DDS 11.0.1",
    "fastdds": "eProsima Fast DDS 3.6.1",
    "rti": "RTI Connext 7.7.0",
    "opendds": "OpenDDS 3.34.0",
    "opensplice": "OpenSplice CE 6.9.210323",
}


def num(s):
    """CSV field -> float or None."""
    try:
        return float(s)
    except (TypeError, ValueError):
        return None


def main() -> int:
    args = sys.argv[1:]
    unmeasurable = []
    while "--unmeasurable" in args:
        i = args.index("--unmeasurable")
        if len(args) < i + 4:
            print("usage: --unmeasurable <a> <b> <reason>", file=sys.stderr)
            return 2
        unmeasurable.append((args[i + 1], args[i + 2], args[i + 3]))
        del args[i:i + 4]
    keep = None
    if "--keep" in args:
        i = args.index("--keep")
        if len(args) < i + 3:
            print("usage: --keep <earlier.json> <vendor>", file=sys.stderr)
            return 2
        keep = (args[i + 1], args[i + 2])
        del args[i:i + 3]
    if not args:
        print("usage: matrix_json.py <matrix_results.csv> [out.json] "
              "[--keep <earlier.json> <vendor>]", file=sys.stderr)
        return 2
    out_path = args[1] if len(args) >= 2 else None
    today = datetime.date.today().isoformat()

    with open(args[0], newline="") as fh:
        rows = list(csv.DictReader(fh))
    if not rows:
        print("error: empty CSV", file=sys.stderr)
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
            cell["n"] = int(r["n"])
            cell["min_us"] = num(r["min_us"])
            cell["p50_us"] = num(r["p50_us"])
            cell["p90_us"] = num(r["p90_us"])
            cell["p99_us"] = num(r["p99_us"])
            cell["p999_us"] = num(r["p999_us"])
            cell["max_us"] = num(r["max_us"])
        else:
            fail += 1
        matrix[p][pi][po] = cell

    for a, b, reason in unmeasurable:
        for p in payloads:
            for pi, po in ((a, b), (b, a)):
                c = matrix[str(p)].get(pi, {}).get(po)
                if c is None:
                    continue
                if c.get("status") == "ok":
                    print(f"error: {pi}->{po} at {p} B was measured", file=sys.stderr)
                    return 1
                fail -= 1
                matrix[str(p)][pi][po] = {"status": "not-measured", "reason": reason}

    measured = {v: today for v in vendors}
    kept_versions = {}
    if keep:
        with open(keep[0]) as fh:
            earlier = json.load(fh)
        kv = keep[1]
        if kv not in earlier["vendors"]:
            print(f"error: {kv} is not in {keep[0]}", file=sys.stderr)
            return 1
        vendors.append(kv)
        measured[kv] = earlier["date"]
        kept_versions[kv] = earlier["vendor_versions"].get(kv, kv)
        for p in payloads:
            old_p = earlier["matrix"].get(str(p), {})
            matrix[str(p)][kv] = {}
            for po in vendors:
                c = old_p.get(kv, {}).get(po)
                if c is not None:
                    matrix[str(p)][kv][po] = dict(c, measured=earlier["date"])
            for pi in vendors:
                if pi == kv:
                    continue
                c = old_p.get(pi, {}).get(kv)
                if c is not None:
                    matrix[str(p)][pi][kv] = dict(c, measured=earlier["date"])
        # A pair of the kept vendor with one the earlier matrix did not
        # have was never measured: said so, not left empty.
        for p in payloads:
            for pi in vendors:
                for po in vendors:
                    if kv not in (pi, po):
                        continue
                    c = matrix[str(p)].setdefault(pi, {}).get(po)
                    if c is None:
                        matrix[str(p)][pi][po] = {
                            "status": "not-measured",
                            "reason": f"{kv} could not be measured on {today} "
                                      f"and its matrix of {earlier['date']} has no "
                                      f"{po if pi == kv else pi}",
                        }
                    elif c.get("status") == "ok":
                        ok += 1
                    else:
                        fail += 1

    def cells_of(pred):
        return [(p, pi, po, c) for p, by_ping in matrix.items()
                for pi, by_pong in by_ping.items()
                for po, c in by_pong.items() if pred(pi, po)]

    zd_fail = sum(1 for _, pi, po, c in cells_of(lambda a, b: "zerodds" in (a, b))
                  if c.get("status") != "ok")
    foreign_gaps = sorted({f"{pi}->{po}" for _, pi, po, c in
                           cells_of(lambda a, b: "zerodds" not in (a, b))
                           if c.get("status") not in ("ok", "not-measured")})

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
        "date": today,
        "host": "x86-host (LXC, x86_64, Linux loopback)",
        "config": {
            "samples_per_cell": 2000,
            "warmup_per_cell": 200,
            "qos": "RELIABLE, KEEP_LAST(64)",
            "data_representation": "XCDR2; XCDR1 for the pairs with OpenSplice, "
                                   "which writes and reads XCDR1 only",
            # matrix_sweep.sh runs ZeroDDS with transport.same_host_shm off
            # (ZERODDS_SAME_HOST_SHM=0), so its self pair is UDPv4 like
            # every other pair; the apps of the other stacks set UDPv4
            # alone (Fast DDS and RTI explicitly).
            "transport": "UDPv4 loopback for every pair; ZeroDDS with "
                         "transport.same_host_shm=false",
            "pong": "event-driven (Listener/Callback, no busy-poll)",
        },
        "vendors": vendors,
        "vendor_versions": {v: kept_versions.get(v, VENDOR_VERSIONS.get(v, v))
                            for v in vendors},
        "measured": measured,
        "payloads_bytes": payloads,
        "cells": {"total": ok + fail, "ok": ok, "fail": fail},
        "zerodds_fail_cells": zd_fail,
        "interop": {
            "zerodds": "full" if zd_fail == 0 else "partial",
            "note": ("ZeroDDS interoperates in both directions with all "
                     "third-party stacks; 0 failing cells."
                     if zd_fail == 0 else
                     f"{zd_fail} ZeroDDS cells without a result."),
            "gaps_between_other_stacks": foreign_gaps,
            "not_measured": sorted({f"{pi}->{po}" for _, pi, po, c in
                                    cells_of(lambda a, b: True)
                                    if c.get("status") == "not-measured"}),
        },
        "headline": headline,
        "matrix": matrix,
    }

    text = json.dumps(out, indent=2, ensure_ascii=False)
    if out_path:
        with open(out_path, "w") as fh:
            fh.write(text + "\n")
    else:
        print(text)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
