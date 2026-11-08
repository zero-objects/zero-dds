#!/usr/bin/env python3
"""Per case, one vendor version against another, from the suites' JUnit reports.

When a vendor pin moves (internal/release/release-playbook.md, "Vendor
pins"), the result of every case with the new version is recorded against
the version before:

    compare_runs.py --old <reports of the old pin>... --new <reports of the new pin>...

A report is the harness's JUnit file of one worker, as `omg_suite.py`
writes it (`<suite>-<publisher>--<subscriber>-<chunk>-w<n>.xml`, in the
`omg-out/` artifact of a suite job). The direction is read from its name:
`zerodds->vendor` or `vendor->zerodds`. Per suite and direction, a case is
`same` (the same verdict and, for a failure, the same codes), `passes now`,
`fails now`, `other codes`, or run with one version only (a case that
version fails against itself is not run, `vendor-self-failures.tsv`).
Prints the cases that are not `same` and a count per class.
"""
import argparse
import collections
import re
from pathlib import Path

from omg_suite import is_ours, read_report
PRODUCED = re.compile(r"<th>\s*((?:Publisher|Subscriber)_\d+)\s*</th>\s*<th>[^<]*</th>"
                      r"\s*<th>\s*([A-Z_]+)\s*</th>")
NAME = re.compile(r"^(?P<suite>dds-[a-z]+)-(?P<pub>.+?)--(?P<sub>.+?)-(?P<chunk>[a-z0-9\-]+?)-w\d+\.xml$")


def verdicts(paths):
    """(suite, direction, case) -> (verdict, codes)."""
    out = {}
    for path in paths:
        m = NAME.match(path.name)
        if not m:
            raise SystemExit(f"compare_runs.py: {path.name} is not a report of omg_suite.py")
        direction = "zerodds->vendor" if is_ours(m.group("pub")) else "vendor->zerodds"
        for case in read_report(path).iter("testcase"):
            node = case.find("failure")
            if node is None:
                node = case.find("error")
            name = re.sub(r"^(xtypes_v2_[a-z_]+?_test_suite|rtps_test_suite_\d+)_", "",
                          case.get("name") or "")
            if node is None:
                out[(m.group("suite"), direction, name)] = ("pass", "")
            else:
                text = (node.get("message") or "") + " " + (node.text or "")
                codes = " ".join(f"{w}={c}" for w, c in PRODUCED.findall(text))
                out[(m.group("suite"), direction, name)] = ("fail", codes)
    return out


def main():
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("--old", type=Path, nargs="+", required=True)
    ap.add_argument("--new", type=Path, nargs="+", required=True)
    a = ap.parse_args()
    old, new = verdicts(a.old), verdicts(a.new)
    count = collections.Counter()
    for key in sorted(set(old) | set(new)):
        o, n = old.get(key), new.get(key)
        if o and n:
            if o == n:
                cls = "same"
            elif o[0] == n[0]:
                cls = "other codes"
            else:
                cls = "passes now" if n[0] == "pass" else "fails now"
        else:
            cls = "new version only" if n else "old version only"
        count[(key[0], key[1], cls)] += 1
        if cls != "same":
            print("\t".join([*key, cls, (o or ("", ""))[1], (n or ("", ""))[1]]))
    for (suite, direction, cls), n in sorted(count.items()):
        print(f"# {suite} {direction} {cls}: {n}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
