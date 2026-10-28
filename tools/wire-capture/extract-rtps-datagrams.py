#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 ZeroDDS Contributors
#
# Every RTPS datagram in a classic-`pcap` capture, as one hex line.
#
#   extract-rtps-datagrams.py <capture.pcap> [--out session.frames]
#                             [--limit N] [--summary]
#
# The companion of `extract-rtps.py`, and the difference is the whole
# reason it exists: that one keeps the UDP payload of packets that
# carried a **user `DATA`**, because its question is what a serialiser
# put in a sample. This one keeps **every** datagram whose payload
# begins with `RTPS`, because its question is what the protocol stack
# put on the socket — SPDP participant announcements, SEDP publication
# and subscription announcements, `HEARTBEAT`, `ACKNACK`, `GAP`,
# `INFO_*`, the vendor submessages, and the user data among them.
#
# Reads the capture itself rather than asking `tshark` to dissect it,
# so a committed `.frames` file can be re-decoded on a machine with no
# `tshark` and no vendor installed.
#
# ## Duplicates
#
# A loopback capture sees a packet once per direction it crosses the
# interface, and a participant repeats its SPDP announcement on a timer
# for as long as it lives. Both would fill the file with octets that
# add no case. Identical datagrams are therefore written once, in the
# order of first appearance, and `--summary` reports how many were
# collapsed — a count that is dropped silently is a count nobody can
# check.
import argparse
import struct
import sys


def pcap_packets(path):
    """(link type, link-layer bytes) per record."""
    with open(path, "rb") as handle:
        header = handle.read(24)
        if len(header) != 24:
            raise SystemExit(f"{path}: not a pcap file")
        magic = header[:4]
        if magic == b"\xd4\xc3\xb2\xa1":
            endian, nanos = "<", False
        elif magic == b"\xa1\xb2\xc3\xd4":
            endian, nanos = ">", False
        elif magic == b"\x4d\x3c\xb2\xa1":
            endian, nanos = "<", True
        elif magic == b"\xa1\xb2\x3c\x4d":
            endian, nanos = ">", True
        else:
            raise SystemExit(
                f"{path}: not a classic pcap — capture with `-F pcap`"
            )
        del nanos
        link = struct.unpack(endian + "I", header[20:24])[0]
        while True:
            record = handle.read(16)
            if len(record) < 16:
                return
            _ts_sec, _ts_sub, captured, original = struct.unpack(
                endian + "IIII", record
            )
            data = handle.read(captured)
            if len(data) < captured:
                return
            if captured < original:
                raise SystemExit(
                    f"{path}: packet truncated at {captured} of {original} "
                    "octets — capture with `-s 0`"
                )
            yield link, data


def udp_payload(link, frame):
    """(source port, destination port, UDP payload), or None."""
    if link == 1:            # Ethernet
        if len(frame) < 14:
            return None
        rest, offset = struct.unpack(">H", frame[12:14])[0], 14
    elif link == 113:        # Linux cooked capture
        if len(frame) < 16:
            return None
        rest, offset = struct.unpack(">H", frame[14:16])[0], 16
    elif link == 276:        # Linux cooked capture v2
        if len(frame) < 20:
            return None
        rest, offset = struct.unpack(">H", frame[0:2])[0], 20
    elif link == 101:        # raw IP
        rest, offset = 0x0800, 0
    else:
        return None
    if rest not in (0x0800, 0x86DD):
        return None
    if rest == 0x0800:
        if len(frame) < offset + 20:
            return None
        ihl = (frame[offset] & 0x0F) * 4
        if frame[offset + 9] != 17:
            return None
        udp = offset + ihl
    else:
        if len(frame) < offset + 40:
            return None
        if frame[offset + 6] != 17:
            return None
        udp = offset + 40
    if len(frame) < udp + 8:
        return None
    src, dst, length = struct.unpack(">HHH", frame[udp : udp + 6])
    return src, dst, frame[udp + 8 : udp + max(length, 8)]


