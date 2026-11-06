#!/usr/bin/env python3
"""Cut a suite's cases into chunks that fit one job, from measured durations.

    cut_chunks.py --suite dds-rtps --harness <suite checkout> \
        --budget 200 --workers 4 --prefix rtps [--cases "<case> ..."] <JUnit report>...

A case's duration is the longest the reports record for it, over every pair
they hold (a case that fails waits for its timeout and is the long one). The
cases are packed first fit, longest first, into workers of at most `--budget`
seconds; `--workers` of them, the fullest together, make one chunk, whose
workers `omg_suite.py` runs at the same time (` | ` between them in the row).
Prints `chunks.tsv` rows; a case no report holds is an error, so a chunk is
never cut from a guess.
"""
import argparse
import sys
import xml.etree.ElementTree as ET
from pathlib import Path

from omg_suite import suite_names


def durations(reports, names):
    longest = {}
    for report in reports:
        for case in ET.parse(report).getroot().iter("testcase"):
            name = case.get("name") or ""
            name = names.get(name, name)
            seconds = float(case.get("time") or 0)
            longest[name] = max(longest.get(name, 0.0), seconds)
    return longest


def main():
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    parser.add_argument("--suite", required=True)
    parser.add_argument("--harness", type=Path,
                        help="the suite checkout; its test_suite.py gives the cases, in order "
                             "(dds-security: the cells of tools/omg-security-main/"
                             "compatibility_matrix.tsv instead)")
    parser.add_argument("--budget", type=float, default=200.0,
                        help="seconds of cases per worker")
    parser.add_argument("--workers", type=int, default=1,
                        help="workers per chunk, run at the same time")
    parser.add_argument("--prefix", required=True)
    parser.add_argument("--cases", default="",
                        help="only these cases (space-separated), in the suite's order: the "
                             "selection a group runs (chunks.tsv says which and why)")
    parser.add_argument("reports", type=Path, nargs="+")
    args = parser.parse_args()

    if args.suite == "dds-security":
        matrix = Path(__file__).resolve().parents[3] / "tools/omg-security-main/compatibility_matrix.tsv"
        cases = [f"{t}__R-{r}__W-{w}" for t, r, w, _ in
                 (line.split("\t") for line in matrix.read_text().splitlines()
                  if line and not line.startswith("#"))]
        names = {case: case for case in cases}
    else:
        names = suite_names(args.harness)
        cases = list(names.values())
    if args.cases:
        wanted = args.cases.split()
        unknown = [case for case in wanted if case not in cases]
        if unknown:
            raise SystemExit(f"cut_chunks.py: the suite has no case {' '.join(unknown)}")
        cases = [case for case in cases if case in wanted]
    longest = durations(args.reports, names)
    missing = [case for case in cases if case not in longest]
    if missing:
        raise SystemExit(f"cut_chunks.py: no report holds {' '.join(missing)}")
    # First fit, longest case first: fewer chunks than packing in suite
    # order, where one long case leaves the rest of a chunk empty. Inside a
    # chunk the cases keep the suite's order.
    bins = []
    for case in sorted(cases, key=lambda c: -longest[c]):
        for chunk in bins:
            if chunk[1] + longest[case] <= args.budget:
                chunk[0].append(case)
                chunk[1] += longest[case]
                break
        else:
            bins.append([[case], longest[case]])
    position = {case: index for index, case in enumerate(cases)}
    # The fullest workers together: a chunk lasts as long as its longest worker.
    bins.sort(key=lambda worker: -worker[1])
    chunks = [bins[i:i + args.workers] for i in range(0, len(bins), args.workers)]
    chunks.sort(key=lambda chunk: min(position[c] for worker in chunk for c in worker[0]))
    for number, chunk in enumerate(chunks, 1):
        workers = sorted((sorted(members, key=position.get) for members, _ in chunk),
                         key=lambda members: position[members[0]])
        longest = max(total for _, total in chunk)
        print(f"# {args.prefix}-{number}: {len(chunk)} workers, "
              f"{sum(len(w) for w in workers)} cases, {longest:.0f} s at the longest")
        print(f"{args.suite}\t{args.prefix}-{number}\t"
              + " | ".join(" ".join(members) for members in workers))
    print(f"# {len(chunks)} chunks of {args.workers} workers, "
          f"{sum(max(t for _, t in chunk) for chunk in chunks):.0f} s at the longest in all",
          file=sys.stderr)


if __name__ == "__main__":
    main()
