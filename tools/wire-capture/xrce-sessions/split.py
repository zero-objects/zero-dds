#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 ZeroDDS Contributors
"""Turn `capture.sh`'s pcap into the two per-direction hex files.

    tools/wire-capture/xrce-sessions/split.py --capture ~/xrce-capture

Reads `session-udp.pcap`, writes `client-to-agent.hex` and
`agent-to-client.hex` beside it — one UDP payload per line, lower-case
hex, no separators. Those two files are what
`crates/xrce/tests/foreign_frames.rs` reads.

Direction is decided by the agent's port and not by address, because
both endpoints are on loopback: a datagram *to* the agent port is the
client's, one *from* it is the agent's. A datagram that is neither is
counted and reported rather than dropped — a silent drop here would
show up as a shorter session and nothing else.

`tshark` does the extraction. It is the same tool the recorded session
was extracted with, and re-deriving the two files from the pcap a
second time is how the recorded ones were checked.
"""

import argparse
import pathlib
import subprocess
import sys

FIELDS = ("-e", "udp.srcport", "-e", "udp.dstport", "-e", "data.data")


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--capture", required=True, type=pathlib.Path,
                    help="the directory capture.sh wrote")
    ap.add_argument("--pcap", default="session-udp.pcap",
                    help="the capture file inside it")
    ap.add_argument("--agent-port", type=int, default=2018)
    args = ap.parse_args()

    pcap = args.capture / args.pcap
    if not pcap.is_file():
        print(f"NOT PRESENT: {pcap}", file=sys.stderr)
        return 2

    try:
        out = subprocess.run(
            ("tshark", "-r", str(pcap), "-T", "fields", *FIELDS),
            check=True, capture_output=True, text=True,
        ).stdout
    except FileNotFoundError:
        print("NOT PRESENT: tshark", file=sys.stderr)
        return 2

    c2a: list[str] = []
    a2c: list[str] = []
    neither = 0
    for line in out.splitlines():
        parts = line.split("\t")
        if len(parts) < 3 or not parts[2]:
            continue
        src, dst, payload = int(parts[0]), int(parts[1]), parts[2].replace(":", "").lower()
        if dst == args.agent_port:
            c2a.append(payload)
        elif src == args.agent_port:
            a2c.append(payload)
        else:
            neither += 1

    for name, rows in (("client-to-agent.hex", c2a), ("agent-to-client.hex", a2c)):
        (args.capture / name).write_text("\n".join(rows) + "\n", encoding="ascii")
        print(f"{name}: {len(rows)}")

    if neither:
        print(f"{neither} datagrams touched neither side of port {args.agent_port} "
              f"and are in no file", file=sys.stderr)
    if len(c2a) != len(a2c):
        print(f"the two directions differ in count: {len(c2a)} vs {len(a2c)}", file=sys.stderr)
    return 0


if __name__ == "__main__":
    sys.exit(main())
