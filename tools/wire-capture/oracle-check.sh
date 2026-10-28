#!/bin/bash
# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 ZeroDDS Contributors
#
# `extract-rtps.py` against Wireshark's RTPS dissector.
#
#   tools/wire-capture/oracle-check.sh
#
# Every other check in this directory is this harness checking itself:
# the four run checks compare the harness's octets against the
# harness's other octets, and `recheck.sh` re-derives the table with
# the same decoder that produced it. None of that can catch a decoder
# that is wrong in the same way twice.
#
# `zerodds-examples/idl-conformance/proofs/rtps-framing/` is an
# independent one. It holds user `DATA` frames from Cyclone 11.0.1,
# Fast DDS 3.6.1 and this project, and beside each frame the decode
# **Wireshark's** RTPS dissector produced for it — the encapsulation
# kind, the options field, and the `serializedData` octets. Those are
# the three columns `extract-rtps.py` produces, from a different
# codebase by different people.
#
# This runs the decoder over those frames and holds its answer against
# the recorded decode. It needs no vendor, no capture tool and no host —
# but it does need that directory, and `zerodds-examples` is its own
# repository, ignored here (`.gitignore` line 122). Where it is not
# checked out beside this tree the script says so and exits 2 rather
# than passing on an absence.
set -uo pipefail

TOOLS="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$TOOLS/../.." && pwd)"
PROOF="$ROOT/zerodds-examples/idl-conformance/proofs/rtps-framing"

if [ ! -d "$PROOF/hex" ]; then
    echo "oracle-check: $PROOF/hex is not here." >&2
    echo '  zerodds-examples is its own repository and is not part of this' >&2
    echo '  one; check it out beside this tree to run the check.' >&2
    exit 2
fi

status=0
for vendor in cyclone fastdds zerodds; do
    hex="$PROOF/hex/$vendor.user_data.hex"
    decode="$PROOF/decodes/$vendor.user_data.txt"
    [ -f "$hex" ] && [ -f "$decode" ] || { echo "$vendor: absent"; continue; }
    python3 - "$vendor" "$hex" "$decode" "$TOOLS" <<'PY'
import re, subprocess, sys, tempfile, os

vendor, hex_path, decode_path, tools = sys.argv[1:5]

# The `.hex` files are Wireshark's frame dump: `offset  xx xx ...  ascii`.
# The frame is Ethernet + IPv4 + UDP, so the payload starts at 42.
octets = bytearray()
for line in open(hex_path, encoding="utf-8", errors="replace"):
    m = re.match(r"^[0-9a-f]{4}\s+((?:[0-9a-f]{2} )+)", line)
    if m:
        octets += bytes.fromhex(m.group(1).replace(" ", ""))
payload = bytes(octets[42:])

with tempfile.NamedTemporaryFile("w", suffix=".frames", delete=False) as handle:
    handle.write(payload.hex() + "\n")
    frames = handle.name
try:
    out = subprocess.run(
        ["python3", os.path.join(tools, "extract-rtps.py"), frames, "--from-frames"],
        capture_output=True, text=True)
finally:
    os.unlink(frames)

rows = [line.split("\t") for line in out.stdout.splitlines()]
if len(rows) != 1:
    print(f"{vendor}: extract-rtps produced {len(rows)} rows, expected 1")
    raise SystemExit(1)
_, kind, _, _, _, _, encapsulation, options, body = rows[0]

text = open(decode_path, encoding="utf-8", errors="replace").read()
want_kind = re.search(r"encapsulation kind:.*\(0x([0-9a-fA-F]{4})\)", text)
want_options = re.search(r"encapsulation options: 0x([0-9a-fA-F]{4})", text)
want_body = re.search(r"serializedData: ([0-9a-fA-F]+)", text)
if not (want_kind and want_options and want_body):
    print(f"{vendor}: the recorded decode does not carry all three fields")
    raise SystemExit(1)

problems = []
if encapsulation.lower() != want_kind.group(1).lower():
    problems.append(f"encapsulation {encapsulation} against {want_kind.group(1)}")
if options.lower() != want_options.group(1).lower():
    problems.append(f"options {options} against {want_options.group(1)}")
if body.lower() != want_body.group(1).lower():
    problems.append("serializedData differs")
if problems:
    print(f"{vendor}: " + "; ".join(problems))
    raise SystemExit(1)
print(f"{vendor}: encapsulation {encapsulation}, options {options}, "
      f"{len(body) // 2} payload octets — same as Wireshark")
PY
    [ $? -eq 0 ] || status=1
done

exit $status
