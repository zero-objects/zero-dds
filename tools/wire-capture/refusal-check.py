#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 ZeroDDS Contributors
#
# refusal-check.py — a wire refusal the in-memory column does not carry.
#
#   tools/wire-capture/refusal-check.py
#
# The wire run refuses a lot: 107 cases through nine implementations is
# every front end meeting every construct, and most of what comes back
# is `!refused`. The claim the table rests on is that **every one of
# those is a refusal the in-memory column already carries** — Cyclone's
# `idlc` on a bitset, `wireprobe-corba-idl` on `@mutable`,
# `wireprobe-ros-package` on a map. A refusal the in-memory column does
# *not* carry is a different thing entirely: it is this harness refusing
# a case the product takes, and it would sit in the table looking like a
# product limit.
#
# So the check is one-directional: it prints the rows where the
# in-memory column **wrote** octets and the wire produced none. A case
# the in-memory column refuses and the wire captures is not an error at
# all, and is not looked for here.
#
# Those rows then split in two, and the split is where the value is:
#
#   `refused` — the case never reached a socket because a **build**
#       failed. The front end had already accepted the type on the other
#       level, so what failed is this harness: a template missing an
#       include, an IDL compiler invoked at the wrong language version.
#       Six RTI rows and four TAO rows of the 2026-08-19 sweep were
#       exactly that and each read, in the table, as the product
#       refusing its own type. **Exit status 1.**
#
#   `publisher`, `unread`, `no-server`, `nothing-captured` — the build
#       succeeded and the **run** did not produce a golden. When that is
#       `create_datawriter` returning null for a representation the same
#       product's serialiser writes, it is a finding and the inventory
#       carries it: OpenDDS on `@mutable` under XCDR1, Cyclone on
#       `combined-everything` under XCDR1. Printed, not failed.
#
# Exit status 1 if any **build** refusal has an in-memory counterpart.
import argparse
import os
import sys

# `compare-levels.py`'s verdicts that are not octets.
NOT_OCTETS = ("refused", "publisher", "no-server", "nothing-captured", "unread")


def tsv(path):
    if not os.path.exists(path):
        return
    with open(path, encoding="utf-8") as handle:
        for line in handle:
            line = line.rstrip("\n")
            if not line or line.startswith("#"):
                continue
            yield line.split("\t")


def in_memory(root):
    """(case, impl, encoding) -> did the in-memory probe write octets.

    Little-endian only: that is the order every wire run is on this
    host, and a big-endian in-memory row is an answer to a different
    question."""
    wrote = set()
    for name in ("vendor-corpus.tsv", "ros-corpus.tsv"):
        for parts in tsv(os.path.join(root, "tests/interop-results/sources", name)):
            if len(parts) == 6 and parts[3] == "le" and parts[4] == "wrote":
                wrote.add((parts[0], parts[1], parts[2]))
    # `corba-corpus.tsv` has no status column: five fields, and a
    # **refusal lives in the payload column behind an `ERR1:` prefix**
    # (`tools/corba-oracle/corba-sweep.sh`). Reading `write` as "octets
    # were produced" therefore counts every ORB refusal as a write, and
    # this check then reports the wire run refusing what the ORB
    # accepts — for twelve rows of the 2026-08-20 sweep it did exactly
    # that: `fixed-basic`/`fixed-scales` through TAO,
    # `primitive-wchar-above-bmp` through all three,
    # `union-discriminator-{octet,wchar,longlong}` through the ORBs
    # whose IDL compiler names the discriminator illegal. Every one of
    # those carries `ERR1:` on the in-memory side too, which is the
    # opposite of what the row said. `wstring_positions.rs` and
    # `fixed_digit_count.rs` already read the file this way.
    for parts in tsv(os.path.join(root, "tests/interop-results/sources/corba-corpus.tsv")):
        if len(parts) == 5 and parts[3] == "write" and not parts[4].startswith("ERR"):
            wrote.add((parts[0], parts[1], "xcdr1"))
    return wrote


def main():
    here = os.path.dirname(os.path.abspath(__file__))
    parser = argparse.ArgumentParser()
    parser.add_argument("--root", default=os.path.abspath(os.path.join(here, "..", "..")))
    arguments = parser.parse_args()

    wrote = in_memory(arguments.root)
    table = os.path.join(arguments.root, "tests/interop-results/wire-goldens.tsv")
    total, build, run = 0, [], []
    for parts in tsv(table):
        if len(parts) < 11 or parts[10] not in NOT_OCTETS:
            continue
        case, impl, encoding = parts[0], parts[1], parts[2]
        total += 1
        if (case, impl, encoding) not in wrote:
            continue
        row = (case, impl, encoding, parts[10],
               (parts[11] if len(parts) > 11 else "")[:160])
        (build if parts[10] == "refused" else run).append(row)

    print(f"refusal-check: {total} rows produced no octets; "
          f"{len(build)} failed to build and {len(run)} failed to run "
          f"for a case the same implementation's in-memory entry point wrote",
          file=sys.stderr)
    for row in build:
        print("build\t" + "\t".join(row))
    for row in run:
        print("run\t" + "\t".join(row))
    return 1 if build else 0


if __name__ == "__main__":
    sys.exit(main())
