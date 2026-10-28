#!/usr/bin/env python3
"""The tables the round-trip fixture carries, out of the pairwise reports."""
import sys, collections, re

IMPLS = ["zerodds", "cyclone", "rti", "fastdds"]


def load(paths):
    rows = []
    for path in paths:
        with open(path) as handle:
            handle.readline()
            for line in handle:
                parts = line.rstrip("\n").split("\t")
                if len(parts) < 7 or parts[1] == "-":
                    continue
                while len(parts) < 8:
                    parts.append("")
                rows.append(parts)
    return rows


def cause(row):
    """Which known divergence a failing row is an instance of."""
    detail = row[7]
    case = row[0]
    if "wide_letter" in detail or "wchar" in detail:
        return "wchar literal in the C fill emitter"
    if case.endswith("-unannotated"):
        return "default extensibility"
    if re.search(r"\bitems\[|\brows\[|\bgrid\[", detail) or case.startswith(
        ("array-", "sequence-")
    ):
        return "collection delimiter"
    if case.startswith("annotation-optional") or "optional" in detail:
        return "optional member framing"
    if case.startswith("annotation-hashid") or case.startswith("annotation-id"):
        return "member id assignment"
    if case.startswith("union-"):
        return "union framing"
    if case.startswith("map-"):
        return "map framing"
    if case.startswith("wstring") or "wstring" in detail:
        return "wstring length"
    return "unclassified"


def pair_table(rows, title):
    print(title)
    print("| writer | reader | same | different | unread | unwritten | uncomparable |")
    print("|---|---|---|---|---|---|---|")
    clean = 0
    for a in IMPLS:
        for b in IMPLS:
            cell = collections.Counter(r[6] for r in rows if r[4] == a and r[5] == b)
            if not sum(cell.values()):
                continue
            if not (cell["different"] or cell["unread"] or cell["uncomparable"]):
                clean += 1
            print(
                f"| {a} | {b} | {cell['same']} | {cell['different']} | "
                f"{cell['unread']} | {cell['unwritten']} | {cell['uncomparable']} |"
            )
    print(f"\nordered pairs that round-trip clean: {clean} of 16\n")


def main():
    paths = sys.argv[1:]
    matrix = [p for p in paths if "dialect" not in p]
    dialects = [p for p in paths if "dialect" in p]

    rows = load(matrix)
    print(f"rows {len(rows)}   cases {len({r[0] for r in rows})}\n")
    tally = collections.Counter(r[6] for r in rows)
    for verdict, count in tally.most_common():
        print(f"{verdict:14} {count}")
    print()
    pair_table(rows, "## every ordered pair")

    print("## what the differences are")
    by_cause = collections.Counter(cause(r) for r in rows if r[6] == "different")
    for name, count in by_cause.most_common():
        print(f"{name:38} {count}")
    print()

    print("## a value came back different — one line per case and pair")
    seen = {}
    for r in rows:
        if r[6] != "different":
            continue
        key = (r[0], r[4], r[5])
        seen.setdefault(key, r[7].replace("differs ", ""))
    for (case, a, b), detail in sorted(seen.items())[:400]:
        print(f"{case:46} {a:8} -> {b:8}  {detail[:90]}")
    print(f"\n{len(seen)} distinct (case, writer, reader) triples\n")

    print("## a reader refused the payload — by pair and reason")
    refused = collections.Counter()
    for r in rows:
        if r[6] != "unread":
            continue
        why = re.sub(r"/tmp/\S+", "", r[7])[:80]
        refused[(r[4], r[5], why)] += 1
    for (a, b, why), count in refused.most_common(30):
        print(f"{a:8} -> {b:8} {count:4}  {why}")
    print()

    if not dialects:
        return
    sweep = load(dialects)
    print("## the policy table — which dialect the ZeroDDS writer needs")
    print("| policy | reader | same | different | unread | uncomparable |")
    print("|---|---|---|---|---|---|")
    for policy in ["cyclone", "rti", "fastdds", "specification"]:
        for reader in IMPLS:
            cell = collections.Counter(
                r[6] for r in sweep if r[3] == policy and r[4] == "zerodds" and r[5] == reader
            )
            if not sum(cell.values()):
                continue
            print(
                f"| {policy} | {reader} | {cell['same']} | {cell['different']} | "
                f"{cell['unread']} | {cell['uncomparable']} |"
            )
    print()
    print("## the policy table — the ZeroDDS reader against each writer")
    print("| policy | writer | same | different | unread | uncomparable |")
    print("|---|---|---|---|---|---|")
    for policy in ["cyclone", "rti", "fastdds", "specification"]:
        for writer in IMPLS:
            cell = collections.Counter(
                r[6] for r in sweep if r[3] == policy and r[5] == "zerodds" and r[4] == writer
            )
            if not sum(cell.values()):
                continue
            print(
                f"| {policy} | {writer} | {cell['same']} | {cell['different']} | "
                f"{cell['unread']} | {cell['uncomparable']} |"
            )


main()
