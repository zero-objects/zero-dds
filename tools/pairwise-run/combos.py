#!/usr/bin/env python3
"""The per-combination verdict table the fixture's last section carries."""
import sys, collections

print("| encoding | byte order | same | different | unread | unwritten | uncomparable |")
print("|---|---|---|---|---|---|---|")
for path in sys.argv[1:]:
    stem = path.split("report-")[-1].removesuffix(".tsv")
    encoding, _, endian = stem.partition("-")
    cell = collections.Counter()
    with open(path) as handle:
        handle.readline()
        for line in handle:
            parts = line.rstrip("\n").split("\t")
            if len(parts) >= 7 and parts[1] != "-":
                cell[parts[6]] += 1
    print(
        f"| {encoding} | {endian} | {cell['same']} | {cell['different']} | "
        f"{cell['unread']} | {cell['unwritten']} | {cell['uncomparable']} |"
    )