SUBMESSAGE_NAMES = {
    0x01: "PAD", 0x06: "ACKNACK", 0x07: "HEARTBEAT", 0x08: "GAP",
    0x09: "INFO_TS", 0x0A: "INFO_SRC", 0x0C: "INFO_REPLY_IP4",
    0x0E: "INFO_DST", 0x0F: "INFO_REPLY", 0x12: "NACK_FRAG",
    0x13: "HEARTBEAT_FRAG", 0x15: "DATA", 0x16: "DATA_FRAG",
    0x30: "SEC_BODY", 0x31: "SEC_PREFIX", 0x32: "SEC_POSTFIX",
    0x33: "SRTPS_PREFIX", 0x34: "SRTPS_POSTFIX", 0x80: "HEADER_EXTENSION",
}

# DDSI-RTPS 2.5 §9.3.1.2 builtin entity ids, by writer. Reported so a
# session capture states which discovery endpoints it actually caught
# rather than leaving it to be assumed from the submessage counts.
BUILTIN_WRITERS = {
    0x000100C2: "SPDP participant",
    0x000003C2: "SEDP publications",
    0x000004C2: "SEDP subscriptions",
    0x000002C2: "SEDP topics",
    0x000200C2: "participant message",
    0x000201C2: "participant message (secure)",
    0xFF0003C2: "SEDP publications (secure)",
    0xFF0004C2: "SEDP subscriptions (secure)",
    0xFF0100C2: "SPDP participant (secure)",
    0x000300C2: "type lookup request",
    0x000301C2: "type lookup reply",
}


