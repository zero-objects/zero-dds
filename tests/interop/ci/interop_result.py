#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 ZeroDDS Contributors
"""Result model + classifier for the cross-vendor interop CI gate (issue #28).

A *cell* is one directed interop attempt: a vendor and a direction
(vendor -> ZeroDDS, or ZeroDDS -> vendor) exchanging typed `Robot` samples
on a unique domain. The classifier turns the observed reader-side counts
into exactly one status so the CI gate can distinguish a real product
failure from an expected-negative case, a timeout, or a setup problem.

The classifier is intentionally pure and side-effect free so it can be unit
tested against small committed fixtures without any DDS stack present.

Statuses
--------
PASS               correctness assertion for the cell was met.
EXPECTED_NEGATIVE  a negative case produced the expected VISIBLE decode
                   error (match + zero decoded samples + decode errors > 0),
                   not a silent timeout. Counts as a green cell.
PRODUCT_FAIL       the cell ran to completion but the assertion failed
                   (e.g. matched but zero samples in a positive case).
TIMEOUT            the cell exceeded its bounded wall-clock budget without
                   reaching a terminal result.
SETUP_FAIL         the environment was not usable (missing vendor binary,
                   vendor process died at startup, build failure).
"""
from __future__ import annotations

import json
import re
import sys
from dataclasses import asdict, dataclass, field
from typing import Optional

PASS = "PASS"
EXPECTED_NEGATIVE = "EXPECTED_NEGATIVE"
PRODUCT_FAIL = "PRODUCT_FAIL"
TIMEOUT = "TIMEOUT"
SETUP_FAIL = "SETUP_FAIL"

# A cell is green (does not fail the gate) iff its status is one of these.
GREEN_STATUSES = frozenset({PASS, EXPECTED_NEGATIVE})

# Expectation kinds a cell can declare.
EXPECT_SAMPLES = "samples"  # positive: expect matched + decoded samples, no errors
EXPECT_NEGATIVE = "negative"  # expect a visible decode error, zero samples


@dataclass
class Observed:
    """Reader-side observation of one cell.

    `matched` and `errors` are `None` when the reader side is a vendor that
    does not report those counts (e.g. the reverse direction, where only the
    delivered sample count is available). `samples` is always required for a
    terminal (non-setup, non-timeout) verdict.
    """

    samples: Optional[int] = None
    matched: Optional[int] = None
    errors: Optional[int] = None
    discovered: Optional[int] = None
    timed_out: bool = False
    setup_ok: bool = True


def classify(expect: str, obs: Observed) -> str:
    """Return exactly one status for a cell. Pure function."""
    if expect not in (EXPECT_SAMPLES, EXPECT_NEGATIVE):
        raise ValueError(f"unknown expectation {expect!r}")
    # Setup problems dominate: nothing meaningful ran.
    if not obs.setup_ok:
        return SETUP_FAIL
    # A timeout is only a timeout if we did not already have a terminal signal.
    # For a positive case, an explicit match with zero samples inside the
    # window is a product failure, not a timeout — surface it as such.
    if expect == EXPECT_NEGATIVE:
        # Expected visible framing/decode error: matched, nothing decoded,
        # and decode errors actually surfaced (not a silent timeout).
        if obs.matched == 1 and (obs.samples or 0) == 0 and (obs.errors or 0) > 0:
            return EXPECTED_NEGATIVE
        # No match at all within the budget → timeout, not a product bug.
        if obs.timed_out and not obs.matched:
            return TIMEOUT
        return PRODUCT_FAIL
    # Positive case: require decoded samples, no decode errors, and — where the
    # reader reports it — an endpoint match. SPDP discovery alone is not a pass.
    matched_ok = obs.matched is None or obs.matched >= 1
    errors_ok = obs.errors is None or obs.errors == 0
    if matched_ok and (obs.samples or 0) > 0 and errors_ok:
        return PASS
    # Distinguish "never matched, ran out of time" (timeout) from "matched but
    # broken" (product failure).
    if obs.timed_out and not matched_ok and (obs.samples or 0) == 0:
        return TIMEOUT
    return PRODUCT_FAIL


@dataclass
class CellResult:
    vendor: str
    direction: str  # "vendor_to_zerodds" | "zerodds_to_vendor"
    case: str  # human-readable case id
    expect: str
    status: str
    observed: Observed
    detail: str = ""


@dataclass
class RunResult:
    vendor: str
    base_sha: str = ""
    versions: dict = field(default_factory=dict)
    cells: list = field(default_factory=list)

    @property
    def ok(self) -> bool:
        return bool(self.cells) and all(
            c["status"] in GREEN_STATUSES for c in self.cells
        )


def _field(line: str, name: str) -> Optional[int]:
    """Return the integer value of ``name=<int>`` in *line*, or None."""
    m = re.search(rf"\b{name}=(\d+)", line)
    return int(m.group(1)) if m else None


def parse_result_line(line: str) -> Observed:
    """Extract counts from a ZeroDDS/vendor reader RESULT line.

    Recognises the ZeroDDS reader form
    ``RESULT discovered=.. matched=.. samples=.. errors=..`` and the vendor
    reader form ``*_RESULT samples=..``. Each field is matched independently,
    so field order and absent fields are both handled. Missing fields stay
    `None`.
    """
    return Observed(
        samples=_field(line, "samples"),
        matched=_field(line, "matched"),
        errors=_field(line, "errors"),
        discovered=_field(line, "discovered"),
    )


def _cell_to_jsonable(c: CellResult) -> dict:
    d = asdict(c)
    d["observed"] = asdict(c.observed)
    d["green"] = c.status in GREEN_STATUSES
    return d


def main(argv: list) -> int:
    """CLI: classify one cell and print its JSON object.

    Usage:
      interop_result.py --vendor V --direction D --case C --expect E \
        [--result-line "RESULT ..."] [--samples N] [--matched N] \
        [--errors N] [--timed-out] [--setup-failed] [--detail TEXT]

    Prints the cell JSON to stdout and exits 0 if the cell is green, 1 if not.
    """
    import argparse

    ap = argparse.ArgumentParser()
    ap.add_argument("--vendor", required=True)
    ap.add_argument("--direction", required=True)
    ap.add_argument("--case", required=True)
    ap.add_argument("--expect", required=True, choices=[EXPECT_SAMPLES, EXPECT_NEGATIVE])
    ap.add_argument("--result-line", default="")
    ap.add_argument("--samples", type=int)
    ap.add_argument("--matched", type=int)
    ap.add_argument("--errors", type=int)
    ap.add_argument("--discovered", type=int)
    ap.add_argument("--timed-out", action="store_true")
    ap.add_argument("--setup-failed", action="store_true")
    ap.add_argument("--detail", default="")
    a = ap.parse_args(argv)

    if a.result_line:
        obs = parse_result_line(a.result_line)
    else:
        obs = Observed()
    # Explicit flags/counts override parsed values.
    for name in ("samples", "matched", "errors", "discovered"):
        v = getattr(a, name)
        if v is not None:
            setattr(obs, name, v)
    obs.timed_out = a.timed_out
    obs.setup_ok = not a.setup_failed

    status = classify(a.expect, obs)
    cell = CellResult(
        vendor=a.vendor,
        direction=a.direction,
        case=a.case,
        expect=a.expect,
        status=status,
        observed=obs,
        detail=a.detail,
    )
    print(json.dumps(_cell_to_jsonable(cell)))
    return 0 if status in GREEN_STATUSES else 1


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
