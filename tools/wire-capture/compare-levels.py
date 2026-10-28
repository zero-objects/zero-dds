#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 ZeroDDS Contributors
#
# The two measurement levels beside each other, one row per
# (case, implementation, encoding version).
#
#   compare-levels.py --rows /root/wire-goldens-full --out wire-goldens.tsv
#
# The **wire** level is `wire-golden.sh`'s: the octets a real
# `DataWriter` or a real GIOP call put on a socket. The **in-memory**
# level is the corpus sweep already in the tree —
# `tests/interop-results/sources/{vendor,ros,corba}-corpus.tsv` — which
# is what every column of `docs/wire-feature-inventory.md` was built
# from until now.
#
# The verdict column is the point of the file:
#
#   same                 the two levels produced the same octets
#   wire-pads            the wire adds trailing zeros to the next
#                        4-octet offset and is otherwise identical —
#                        §7.4.1.1's trailing padding, added by the
#                        transport and absent from the serialiser's own
#                        output
#   differs <offsets>    the two levels part company, at these offsets
#   wire-only            nothing in-memory was ever recorded for this
#                        combination
#
# A `differs` at exactly the alignment fill of a CORBA case is not news:
# `omniorb-template.cpp` and `tao-template.cpp` put `0xAA` in their
# buffer on purpose, precisely so that the octets those ORBs do not
# write are visible as `aa`. The offsets say which case that is.
import argparse
import os
import sys

WIRE_IMPLS = ("cyclone", "rti", "fastdds", "opendds", "opensplice", "hdds",
              "ros-fastrtps", "ros-cyclone", "omniorb", "tao", "jacorb")

TRANSPORT = {
    "cyclone": "RTPS/UDP", "rti": "RTPS/UDP", "fastdds": "RTPS/UDP",
    "opendds": "RTPS/UDP", "opensplice": "RTPS/UDP", "hdds": "RTPS/UDP",
    "ros-fastrtps": "RTPS/UDP", "ros-cyclone": "RTPS/UDP",
    "omniorb": "GIOP/TCP", "tao": "GIOP/TCP", "jacorb": "GIOP/TCP",
}


def read_in_memory(root):
    """(case, impl, encoding, endian) -> hex, out of the recorded sweeps."""
    found = {}
    vendor = os.path.join(root, "tests/interop-results/sources/vendor-corpus.tsv")
    ros = os.path.join(root, "tests/interop-results/sources/ros-corpus.tsv")
    corba = os.path.join(root, "tests/interop-results/sources/corba-corpus.tsv")
    for path in (vendor, ros):
        if not os.path.exists(path):
            continue
        with open(path, encoding="utf-8") as handle:
            for line in handle:
                parts = line.rstrip("\n").split("\t")
                if len(parts) != 6 or parts[4] != "wrote":
                    continue
                case, impl, encoding, endian, _, payload = parts
                found[(case, impl, encoding, endian)] = payload
    if os.path.exists(corba):
        with open(corba, encoding="utf-8") as handle:
            for line in handle:
                parts = line.rstrip("\n").split("\t")
                if len(parts) != 5 or parts[3] != "write":
                    continue
                case, impl, endian, _, payload = parts
                # A refusal is in the payload column behind `ERR1:`, and
                # there is no status column to say so
                # (`tools/corba-oracle/corba-sweep.sh`). Keeping one
                # would put a sentence where the `in-memory` column
                # promises octets, and `verdict()` would hand it to
                # `bytes.fromhex`. Skipping it makes the row `wire-only`,
                # which is what "nothing in-memory was recorded for this
                # combination" means.
                if payload.startswith("ERR"):
                    continue
                # CORBA CDR is encoding version 1 and has no other.
                found[(case, impl, "xcdr1", endian)] = payload
    return found


def differing(left, right):
    a, b = bytes.fromhex(left), bytes.fromhex(right)
    if len(a) != len(b):
        return None
    return [i for i in range(len(a)) if a[i] != b[i]]


def spans(positions):
    out, start = [], None
    for index, position in enumerate(positions):
        if start is None:
            start = position
        if index + 1 == len(positions) or positions[index + 1] != position + 1:
            out.append(str(start) if start == position else f"{start}-{position}")
            start = None
    return ",".join(out)


def verdict(wire, memory):
    if memory is None:
        return "wire-only", ""
    if wire == memory:
        return "same", ""
    blob, base = bytes.fromhex(wire), bytes.fromhex(memory)
    pad = (-len(base)) % 4
    if pad and blob == base + b"\x00" * pad:
        return "wire-pads", f"{pad} trailing zero octets"
    positions = differing(wire, memory)
    if positions is None:
        return "differs", f"length {len(blob)} against {len(base)}"
    return "differs", spans(positions)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--rows", required=True,
                        help="the directory wire-golden.sh wrote")
    parser.add_argument("--root", default=".", help="the repository root")
    parser.add_argument("--out", required=True)
    arguments = parser.parse_args()

    memory = read_in_memory(arguments.root)
    lines = []
    for impl in WIRE_IMPLS:
        path = os.path.join(arguments.rows, f"rows-{impl}.tsv")
        if not os.path.exists(path):
            continue
        with open(path, encoding="utf-8") as handle:
            for line in handle:
                parts = line.rstrip("\n").split("\t")
                if len(parts) < 4:
                    continue
                name, case, encoding, status = parts[:4]
                if status != "ok":
                    lines.append((case, impl, encoding, "", TRANSPORT[impl],
                                  "", "", "", "", "", status.lstrip("!"),
                                  parts[4] if len(parts) > 4 else ""))
                    continue
                head, options, key_hash, payload, poison, control, drift = parts[4:11]
                if TRANSPORT[impl] == "GIOP/TCP":
                    endian = head            # the flag byte's order
                    head, options = "", ""
                else:
                    # Table 39: the identifiers are pairs, big endian
                    # even and little endian odd.
                    endian = "le" if int(head, 16) % 2 else "be"
                where = memory.get((case, impl, encoding, endian))
                said, detail = verdict(payload, where)
                lines.append((case, impl, encoding, endian, TRANSPORT[impl],
                              head, options, key_hash, payload,
                              where or "", said,
                              "; ".join(x for x in (detail, drift, control) if x)))

    with open(arguments.out, "w", encoding="utf-8") as handle:
        handle.write(
            "# The octets nine implementations put on a socket, against the\n"
            "# octets their in-memory serialisation entry point returns for\n"
            "# the same corpus case. Produced by\n"
            "# `tools/wire-capture/compare-levels.py`; the runs behind it are\n"
            "# `tools/wire-capture/wire-golden.sh` and the frames under\n"
            "# `tools/wire-capture/frames/`.\n"
            "#\n"
            "# case\timpl\tencoding\tendian\ttransport\tencapsulation\t"
            "options\tkeyhash\twire\tin-memory\tverdict\tnote\n")
        for row in sorted(lines):
            handle.write("\t".join(row) + "\n")
    print(f"{len(lines)} rows -> {arguments.out}", file=sys.stderr)


if __name__ == "__main__":
    main()
