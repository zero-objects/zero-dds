#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 ZeroDDS Contributors
#
# coverage.py — one line per behaviour of `docs/wire-feature-inventory.md`,
# saying whether a capture reached it and, where it did not, why.
#
#   tools/wire-capture/coverage.py --out tools/wire-capture/coverage.tsv
#
# ## What it joins
#
#   `docs/wire-feature-inventory.md`   the behaviours themselves. The
#                                      row ids are read out of the
#                                      tables rather than restated here,
#                                      so a behaviour added there and
#                                      nowhere else fails this join
#                                      instead of disappearing from it.
#   `cases.tsv`                        which corpus cases bear on which
#                                      behaviour.
#   `wire-goldens.tsv`                 which of those cases actually
#                                      produced octets, and through
#                                      which implementations.
#   `reasons.tsv`                      the behaviours no capture of a
#                                      writer can reach, one line each.
#
# ## The four verdicts, and why `no-golden` is not a reason column
#
#   wire        at least one implementation put octets on a socket for
#               at least one case that bears on this behaviour. The
#               implementations are named, because "measured on the
#               wire" by one product and by seven are different claims.
#   no-golden   a case exists and every implementation refused it or
#               failed the run. That is itself the one-line reason —
#               *no implementation can be driven live for it* — and the
#               `detail` column carries both who and what they said, so
#               a row resting on `refused` (settled, the front end) reads
#               differently from one resting on `unread` (a run that did
#               not happen and is waiting for `--retry`).
#   no-wire     `reasons.tsv` says why, in one line and under one of
#               three kinds.
#   not-run     a case names the behaviour and no run reached that case.
#               Neither a measurement nor a reason.
#
# The exit status is the point: a behaviour that appears in neither
# `cases.tsv` nor `reasons.tsv`, in both, or in a case nothing ran, is an
# unfinished claim and this refuses to write a table that hides it.
import argparse
import os
import re
import sys

ROW_ID = re.compile(r"^\|\s*([A-Z]\d+)\s*\|")
# `wire-goldens.tsv`'s verdicts that mean octets were captured. Anything
# else in that column — `refused`, `publisher`, `unstable`,
# `nothing-captured` — is a run that produced no golden.
GOLDEN = ("same", "wire-pads", "differs", "wire-only")


def inventory_rows(path):
    rows = []
    with open(path, encoding="utf-8") as handle:
        for line in handle:
            found = ROW_ID.match(line)
            if found:
                rows.append(found.group(1))
    return rows


def tsv(path):
    with open(path, encoding="utf-8") as handle:
        for line in handle:
            line = line.rstrip("\n")
            if not line or line.startswith("#"):
                continue
            yield line.split("\t")


def case_rows(path):
    """behaviour -> the corpus cases that bear on it."""
    out = {}
    for parts in tsv(path):
        if len(parts) < 5 or parts[0] == "case":
            continue
        for row in parts[3].split():
            # `breadth` is `combined-everything`'s entry: the case is
            # there for the shape and not for one clause.
            if row == "breadth":
                continue
            out.setdefault(row, []).append(parts[0])
    return out


def goldens(path):
    """corpus case -> (implementations with octets, implementations without),
    each split by transport.

    The split is not decoration. GIOP carries plain CDR and no
    encapsulation header at all, so an ORB that produced octets for
    `struct-appendable` is evidence for the DHEADER and none at all for
    the identifier in front of it. A single list of nine names would say
    the opposite."""
    got, missed, said = {}, {}, {}
    if not os.path.exists(path):
        return got, missed, said
    for parts in tsv(path):
        if len(parts) < 11:
            continue
        case, impl, transport, verdict = parts[0], parts[1], parts[4], parts[10]
        # A refused run has no transport column filled in; it is still
        # that implementation's answer, and `compare-levels.py` writes
        # the transport for it too.
        wire = "giop" if transport.startswith("GIOP") else "rtps"
        target = got if verdict in GOLDEN else missed
        target.setdefault(case, set()).add((wire, impl))
        if verdict not in GOLDEN:
            said[(case, impl)] = verdict
    return got, missed, said


def by_transport(pairs):
    out = []
    for wire in ("rtps", "giop"):
        names = sorted(impl for kind, impl in pairs if kind == wire)
        if names:
            out.append(f"{wire}:" + ",".join(names))
    return " ".join(out)