def walk(datagram):
    """(submessage id, writer entity id or None) per submessage."""
    offset = 20
    while offset + 4 <= len(datagram):
        sid = datagram[offset]
        flags = datagram[offset + 1]
        endian = "<" if flags & 1 else ">"
        to_next = struct.unpack(endian + "H", datagram[offset + 2 : offset + 4])[0]
        body = offset + 4
        size = to_next if to_next else len(datagram) - body
        if body + size > len(datagram):
            return
        writer = None
        if sid in (0x15, 0x16) and size >= 16:
            writer = struct.unpack(">I", datagram[body + 8 : body + 12])[0]
        elif sid in (0x06, 0x07) and size >= 8:
            writer = struct.unpack(">I", datagram[body + 4 : body + 8])[0]
        yield sid, writer
        offset = body + size


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("capture")
    parser.add_argument("--out", help="write the hex lines here")
    parser.add_argument(
        "--limit", type=int, default=0,
        help="stop after N distinct datagrams (0 = no limit)",
    )
    parser.add_argument(
        "--vendor-id",
        help="keep only datagrams whose RTPS header carries this vendor id, "
             "as four hex digits (0110 Cyclone, 0101 RTI, 010f Fast DDS, "
             "0103 OpenDDS). Attribution read off the message rather than "
             "off the port a capture filter guessed.",
    )
    parser.add_argument(
        "--domain", type=int,
        help="keep only the participants whose SPDP went to this domain's "
             "discovery ports (DDSI-RTPS 2.5 §9.6.1.1), and then every "
             "datagram those participants sent, whatever port it used",
    )
    parser.add_argument("--summary", action="store_true")
    args = parser.parse_args()

    wanted = None
    if args.vendor_id:
        wanted = bytes.fromhex(args.vendor_id)
        if len(wanted) != 2:
            raise SystemExit("--vendor-id takes four hex digits")

    # DDSI-RTPS 2.5 §9.6.1.1: PB + DG*domain + d0 is the domain's
    # multicast discovery port and PB + DG*domain + d1 + PG*i the unicast
    # one of participant i. A participant that sent SPDP to one of them
    # is on this domain, and everything it sends afterwards is this
    # domain's whatever port it used — which is the part a port filter
    # cannot do, because Cyclone 11.0.1 sends user traffic between
    # ephemeral ports.
    on_domain = None
    if args.domain is not None:
        base = 7400 + 250 * args.domain
        discovery = {base} | {base + 10 + 2 * i for i in range(120)}
        on_domain = set()
        for link, frame in pcap_packets(args.capture):
            parsed = udp_payload(link, frame)
            if not parsed:
                continue
            _src, dst, payload = parsed
            if payload[:4] != b"RTPS" or len(payload) < 20:
                continue
            if dst not in discovery:
                continue
            on_domain.add(payload[8:20])

    seen = set()
    order = []
    duplicates = 0
    rejected = {}
    off_domain = 0
    short_header = 0
    ports = {}
    for link, frame in pcap_packets(args.capture):
        parsed = udp_payload(link, frame)
        if not parsed:
            continue
        src, dst, payload = parsed
        if payload[:4] != b"RTPS":
            continue
        # §8.3.3.1 makes the header 20 octets: magic, version, vendor id
        # and a 12-octet guidPrefix. Anything shorter carries no header
        # and so no sender, and counting it as a participant is how a
        # two-participant run reports three: `rtiddsping` sends one
        # 16-octet `RTPS` + `NDDSPING` probe per run.
        if len(payload) < 20:
            short_header += 1
            continue
        vendor = payload[6:8]
        if wanted is not None and vendor != wanted:
            rejected[vendor.hex()] = rejected.get(vendor.hex(), 0) + 1
            continue
        if on_domain is not None and payload[8:20] not in on_domain:
            off_domain += 1
            continue
        if payload in seen:
            duplicates += 1
            continue
        seen.add(payload)
        order.append(payload)
        ports[(src, dst)] = ports.get((src, dst), 0) + 1
        if args.limit and len(order) >= args.limit:
            break

    if args.out:
        with open(args.out, "w") as handle:
            for datagram in order:
                handle.write(datagram.hex() + "\n")

    if args.summary:
        kinds = {}
        writers = {}
        for datagram in order:
            for sid, writer in walk(datagram):
                name = SUBMESSAGE_NAMES.get(sid, f"0x{sid:02x}")
                kinds[name] = kinds.get(name, 0) + 1
                if writer is not None and writer in BUILTIN_WRITERS:
                    label = BUILTIN_WRITERS[writer]
                    writers[label] = writers.get(label, 0) + 1
        # The sender's GUID prefix, from the RTPS header's own octets
        # 8..20. One per participant, so the count says how many
        # participants of this vendor were on the wire — which is the
        # only thing that separates this run's pair from another run of
        # the same vendor on the same host.
        senders = {}
        versions = {}
        vendors = {}
        for datagram in order:
            senders[datagram[8:20].hex()] = senders.get(datagram[8:20].hex(), 0) + 1
        for datagram in order:
            versions[f"{datagram[4]}.{datagram[5]}"] = (
                versions.get(f"{datagram[4]}.{datagram[5]}", 0) + 1
            )
            vendors[datagram[6:8].hex()] = vendors.get(datagram[6:8].hex(), 0) + 1
        print(f"datagrams\t{len(order)}")
        print(f"duplicates-collapsed\t{duplicates}")
        for vendor in sorted(rejected):
            print(f"other-vendor-dropped\t{vendor}\t{rejected[vendor]}")
        if on_domain is not None:
            print(f"other-domain-dropped\t{off_domain}")
        print(f"short-header-dropped\t{short_header}")
        for version in sorted(versions):
            print(f"protocol-version\t{version}\t{versions[version]}")
        for vendor in sorted(vendors):
            print(f"vendor-id\t{vendor}\t{vendors[vendor]}")
        print(f"participants\t{len(senders)}")
        for prefix in sorted(senders):
            print(f"participant\t{prefix}\t{senders[prefix]}")
        for name in sorted(kinds):
            print(f"submessage\t{name}\t{kinds[name]}")
        for label in sorted(writers):
            print(f"builtin-writer\t{label}\t{writers[label]}")
        for (src, dst) in sorted(ports):
            print(f"udp\t{src}\t{dst}\t{ports[(src, dst)]}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
