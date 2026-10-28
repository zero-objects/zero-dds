#!/usr/bin/env python3
"""Drive the 625 publisher/subscriber pairs of the OMG ``dds-xtypes`` suite
(``master``, 25 compiled-in types) and compare each verdict with
``compatibility_matrix.xlsx``.

The suite ships one C++ client and a spreadsheet and no harness. This is
the harness: one process per side, one pair at a time per worker, the
verdict read off the subscriber's stdout, a JUnit report at the end.

The expectation is the transcription in ``tests/compatibility_matrix.rs``
(``SUITE_EXPECTED``), read from that file so that there is one copy of
the spreadsheet's colouring in the tree. Type order is the order of
``VARIANT_NAMES``: version outermost, extensibility kind innermost.

A pair is *compatible* when the subscriber prints at least one
``Read data for Topic`` line inside the window. Nothing else is read:
the suite's own clients exit ``OK`` on a subscriber timeout, so the exit
status carries no verdict.

Usage::

    run_matrix.py --pub-exe ./xtypes_main --sub-exe ./xtypes_main \
        --domain 40 --workers 4 -o report.xml --outdir logs

``--pairs Shape1Final:Shape2Final`` restricts the run (writer:reader,
repeatable); ``--types`` restricts both sides to a list. The RTI binary
of release ``2020-11-03`` knows neither ``-samples`` nor ``-timeout``;
``--pub-extra`` and ``--sub-extra`` are therefore the only per-side
arguments beyond ``-pub``/``-sub``, ``-domain`` and ``-type``, and the
driver ends both processes itself.
"""

from __future__ import annotations

import argparse
import os
import re
import signal
import subprocess
import sys
import threading
import time
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path
from xml.sax.saxutils import escape

HERE = Path(__file__).resolve().parent
EXPECTED_SOURCE = HERE / "tests" / "compatibility_matrix.rs"

VERSIONS = ["Shape1", "Shape2", "Shape3", "Shape4", "Shape5"]
KINDS = ["Default", "Final", "Extensible", "Mutable", "MutableExplicitID"]
TYPES = [v + k for v in VERSIONS for k in KINDS]

RECEIVED = re.compile(r"Read data for Topic")


def load_expected(path: Path) -> dict[tuple[str, str], bool]:
    """Read ``SUITE_EXPECTED`` out of the Rust test file."""
    text = path.read_text(encoding="utf-8")
    start = text.index("const SUITE_EXPECTED")
    end = text.index("\n];", start)
    rows = re.findall(r'b"([01]{25})"', text[start:end])
    if len(rows) != 25:
        sys.exit(f"{path}: expected 25 rows in SUITE_EXPECTED, found {len(rows)}")
    table: dict[tuple[str, str], bool] = {}
    for w, row in enumerate(rows):
        for r, cell in enumerate(row):
            table[(TYPES[w], TYPES[r])] = cell == "1"
    return table


class Proc:
    """One client process whose stdout is scanned as it arrives."""

    def __init__(self, argv: list[str], log: Path):
        self.log = log
        self.received = threading.Event()
        self.lines: list[str] = []
        self.proc = subprocess.Popen(
            argv,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            text=True,
            bufsize=1,
            start_new_session=True,
        )
        self.thread = threading.Thread(target=self._pump, daemon=True)
        self.thread.start()

    def _pump(self) -> None:
        assert self.proc.stdout is not None
        for line in self.proc.stdout:
            self.lines.append(line)
            if RECEIVED.search(line):
                self.received.set()

    def stop(self) -> int:
        for sig in (signal.SIGTERM, signal.SIGKILL):
            if self.proc.poll() is not None:
                break
            try:
                os.killpg(self.proc.pid, sig)
            except ProcessLookupError:
                break
            try:
                self.proc.wait(timeout=3)
            except subprocess.TimeoutExpired:
                continue
        self.thread.join(timeout=3)
        self.log.write_text("".join(self.lines), encoding="utf-8")
        return self.proc.returncode if self.proc.returncode is not None else -1


