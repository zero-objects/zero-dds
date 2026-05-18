#!/usr/bin/env python3
"""Compare two criterion-bench-runs and detect statistically-significant regressions.

Usage:
    python3 tests/perf/check_bench_regressions.py \
        --baseline-dir target/criterion-baseline \
        --current-dir target/criterion \
        --threshold 0.10

Exit codes:
    0 — no regression > threshold (or all changes within noise)
    1 — at least one benchmark regressed > threshold AND confidence intervals
        don't overlap (statistically significant slowdown)
    2 — usage / setup error (missing files etc.)

The "confidence intervals don't overlap" gate prevents flapping: criterion
publishes 95% CIs for the mean, and we only fail if the upper-bound of the
baseline is below the lower-bound of the current run. Pure point-estimate
threshold checks would fire on noise alone.

Each criterion benchmark dir layout (after `cargo bench -- --baseline pre`):
    target/criterion/<bench-id>/pre/estimates.json   (baseline)
    target/criterion/<bench-id>/new/estimates.json   (current)

When run with two separate dirs (e.g. baseline downloaded as artifact),
we expect:
    <baseline-dir>/<bench-id>/new/estimates.json     (was main's "new")
    <current-dir>/<bench-id>/new/estimates.json      (this run's "new")
"""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path


def find_estimates(root: Path) -> dict[str, Path]:
    """Find all estimates.json files under root, keyed by bench-id.

    Bench-id = directory path relative to root, minus the trailing
    "/new" or "/base" or "/pre" segment.
    """
    out: dict[str, Path] = {}
    for est in root.rglob("estimates.json"):
        # Skip "change" subdir — those are diff-data from --baseline runs,
        # not absolute estimates.
        rel_parts = est.relative_to(root).parts
        if "change" in rel_parts:
            continue
        # Prefer "new" over "base"; ignore named baselines like "pre".
        if rel_parts[-2] != "new":
            continue
        bench_id = "/".join(rel_parts[:-2])
        out[bench_id] = est
    return out


def load_mean(est_path: Path) -> tuple[float, float, float]:
    """Return (point, ci_lo, ci_hi) for the mean estimator (in ns)."""
    with est_path.open() as f:
        data = json.load(f)
    mean = data["mean"]
    return (
        mean["point_estimate"],
        mean["confidence_interval"]["lower_bound"],
        mean["confidence_interval"]["upper_bound"],
    )


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--baseline-dir", required=True, type=Path)
    parser.add_argument("--current-dir", required=True, type=Path)
    parser.add_argument(
        "--threshold",
        type=float,
        default=0.10,
        help="Regression threshold (fraction). 0.10 = 10%% slower.",
    )
    parser.add_argument(
        "--report",
        type=Path,
        default=None,
        help="Optional Markdown-report-output (always written, even on pass).",
    )
    args = parser.parse_args()

    if not args.baseline_dir.is_dir():
        print(f"baseline-dir does not exist: {args.baseline_dir}", file=sys.stderr)
        return 2
    if not args.current_dir.is_dir():
        print(f"current-dir does not exist: {args.current_dir}", file=sys.stderr)
        return 2

    baseline = find_estimates(args.baseline_dir)
    current = find_estimates(args.current_dir)

    if not baseline:
        print(f"no estimates.json files found under {args.baseline_dir}", file=sys.stderr)
        return 2
    if not current:
        print(f"no estimates.json files found under {args.current_dir}", file=sys.stderr)
        return 2

    only_baseline = sorted(set(baseline) - set(current))
    only_current = sorted(set(current) - set(baseline))
    common = sorted(set(baseline) & set(current))

    regressions: list[tuple[str, float, float, float, float]] = []
    improvements: list[tuple[str, float, float, float]] = []
    flat: list[str] = []

    for bench_id in common:
        b_pt, b_lo, b_hi = load_mean(baseline[bench_id])
        c_pt, c_lo, c_hi = load_mean(current[bench_id])
        # Fraction slower: positive = regression.
        delta = (c_pt - b_pt) / b_pt
        # Statistical significance: CIs do not overlap.
        slower_significant = c_lo > b_hi
        faster_significant = c_hi < b_lo
        if delta > args.threshold and slower_significant:
            regressions.append((bench_id, b_pt, c_pt, delta, c_lo - b_hi))
        elif delta < -args.threshold and faster_significant:
            improvements.append((bench_id, b_pt, c_pt, delta))
        else:
            flat.append(bench_id)

    lines: list[str] = []
    lines.append("# Bench-Regression-Report")
    lines.append("")
    lines.append(
        f"baseline-dir: `{args.baseline_dir}`  current-dir: `{args.current_dir}`  "
        f"threshold: {args.threshold * 100:.1f}%"
    )
    lines.append("")
    lines.append(
        f"benchmarks: {len(common)} compared, "
        f"{len(only_baseline)} only-in-baseline, {len(only_current)} only-in-current"
    )
    lines.append("")

    if regressions:
        lines.append(f"## Regressions ({len(regressions)})")
        lines.append("")
        lines.append("| bench | baseline ns | current ns | delta % | gap ns |")
        lines.append("|---|--:|--:|--:|--:|")
        for bench_id, b_pt, c_pt, delta, gap in regressions:
            lines.append(
                f"| `{bench_id}` | {b_pt:.2f} | {c_pt:.2f} | "
                f"{delta * 100:+.1f}% | {gap:+.2f} |"
            )
        lines.append("")

    if improvements:
        lines.append(f"## Improvements ({len(improvements)})")
        lines.append("")
        for bench_id, b_pt, c_pt, delta in improvements:
            lines.append(f"- `{bench_id}` {b_pt:.2f}ns → {c_pt:.2f}ns ({delta * 100:+.1f}%)")
        lines.append("")

    if flat:
        lines.append(f"## Flat / within noise ({len(flat)})")
        lines.append("")
        lines.append(", ".join(f"`{b}`" for b in flat))
        lines.append("")

    if only_baseline:
        lines.append(f"## Only in baseline ({len(only_baseline)})")
        lines.append("")
        lines.append(", ".join(f"`{b}`" for b in only_baseline))
        lines.append("")

    if only_current:
        lines.append(f"## Only in current ({len(only_current)})")
        lines.append("")
        lines.append(", ".join(f"`{b}`" for b in only_current))
        lines.append("")

    report = "\n".join(lines)
    if args.report:
        args.report.write_text(report)
    print(report)

    if regressions:
        print(
            f"\nFAIL: {len(regressions)} benchmark(s) regressed > "
            f"{args.threshold * 100:.1f}% with non-overlapping confidence intervals.",
            file=sys.stderr,
        )
        return 1

    print("\nPASS: no statistically-significant regressions over threshold.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
