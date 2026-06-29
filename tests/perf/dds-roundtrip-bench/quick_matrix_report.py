#!/usr/bin/env python3
"""Schneller Report fuer quick_matrix.csv: 4x4 Matrix pro payload."""

import csv, statistics, sys
from collections import defaultdict


def main():
    if len(sys.argv) < 2:
        print("Usage: quick_matrix_report.py <quick_matrix.csv>")
        sys.exit(1)
    path = sys.argv[1]

    data = defaultdict(list)
    with open(path) as f:
        for r in csv.DictReader(f):
            if r["status"] != "ok":
                continue
            try:
                p50 = float(r["p50"])
                p99 = float(r["p99"])
                if p50 > 0:
                    data[(r["ping"], r["pong"], int(r["payload"]))].append((p50, p99))
            except (ValueError, KeyError):
                pass

    vendors = sorted({k[0] for k in data} | {k[1] for k in data})
    payloads = sorted({k[2] for k in data})

    for payload in payloads:
        print(f"\n=== Payload {payload} B — median p50 µs (N=runs) ===")
        header = f"{'ping ↓ / pong →':<14} | " + " | ".join(f"{v:>10}" for v in vendors)
        print(header)
        print("-" * len(header))
        for ping in vendors:
            row = f"{ping:<14} | "
            for pong in vendors:
                cells = data.get((ping, pong, payload), [])
                if cells:
                    med = statistics.median(c[0] for c in cells)
                    row += f"{med:>10.2f} | "
                else:
                    row += f"{'FAIL':>10} | "
            print(row)

    # Self-Tabelle ueber Payloads
    print("\n=== Self-Latenz (p50 median, p99 median) ueber Payloads ===")
    print(f"{'vendor':<10} | " + " | ".join(f"{p:>4}B p50/p99" for p in payloads))
    for v in vendors:
        row = f"{v:<10} | "
        for p in payloads:
            cells = data.get((v, v, p), [])
            if cells:
                m_p50 = statistics.median(c[0] for c in cells)
                m_p99 = statistics.median(c[1] for c in cells)
                row += f"{m_p50:>5.1f}/{m_p99:>5.1f} | "
            else:
                row += f"{'FAIL':>11} | "
        print(row)


if __name__ == "__main__":
    main()