def run_pair(
    writer: str,
    reader: str,
    domain: int,
    args: argparse.Namespace,
) -> tuple[bool, str]:
    """Return (compatible, note) for one writer/reader pair."""
    tag = f"{writer}_to_{reader}"
    outdir = Path(args.outdir)
    sub_argv = [*args.sub_exe.split(), "-sub", "-domain", str(domain), "-type", reader, *args.sub_extra]
    pub_argv = [*args.pub_exe.split(), "-pub", "-domain", str(domain), "-type", writer, *args.pub_extra]
    sub = Proc(sub_argv, outdir / f"{tag}.sub.log")
    time.sleep(args.sub_lead)
    pub = Proc(pub_argv, outdir / f"{tag}.pub.log")
    got = sub.received.wait(timeout=args.window)
    sub_rc = sub.stop()
    pub_rc = pub.stop()
    note = f"domain={domain} sub_rc={sub_rc} pub_rc={pub_rc}"
    return got, note


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--pub-exe", required=True, help="publisher executable (may carry fixed arguments)")
    ap.add_argument("--sub-exe", required=True, help="subscriber executable (may carry fixed arguments)")
    ap.add_argument("--domain", type=int, default=40, help="first DDS domain; worker i uses domain+i")
    ap.add_argument("--workers", type=int, default=4)
    ap.add_argument("--window", type=float, default=8.0, help="seconds to wait for the first received sample")
    ap.add_argument("--sub-lead", type=float, default=1.0, help="seconds the subscriber starts ahead of the publisher")
    ap.add_argument("--pub-extra", default="", help="extra publisher arguments as one string, e.g. '-samples 200 -timeout 20'")
    ap.add_argument("--sub-extra", default="", help="extra subscriber arguments as one string")
    ap.add_argument("--types", nargs="*", default=None, help="restrict both sides to these type names")
    ap.add_argument("--pairs", nargs="*", default=None, help="writer:reader pairs to run")
    ap.add_argument("--expected", default=str(EXPECTED_SOURCE), help="Rust file carrying SUITE_EXPECTED")
    ap.add_argument("-o", "--output", default="xtypes_v1_report.xml")
    ap.add_argument("--outdir", default="xtypes_v1_logs")
    ap.add_argument("--suite-name", default="dds-xtypes-v1")
    args = ap.parse_args()
    args.pub_extra = args.pub_extra.split()
    args.sub_extra = args.sub_extra.split()

    expected = load_expected(Path(args.expected))
    Path(args.outdir).mkdir(parents=True, exist_ok=True)

    if args.pairs:
        pairs = []
        for p in args.pairs:
            w, r = p.split(":")
            if w not in TYPES or r not in TYPES:
                sys.exit(f"unknown type in pair {p}")
            pairs.append((w, r))
    else:
        types = args.types or TYPES
        for t in types:
            if t not in TYPES:
                sys.exit(f"unknown type {t}")
        pairs = [(w, r) for w in types for r in types]

    lock = threading.Lock()
    results: dict[tuple[str, str], tuple[bool, str, float]] = {}
    slots = list(range(args.workers))
    slot_lock = threading.Lock()

    def job(pair: tuple[str, str]) -> None:
        with slot_lock:
            slot = slots.pop()
        try:
            t0 = time.monotonic()
            got, note = run_pair(pair[0], pair[1], args.domain + slot, args)
            dt = time.monotonic() - t0
        finally:
            with slot_lock:
                slots.append(slot)
        want = expected[pair]
        verdict = "OK" if got == want else "ERROR"
        with lock:
            results[pair] = (got, note, dt)
            print(
                f"{pair[0]}_to_{pair[1]} : {verdict} (expected {'compatible' if want else 'incompatible'}, "
                f"got {'compatible' if got else 'incompatible'}, {dt:.1f}s, {note})",
                flush=True,
            )

    with ThreadPoolExecutor(max_workers=args.workers) as ex:
        list(ex.map(job, pairs))

    failures = 0
    cases = []
    for pair in pairs:
        got, note, dt = results[pair]
        want = expected[pair]
        name = f"{pair[0]}_to_{pair[1]}"
        body = ""
        if got != want:
            failures += 1
            body = (
                f'<failure message="expected {"compatible" if want else "incompatible"}, '
                f'measured {"compatible" if got else "incompatible"}">{escape(note)}</failure>'
            )
        cases.append(f'<testcase classname="{args.suite_name}" name="{name}" time="{dt:.3f}">{body}</testcase>')
    xml = (
        '<?xml version="1.0" encoding="UTF-8"?>\n'
        f'<testsuite name="{args.suite_name}" tests="{len(pairs)}" failures="{failures}" errors="0">\n'
        + "\n".join(cases)
        + "\n</testsuite>\n"
    )
    Path(args.output).write_text(xml, encoding="utf-8")
    print(f"tests={len(pairs)} failures={failures} -> {args.output}", flush=True)
    return 1 if failures else 0


if __name__ == "__main__":
    sys.exit(main())
