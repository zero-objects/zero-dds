#!/usr/bin/env python3
"""Spread-Analyse fuer matrix_runs_raw.csv (N>=2 runs pro Zelle).

Pro (ping, pong, payload) berechnet:
- N runs, alle p50-Werte
- spread = (max - min) / median * 100   (% Range relativ zur Mitte)
- cv = stddev / mean * 100              (% Variationskoeffizient)

Ausgabe-Ranking nach spread, plus Aggregate pro Vendor-Combo.

Aufruf: matrix_spread.py <matrix_runs_raw.csv> [--top N] [--vendor=zerodds]
"""

import csv
import statistics
import sys
from collections import defaultdict
from pathlib import Path


def read_runs(csv_path):
    """Liest matrix_runs_raw.csv → {(ping, pong, payload): [p50_us, ...]}."""
    by_cell = defaultdict(list)
    with open(csv_path, newline="") as f:
        reader = csv.DictReader(f)
        for row in reader:
            if row["status"] != "ok":
                continue
            try:
                p50 = float(row["p50_us"])
            except (ValueError, KeyError):
                continue
            key = (row["ping_vendor"], row["pong_vendor"], int(row["payload_bytes"]))
            by_cell[key].append(p50)
    return by_cell


def cell_stats(p50s):
    """Returns (n, median, min, max, spread_pct, cv_pct)."""
    n = len(p50s)
    med = statistics.median(p50s)
    mn = min(p50s)
    mx = max(p50s)
    spread_pct = (mx - mn) / med * 100 if med > 0 else 0
    if n >= 2:
        mean = statistics.mean(p50s)
        sd = statistics.stdev(p50s)
        cv_pct = sd / mean * 100 if mean > 0 else 0
    else:
        cv_pct = 0
    return n, med, mn, mx, spread_pct, cv_pct


def main(argv):
    if len(argv) < 2:
        print(__doc__)
        sys.exit(1)
    csv_path = Path(argv[1])
    top = 10
    vendor_filter = None
    for arg in argv[2:]:
        if arg.startswith("--top="):
            top = int(arg.split("=", 1)[1])
        elif arg.startswith("--vendor="):
            vendor_filter = arg.split("=", 1)[1]

    by_cell = read_runs(csv_path)
    if not by_cell:
        print("no data")
        sys.exit(1)

    # Filter cells with N>=2 (sonst kein spread)
    rows = []
    for (ping, pong, payload), p50s in by_cell.items():
        if vendor_filter and ping != vendor_filter and pong != vendor_filter:
            continue
        if len(p50s) < 2:
            continue
        n, med, mn, mx, spread, cv = cell_stats(p50s)
        rows.append((ping, pong, payload, n, med, mn, mx, spread, cv))

    if not rows:
        print("no cells with N>=2")
        sys.exit(1)

    # Aggregate-Stats ueber alle qualifizierten Cells
    all_spreads = [r[7] for r in rows]
    all_cvs = [r[8] for r in rows]
    print(f"=== Aggregate ueber {len(rows)} Cells (N>=2 runs) ===")
    print(f"Spread (max-min)/median in %:")
    print(f"  median: {statistics.median(all_spreads):5.2f}")
    print(f"  mean:   {statistics.mean(all_spreads):5.2f}")
    print(f"  p90:    {sorted(all_spreads)[int(len(all_spreads)*0.9)]:5.2f}")
    print(f"  max:    {max(all_spreads):5.2f}")
    print(f"CV (stddev/mean) in %:")
    print(f"  median: {statistics.median(all_cvs):5.2f}")
    print(f"  mean:   {statistics.mean(all_cvs):5.2f}")
    print(f"  p90:    {sorted(all_cvs)[int(len(all_cvs)*0.9)]:5.2f}")
    print(f"  max:    {max(all_cvs):5.2f}")
    print()

    # Per-Vendor (als ping)
    print("=== Spread pro Vendor (als ping) ===")
    by_ping = defaultdict(list)
    for r in rows:
        by_ping[r[0]].append(r[7])
    print(f"{'vendor':<10} | {'cells':>5} | {'median':>7} | {'mean':>7} | {'p90':>7} | {'max':>7}")
    print("-" * 55)
    for v in sorted(by_ping):
        sp = by_ping[v]
        p90 = sorted(sp)[int(len(sp) * 0.9)] if len(sp) >= 10 else max(sp)
        print(f"{v:<10} | {len(sp):>5} | {statistics.median(sp):>7.2f} | {statistics.mean(sp):>7.2f} | {p90:>7.2f} | {max(sp):>7.2f}")
    print()

    # Per-Vendor (als pong)
    print("=== Spread pro Vendor (als pong) ===")
    by_pong = defaultdict(list)
    for r in rows:
        by_pong[r[1]].append(r[7])
    print(f"{'vendor':<10} | {'cells':>5} | {'median':>7} | {'mean':>7} | {'p90':>7} | {'max':>7}")
    print("-" * 55)
    for v in sorted(by_pong):
        sp = by_pong[v]
        p90 = sorted(sp)[int(len(sp) * 0.9)] if len(sp) >= 10 else max(sp)
        print(f"{v:<10} | {len(sp):>5} | {statistics.median(sp):>7.2f} | {statistics.mean(sp):>7.2f} | {p90:>7.2f} | {max(sp):>7.2f}")
    print()

    # Worst Cells (groesster Spread)
    print(f"=== Top {top} Worst-Spread-Cells ===")
    rows_by_spread = sorted(rows, key=lambda r: -r[7])
    print(f"{'ping':<8} → {'pong':<8} | {'payload':>5} | {'N':>2} | {'med':>6} | {'min':>6} | {'max':>6} | {'spr %':>5} | {'cv %':>5}")
    print("-" * 80)
    for ping, pong, payload, n, med, mn, mx, sp, cv in rows_by_spread[:top]:
        print(f"{ping:<8} → {pong:<8} | {payload:>5} | {n:>2} | {med:>6.2f} | {mn:>6.2f} | {mx:>6.2f} | {sp:>5.2f} | {cv:>5.2f}")
    print()

    # Best Cells (kleinster Spread)
    print(f"=== Top {top} Best-Spread-Cells ===")
    print(f"{'ping':<8} → {'pong':<8} | {'payload':>5} | {'N':>2} | {'med':>6} | {'min':>6} | {'max':>6} | {'spr %':>5} | {'cv %':>5}")
    print("-" * 80)
    for ping, pong, payload, n, med, mn, mx, sp, cv in rows_by_spread[-top:][::-1]:
        print(f"{ping:<8} → {pong:<8} | {payload:>5} | {n:>2} | {med:>6.2f} | {mn:>6.2f} | {mx:>6.2f} | {sp:>5.2f} | {cv:>5.2f}")


if __name__ == "__main__":
    main(sys.argv)
