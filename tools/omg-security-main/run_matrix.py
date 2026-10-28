#!/usr/bin/env python3
"""Drive the 275 cells of the OMG ``dds-security`` compatibility matrix:
11 topics × 5 reader permissions files × 5 writer permissions files, one
reader process and one writer process per cell, the verdict read off
the reader's stdout, a JUnit report at the end.

The suite ships clients and ``compatibility_matrix.xlsx`` and no
harness. The expectation is ``compatibility_matrix.tsv`` beside this
file, transcribed from that spreadsheet (see the header of the TSV).

A cell *communicates* when the reader prints a sample line inside the
window. Both spellings the suite's clients use are accepted: the
``%-10s %-10s %03d %03d [%d]`` line of the committed
``ShapeType_main.cxx`` and the ``Received sample...`` line of the
README's older transcript.

Each side is described by a JSON *profile*::

    {
      "name": "zerodds",
      "exe": "/path/to/shapetype_main",
      "cwd": "/root/omg-suites/dds-security/srcCxx",
      "common_args": ["-identityCa", "...", "-acceptX509v1IdentityCerts"],
      "pub_args": [],
      "sub_args": [],
      "governance": "../TESTONLY_governance_signed.p7s",
      "permissions": {
        "Join": "...join_signed.p7s", "Read": "...", "Write": "...",
        "ReadWrite": "...", "AllowAll": "..."
      },
      "env": {}
    }

The suite's own clients hardcode their certificate paths relative to
``srcCxx``; ``cwd`` is where they have to be started, and every path in
the profile is resolved against it by the client, not by this driver.

Usage::

    run_matrix.py --reader zerodds.json --writer rti.json \
        -o report.xml --outdir logs [--cells PD_RWA_EM_ED:Read:Write ...]
"""

from __future__ import annotations

import argparse
import json
import os
import re
import signal
import subprocess
import sys
import threading
import time
from pathlib import Path
from xml.sax.saxutils import escape

HERE = Path(__file__).resolve().parent
EXPECTED_TSV = HERE / "compatibility_matrix.tsv"

PERMISSIONS = ["Join", "Read", "Write", "ReadWrite", "AllowAll"]
SAMPLE_LINE = re.compile(r"^\S+\s+\S+\s+\d+\s+\d+\s+\[\d+\]\s*$|Received sample")


def load_expected(path: Path) -> list[tuple[str, str, str, bool]]:
    cells = []
    for line in path.read_text(encoding="utf-8").splitlines():
        if not line.strip() or line.startswith("#"):
            continue
        topic, reader, writer, expected = line.split("\t")
        if reader not in PERMISSIONS or writer not in PERMISSIONS or expected not in ("OK", "NO"):
            sys.exit(f"{path}: malformed row {line!r}")
        cells.append((topic, reader, writer, expected == "OK"))
    if len(cells) != 275:
        sys.exit(f"{path}: expected 275 cells, found {len(cells)}")
    return cells


def load_profile(path: str) -> dict:
    p = json.loads(Path(path).read_text(encoding="utf-8"))
    for key in ("name", "exe", "cwd", "governance", "permissions"):
        if key not in p:
            sys.exit(f"{path}: profile lacks {key!r}")
    for perm in PERMISSIONS:
        if perm not in p["permissions"]:
            sys.exit(f"{path}: profile lacks permissions[{perm!r}]")
    p.setdefault("common_args", [])
    p.setdefault("pub_args", [])
    p.setdefault("sub_args", [])
    p.setdefault("env", {})
    return p


