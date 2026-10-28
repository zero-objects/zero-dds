#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 ZeroDDS Contributors
"""What moved between two ledgers, by kind rather than by line.

    tools/interop-results/ledger-diff.py OLD.tsv NEW.tsv

`verify-record.sh` answers "did anything change" with an exit code. This
answers "what changed", which is the question a vendor upgrade actually
raises: a diff of 45120 lines says nothing, and the six kinds below say
everything.

## The kinds, and why they are not one number

    gained      refused -> bytes    the system can now carry the case
    lost        bytes -> refused    it no longer can. A regression unless
                                    the toolchain deliberately dropped it
    moved       bytes -> bytes      the octets changed. The one that
                                    matters most and reads the least
                                    alarming
    appeared    absent -> present   a new case, or a column gaining rows
    vanished    present -> absent   a case removed, or a column losing rows
    read-only   the read verdict moved and the write did not

A vendor upgrade that produces only `gained` is an improvement. One that
produces `moved` changed the wire for cases that already worked, and
every `moved` row needs a reason before the new ledger is committed —
that is what this tool is for.

Exit codes: 0 when nothing moved, 1 when something did. Neither is a
verdict on whether the change is right.
"""

import sys
from collections import defaultdict

KEY = 4  # case, encoding, endian, system -> the first four columns


def read(path):
    rows = {}
    with open(path, encoding="utf-8") as handle:
        for number, line in enumerate(handle):
            if number == 0 and line.startswith("case\t"):
                continue
            if line.startswith("#") or not line.strip():
                continue
            parts = line.rstrip("\n").split("\t")
            if len(parts) < 6:
                continue
            rows[tuple(parts[:KEY])] = parts[KEY:]
    return rows


def main(argv):
    if len(argv) != 3:
        print(__doc__, file=sys.stderr)
        return 2
    old, new = read(argv[1]), read(argv[2])

    kinds = defaultdict(list)
    for key in sorted(set(old) | set(new)):
        before, after = old.get(key), new.get(key)
        name = "/".join(key)
        if before is None:
            kinds["appeared"].append((name, "", after[0] if after else ""))
            continue
        if after is None:
            kinds["vanished"].append((name, before[0], ""))
            continue
        wb, db = before[0], (before[1] if len(before) > 1 else "")
        wa, da = after[0], (after[1] if len(after) > 1 else "")
        if wb == wa and db == da:
            # The write is identical; the read verdict may still have moved.
            rb = before[2] if len(before) > 2 else ""
            ra = after[2] if len(after) > 2 else ""
            if rb != ra:
                kinds["read-only"].append((name, rb, ra))
            continue
        if wb == "refused" and wa == "bytes":
            kinds["gained"].append((name, "refused", da[:24]))
        elif wb == "bytes" and wa == "refused":
            kinds["lost"].append((name, db[:24], da[:60]))
        elif wb == "bytes" and wa == "bytes":
            kinds["moved"].append((name, db, da))
        else:
            kinds["moved"].append((name, f"{wb} {db}"[:40], f"{wa} {da}"[:40]))

    order = ["moved", "lost", "gained", "vanished", "appeared", "read-only"]
    total = sum(len(kinds[k]) for k in order)
    if total == 0:
        print("nothing moved: the two ledgers carry the same answer in every cell")
        return 0

    print(f"# {total} cell(s) moved between\n#   {argv[1]}\n#   {argv[2]}\n")
    for kind in order:
        rows = kinds[kind]
        if not rows:
            continue
        print(f"## {kind} — {len(rows)}")
        if kind == "moved":
            print("\nThese already worked and now write different octets. Each one\n"
                  "needs a reason before the new ledger is committed.\n")
        # Group by system, which is how a vendor upgrade actually lands.
        by_system = defaultdict(list)
        for name, before, after in rows:
            by_system[name.split("/")[-1]].append((name, before, after))
        for system in sorted(by_system):
            entries = by_system[system]
            print(f"  {system}: {len(entries)}")
            for name, before, after in entries[:8]:
                case = "/".join(name.split("/")[:3])
                print(f"    {case}\n      was {before or '(absent)'}\n      is  {after or '(absent)'}")
            if len(entries) > 8:
                print(f"    … and {len(entries) - 8} more")
        print()
    return 1


if __name__ == "__main__":
    sys.exit(main(sys.argv))