def main():
    here = os.path.dirname(os.path.abspath(__file__))
    root = os.path.abspath(os.path.join(here, "..", ".."))
    parser = argparse.ArgumentParser()
    parser.add_argument("--root", default=root)
    parser.add_argument("--out", default=os.path.join(here, "coverage.tsv"))
    arguments = parser.parse_args()

    inventory = os.path.join(arguments.root, "docs", "wire-feature-inventory.md")
    rows = inventory_rows(inventory)
    cases = case_rows(os.path.join(here, "cases.tsv"))
    reasons = {p[0]: (p[1], p[2]) for p in tsv(os.path.join(here, "reasons.tsv"))
               if len(p) >= 3}
    got, missed, verdicts = goldens(os.path.join(arguments.root, "tests",
                                                 "interop-results",
                                                 "wire-goldens.tsv"))

    lines, unfinished, both, never_run = [], [], [], []
    counts = {"wire": 0, "no-golden": 0, "no-wire": 0}
    for row in rows:
        has_case, has_reason = row in cases, row in reasons
        if has_case and has_reason:
            both.append(row)
        if not has_case and not has_reason:
            unfinished.append(row)
            continue
        if has_case:
            named = cases[row]
            with_octets = sorted(set().union(*(got.get(c, set()) for c in named)))
            without = sorted(set().union(*(missed.get(c, set()) for c in named)))
            if with_octets:
                counts["wire"] += 1
                lines.append((row, "wire", by_transport(with_octets), " ".join(named)))
            elif not without:
                # The case is in `cases.tsv` and in no row of the table:
                # nothing ran it. That is not a measurement and not a
                # reason, and it exits non-zero for the same cause an
                # unfinished claim does.
                never_run.append(row)
                lines.append((row, "not-run", "none", " ".join(named)))
            else:
                counts["no-golden"] += 1
                # The verdicts, not only the names. `refused` is the
                # product's front end and settles the row; `unread` or
                # `publisher` is a run that did not happen, and a row
                # resting on those is waiting for `--retry` and not for a
                # vendor.
                said = sorted({verdicts[(c, impl)]
                               for kind, impl in without for c in named
                               if (c, impl) in verdicts})
                lines.append((row, "no-golden",
                              (by_transport(without) or "none") + " — " + ",".join(said),
                              "no implementation could be driven live for "
                              + " ".join(named)))
            continue
        kind, why = reasons[row]
        counts["no-wire"] += 1
        lines.append((row, "no-wire", kind, why))

    with open(arguments.out, "w", encoding="utf-8") as handle:
        handle.write(
            "# Every behaviour of `docs/wire-feature-inventory.md`, and whether\n"
            "# a capture of a real writer reached it. Produced by\n"
            "# `tools/wire-capture/coverage.py` out of that document's row ids,\n"
            "# `cases.tsv`, `reasons.tsv` and\n"
            "# `tests/interop-results/wire-goldens.tsv`.\n"
            "#\n"
            "#   wire       octets on a socket; the `detail` column is the\n"
            "#              implementations that produced them, split by\n"
            "#              transport — GIOP carries plain CDR and no\n"
            "#              encapsulation header, so an ORB in the `giop:`\n"
            "#              half is evidence for the payload and for nothing\n"
            "#              that only an RTPS `SerializedPayload` has\n"
            "#   no-golden  a case exists and every implementation refused it —\n"
            "#              which is the reason, and `detail` names who refused\n"
            "#   no-wire    `reasons.tsv`'s kind and its one line\n"
            "#   not-run    a case names it and no run reached that case —\n"
            "#              neither a measurement nor a reason, and\n"
            "#              `coverage.py` exits non-zero for it\n"
            "#\n"
            "# row\tlevel\tdetail\tcases or reason\n")
        for row in lines:
            handle.write("\t".join(row) + "\n")

    # The split first, because it is the question this file exists to
    # answer, and the kinds after it. `not-run` is counted apart from
    # both halves on purpose: a case nothing ran is neither a
    # measurement nor a reason, and adding it to either side would hide
    # the one state that exits non-zero.
    without = counts["no-golden"] + counts["no-wire"]
    print(f"coverage: {counts['wire']} of {len(rows)} behaviours carry a "
          f"wire-level measurement, {without} do not "
          f"({counts['no-golden']} no-golden, {counts['no-wire']} no-wire)"
          + (f", {len(never_run)} not-run" if never_run else "")
          + f" -> {arguments.out}", file=sys.stderr)
    status = 0
    if unfinished:
        print("coverage: no case and no reason: " + " ".join(unfinished),
              file=sys.stderr)
        status = 1
    if both:
        print("coverage: both a case and a reason: " + " ".join(both),
              file=sys.stderr)
        status = 1
    if never_run:
        print("coverage: a case that no run reached: " + " ".join(never_run),
              file=sys.stderr)
        status = 1
    return status


if __name__ == "__main__":
    sys.exit(main())
