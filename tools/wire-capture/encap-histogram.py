"""Which encapsulations a capture's user samples actually carry.

    python3 encap-histogram.py session.pcap

One line per distinct RTPS encapsulation identifier (RTPS 2.5 §10.5) with the
number of user DATA submessages that carried it. Answers "which
DataRepresentation is this peer really writing", which a sample count cannot:
a reader decodes from the header it receives, so a pairing can pass under an
accept list that names the other representation.

`decode-rtps.py` extracts the same submessages, but splits each payload
according to one probe type's member layout. This one reads the two octets in
front of that and nothing else, so it works on any capture.

Builtin traffic is excluded by the writer's entity kind (§9.3.1.2): 0x02 and
0x03 are user writers, the builtin kinds are 0xC2/0xC7. The writer id's prefix
is not a usable filter — SPDP's is 0x000100C2 and SEDP's 0x000003C2.
"""

import collections
import subprocess
import sys

NAMES = {
    0x0000: "CDR_BE",
    0x0001: "CDR_LE",
    0x0002: "PL_CDR_BE",
    0x0003: "PL_CDR_LE",
    0x0006: "CDR2_BE",
    0x0007: "PLAIN_CDR2_LE",
    0x0008: "D_CDR2_BE",
    0x0009: "D_CDR2_LE",
    0x000A: "PL_CDR2_BE",
    0x000B: "PL_CDR2_LE",
}

# XCDR1 carries the first four, XCDR2 the last four; the two PL_CDR forms are
# XCDR1's @mutable framing.
XCDR1 = {0x0000, 0x0001, 0x0002, 0x0003}


def payloads(pcap):
    out = subprocess.run(
        ["tshark", "-r", pcap, "-Y", "udp", "-T", "fields", "-e", "udp.payload"],
        capture_output=True,
        text=True,
    )
    if out.returncode != 0:
        sys.exit("tshark: " + out.stderr.strip()[:400])
    for line in out.stdout.splitlines():
        line = line.strip().replace(":", "")
        if line:
            yield bytes.fromhex(line)


def user_data(pkt):
    """Serialized payloads of the user DATA submessages in one datagram."""
    if pkt[:4] != b"RTPS":
        return
    o = 20
    while o + 4 <= len(pkt):
        sid = pkt[o]
        flags = pkt[o + 1]
        end = "little" if flags & 1 else "big"
        nxt = int.from_bytes(pkt[o + 2 : o + 4], end)
        body = o + 4
        size = nxt if nxt else len(pkt) - body
        # D (0x04) means the payload is the data; K (0x08) means it is the key
        # only, which carries an encapsulation of its own and is not a sample.
        if sid == 0x15 and flags & 0x04:
            oti = int.from_bytes(pkt[body + 2 : body + 4], end)
            writer = pkt[body + 8 : body + 12]
            p2 = body + 4 + oti
            if flags & 0x02:  # inline QoS: skip the ParameterList
                while p2 + 4 <= body + size:
                    pid = int.from_bytes(pkt[p2 : p2 + 2], end)
                    plen = int.from_bytes(pkt[p2 + 2 : p2 + 4], end)
                    p2 += 4
                    if pid == 1:  # PID_SENTINEL
                        break
                    p2 += plen
            if writer[3] in (0x02, 0x03) and p2 + 2 <= len(pkt):
                yield pkt[p2 : p2 + 2]
        o = body + size


def main():
    seen = collections.Counter()
    for pkt in payloads(sys.argv[1]):
        for enc in user_data(pkt):
            seen[int.from_bytes(enc, "big")] += 1
    if not seen:
        print("no user DATA submessage in", sys.argv[1])
        return
    for key, count in seen.most_common():
        version = "XCDR1" if key in XCDR1 else "XCDR2"
        print(f"{key:#06x}  {NAMES.get(key, 'unknown'):<14} {version}  {count}")


main()