class Proc:
    def __init__(self, argv: list[str], cwd: str, env: dict, log: Path):
        self.log = log
        self.got_sample = threading.Event()
        self.lines: list[str] = []
        full_env = dict(os.environ)
        full_env.update(env)
        self.proc = subprocess.Popen(
            argv,
            cwd=cwd,
            env=full_env,
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
            if SAMPLE_LINE.search(line):
                self.got_sample.set()

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


def side_argv(profile: dict, role: str, topic: str, perm: str) -> list[str]:
    argv = [profile["exe"], *profile["common_args"]]
    argv += [f"-{role}", topic, "-governance", profile["governance"], "-permissions", profile["permissions"][perm]]
    argv += profile["pub_args"] if role == "pub" else profile["sub_args"]
    return argv


def run_cell(reader: dict, writer: dict, topic: str, rperm: str, wperm: str, args: argparse.Namespace) -> tuple[bool, str]:
    tag = f"{topic}__R-{rperm}__W-{wperm}"
    outdir = Path(args.outdir)
    sub = Proc(side_argv(reader, "sub", topic, rperm), reader["cwd"], reader["env"], outdir / f"{tag}.reader.log")
    time.sleep(args.sub_lead)
    pub = Proc(side_argv(writer, "pub", topic, wperm), writer["cwd"], writer["env"], outdir / f"{tag}.writer.log")
    got = sub.got_sample.wait(timeout=args.window)
    # A reader that exits early (refused permissions) ends the wait too.
    sub_rc = sub.stop()
    pub_rc = pub.stop()
    return got, f"reader_rc={sub_rc} writer_rc={pub_rc}"


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--reader", required=True, help="profile JSON of the subscribing product")
    ap.add_argument("--writer", required=True, help="profile JSON of the publishing product")
    ap.add_argument("--window", type=float, default=15.0, help="seconds to wait for the reader's first sample")
    ap.add_argument("--sub-lead", type=float, default=2.0, help="seconds the reader starts ahead of the writer")
    ap.add_argument("--cells", nargs="*", default=None, help="topic:reader_perm:writer_perm cells to run")
    ap.add_argument("--topics", nargs="*", default=None, help="restrict to these topics")
    ap.add_argument("--expected", default=str(EXPECTED_TSV))
    ap.add_argument("-o", "--output", default="dds_security_report.xml")
    ap.add_argument("--outdir", default="dds_security_logs")
    args = ap.parse_args()

    reader = load_profile(args.reader)
    writer = load_profile(args.writer)
    cells = load_expected(Path(args.expected))
    if args.cells:
        wanted = {tuple(c.split(":")) for c in args.cells}
        cells = [c for c in cells if (c[0], c[1], c[2]) in wanted]
    if args.topics:
        cells = [c for c in cells if c[0] in args.topics]
    Path(args.outdir).mkdir(parents=True, exist_ok=True)
    suite = f"{reader['name']}-reader---{writer['name']}-writer"

    failures = 0
    cases = []
    for topic, rperm, wperm, want in cells:
        t0 = time.monotonic()
        got, note = run_cell(reader, writer, topic, rperm, wperm, args)
        dt = time.monotonic() - t0
        name = f"{topic}__R-{rperm}__W-{wperm}"
        verdict = "OK" if got == want else "ERROR"
        print(
            f"{name} : {verdict} (expected {'OK' if want else 'NO'}, got {'OK' if got else 'NO'}, {dt:.1f}s, {note})",
            flush=True,
        )
        body = ""
        if got != want:
            failures += 1
            body = f'<failure message="expected {"OK" if want else "NO"}, measured {"OK" if got else "NO"}">{escape(note)}</failure>'
        cases.append(f'<testcase classname="{suite}" name="{name}" time="{dt:.3f}">{body}</testcase>')

    xml = (
        '<?xml version="1.0" encoding="UTF-8"?>\n'
        f'<testsuite name="{suite}" tests="{len(cells)}" failures="{failures}" errors="0">\n'
        + "\n".join(cases)
        + "\n</testsuite>\n"
    )
    Path(args.output).write_text(xml, encoding="utf-8")
    print(f"tests={len(cells)} failures={failures} -> {args.output}", flush=True)
    return 1 if failures else 0


if __name__ == "__main__":
    sys.exit(main())
