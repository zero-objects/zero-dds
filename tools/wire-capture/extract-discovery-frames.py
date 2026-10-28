#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 ZeroDDS Contributors
"""The discovery datagrams of a captured RTPS session, one per line.

`capture-rtps-session.sh` keeps everything an implementation sent for the
length of a session — a few thousand datagrams, most of them repeats of
the same announcement and the same HEARTBEAT/ACKNACK pair. A discovery
round-trip test needs the variety and not the volume, so this keeps one
datagram per *distinct announcement*: the DATA submessages whose writer
is a builtin discovery writer, deduplicated on the serialized payload.

    tools/wire-capture/extract-discovery-frames.py \\
        /root/rtps-sessions/cyclone/session.frames \\
        --out crates/discovery/tests/fixtures/vendor-discovery/cyclone.frames

The output format is the input's: one hex string per line, one line per
UDP payload, from the `RTPS` magic to the last octet. A `#` line carries
the provenance the fixture is read with.

Deduplication is on the payload and not on the datagram, because the
sequence number and the INFO_TS timestamp in front of it change between
two announcements that say the same thing, and keeping both would grow
the fixture without widening what it covers.
"""

import argparse
import struct
import sys

# DDSI-RTPS 2.5 §9.4.5.1 submessage kinds.
SM_DATA = 0x15
SM_DATA_FRAG = 0x16

# §9.3.1.2 builtin discovery writers, plus the DDS-Security ones
# (DDS-Security 1.1 §7.4.5) and the XTypes TypeLookup service.
DISCOVERY_WRITERS = {
    0x000100C2: "SPDP participant",
    0x000003C2: "SEDP publications",
    0x000004C2: "SEDP subscriptions",
    0x000002C2: "SEDP topics",
    0xFF0003C2: "SEDP publications secure",
    0xFF0004C2: "SEDP subscriptions secure",
    0xFF0101C2: "SPDP participant secure",
    0x000201C3: "participant message",
    0xFF0202C3: "participant message secure",
    0x000300C2: "TypeLookup request",
    0x000301C2: "TypeLookup reply",
}


def submessages(dg):
    """Yields (kind, flags, body) for every submessage of a datagram."""
    if len(dg) < 20 or dg[:4] != b"RTPS":
        return
    at = 20
    while at + 4 <= len(dg):
        kind = dg[at]
        flags = dg[at + 1]
        little = flags & 0x01
        (length,) = struct.unpack_from("<H" if little else ">H", dg, at + 2)
        at += 4
        # §8.3.3.2.3: octetsToNextHeader 0 on the last submessage means
        # "to the end of the datagram".
        end = len(dg) if length == 0 else at + length
        if end > len(dg):
            return
        yield kind, flags, dg[at:end]
        at = end


def discovery_payloads(dg):
    """Yields (writer_id, serialized_payload) per discovery DATA."""
    for kind, flags, body in submessages(dg):
        if kind not in (SM_DATA, SM_DATA_FRAG):
            continue
        little = flags & 0x01
        if len(body) < 16:
            continue
        endian = "<" if little else ">"
        (octets_to_inline_qos,) = struct.unpack_from(endian + "H", body, 2)
        (writer_id,) = struct.unpack_from(">I", body, 8)
        if writer_id not in DISCOVERY_WRITERS:
            continue
        # §9.4.5.3.1: octetsToInlineQos counts from the octet after the
        # field itself. Inline QoS, when present, sits there; the payload
        # follows it. A DATA without inline QoS (Q=0) has the payload
        # right after.
        at = 4 + octets_to_inline_qos
        if flags & 0x02:  # Q — inline QoS present
            at = skip_parameter_list(body, at, little)
            if at is None:
                continue
        if kind == SM_DATA_FRAG:
            # A fragment carries part of a payload; a fixture built from
            # one would be a truncated announcement.
            continue
        yield writer_id, body[at:]


def skip_parameter_list(body, at, little):
    """Returns the offset after a ParameterList's sentinel."""
    endian = "<" if little else ">"
    while at + 4 <= len(body):
        pid, length = struct.unpack_from(endian + "HH", body, at)
        at += 4
        if pid == 0x0001:  # PID_SENTINEL
            return at
        at += length
    return None


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("frames", help="a session.frames from capture-rtps-session.sh")
    ap.add_argument("--out", required=True)
    ap.add_argument("--provenance", default="", help="a line for the fixture header")
    args = ap.parse_args()

    seen_payloads = set()
    kept = []
    counts = {}
    total = 0
    with open(args.frames) as fh:
        for line in fh:
            line = line.strip()
            if not line or line.startswith("#"):
                continue
            total += 1
            dg = bytes.fromhex(line)
            novel = False
            for writer_id, payload in discovery_payloads(dg):
                key = (writer_id, payload)
                if key in seen_payloads:
                    continue
                seen_payloads.add(key)
                counts[DISCOVERY_WRITERS[writer_id]] = (
                    counts.get(DISCOVERY_WRITERS[writer_id], 0) + 1
                )
                novel = True
            if novel:
                kept.append(line)

    with open(args.out, "w") as fh:
        if args.provenance:
            fh.write(f"# {args.provenance}\n")
        for line in kept:
            fh.write(line + "\n")

    print(f"{args.frames}: {total} datagrams in, {len(kept)} kept", file=sys.stderr)
    for name in sorted(counts):
        print(f"  {name}\t{counts[name]}", file=sys.stderr)
    if not kept:
        print("no discovery datagram — an empty fixture passes every "
              "assertion it is not run against", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
