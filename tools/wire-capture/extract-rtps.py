#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 ZeroDDS Contributors
#
# Every user `DATA` submessage in a classic-`pcap` capture, as a row.
#
#   extract-rtps.py <capture.pcap> [--domain N] [--frames out.frames]
#
# Reads the capture itself rather than asking `tshark` to dissect it, so
# the committed `.frames` files under `frames/` can be re-decoded on a
# machine that has no `tshark` and no vendor installed. `--frames`
# writes the UDP payload of every packet that carried a user `DATA`, one
# hex string per line, which is the smallest artifact from which every
# column below can be re-derived.
#
# The row is
#
#   domain  kind  topic  writer  seq  keyhash  encapsulation  options
#   payload
#
# `topic` comes out of the capture's own SEDP: the builtin publications
# writer announces every user writer with its `PID_ENDPOINT_GUID` and
# its `PID_TOPIC_NAME`, and that pair is what attributes a `DATA` to a
# case. Counting writers instead — "there is only one, so it must be
# ours" — does not survive an RMW, which creates a `ros_discovery_info`
# writer of its own; picking the writer with the expected number of
# samples would be the harness deciding which measurement it wanted.
#
# `kind` is `data` for a `DATA` that carries a sample (the `D` flag),
# `key` for one that carries only the key (`K` without `D`) and `none`
# for one that carries neither. OpenDDS sends a `key` `DATA` ahead of
# the first sample; counting it as a sample shifts every following one
# by a position, which is how the poison and the corpus sample end up
# swapped and the run reports the wrong octets under the right name.
#
# `payload` is the serialized data *after* the four-byte encapsulation
# header, which is what `wireprobe-*` prints for `--emit payload`, so a
# wire row and an in-memory row are directly comparable. `encapsulation`
# and `options` are the header's two halves and are columns of their own
# because they are exactly what an in-memory API does not decide (B4,
# B5) — RTI's static serialiser leaves the options field zero and its
# RTPS layer fills it in.
#
# `keyhash` is `PID_KEY_HASH` out of the inline QoS when the writer sent
# one, and empty when it did not. Nothing but a real writer sends it, so
# it has no in-memory counterpart at all (N12).
#
# ## Which submessages count
#
# `DATA` and `DATA_FRAG` whose writer entity id has kind `0x02`
# (WITH_KEY, user defined) or `0x03` (NO_KEY, user defined). Everything
# builtin has kind `0xc2`/`0xc3` and is discovery. A `DATA` with the `K`
# flag and no `D` flag carries a key and not a sample and is reported
# with an empty payload rather than skipped, because a dispose that was
# mistaken for a sample is exactly the shape of a false green.
import argparse
import struct
import sys

PID_SENTINEL = 0x0001
PID_TOPIC_NAME = 0x0005
PID_KEY_HASH = 0x0070
PID_ENDPOINT_GUID = 0x005A

# DDSI-RTPS 2.5 §9.3.1.2: the builtin publications writer, in both its
# forms — plain and secure discovery use different entity keys.
SEDP_PUBLICATIONS_WRITERS = (b"\x00\x00\x03\xc2", b"\xff\x00\x03\xc2")

SUBMSG_DATA = 0x15
SUBMSG_DATA_FRAG = 0x16
SUBMSG_INFO_SRC = 0x0C


def pcap_packets(path):
    """(seconds, microseconds, link-layer bytes) per record."""
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
        link = struct.unpack(endian + "I", header[20:24])[0]
        while True:
            record = handle.read(16)
            if len(record) < 16:
                return
            ts_sec, ts_sub, captured, original = struct.unpack(
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
            yield (ts_sec, ts_sub if not nanos else ts_sub // 1000, link, data)


def udp_payload(link, frame):
    """The UDP payload, its source and destination port, or None."""
    if link == 1:            # Ethernet
        if len(frame) < 14:
            return None
        ethertype = struct.unpack(">H", frame[12:14])[0]
        rest, offset = ethertype, 14
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
    body = frame[udp + 8 : udp + max(length, 8)]
    return src, dst, body


def parameters(blob, endian):
    """A ParameterList as {pid: value}, and where it ended."""
    found, offset = {}, 0
    while offset + 4 <= len(blob):
        pid, length = struct.unpack(endian + "HH", blob[offset : offset + 4])
        offset += 4
        if pid == PID_SENTINEL:
            return found, offset
        if offset + length > len(blob):
            return found, len(blob)
        found.setdefault(pid, blob[offset : offset + length])
        offset += length
    return found, len(blob)


def sedp_topics(packet, into):
    """Fill `into` with writer GUID -> topic name from SEDP announcements.

    Keyed by the whole sixteen-octet GUID and not by the entity id: two
    participants number their writers independently, so the same entity
    id names a different writer in each. Measured with OpenSplice 6.9,
    which publishes its own built-in topics through ordinary DDSI
    writers: the subscriber process's `DCPSParticipant` writer and the
    publisher's writer of a keyed user topic were both `00000202`, and an
    entity-id map gave the user samples the built-in topic's name.
    """
    if packet[:4] != b"RTPS" or len(packet) < 20:
        return
    offset = 20
    while offset + 4 <= len(packet):
        kind = packet[offset]
        flags = packet[offset + 1]
        endian = "<" if flags & 0x01 else ">"
        to_next = struct.unpack(endian + "H", packet[offset + 2 : offset + 4])[0]
        body = offset + 4
        size = to_next if to_next else len(packet) - body
        end = min(body + size, len(packet))
        if kind == SUBMSG_DATA and body + 20 <= end:
            to_inline = struct.unpack(endian + "H", packet[body + 2 : body + 4])[0]
            writer = packet[body + 8 : body + 12]
            cursor = body + 4 + to_inline
            if flags & 0x02 and cursor < end:
                _, used = parameters(packet[cursor:end], endian)
                cursor += used
            if writer in SEDP_PUBLICATIONS_WRITERS and cursor + 4 <= end:
                blob = packet[cursor + 4 : end]      # past the encapsulation
                inner = "<" if packet[cursor + 1] & 0x01 else ">"
                found, _ = parameters(blob, inner)
                guid = found.get(PID_ENDPOINT_GUID)
                name = found.get(PID_TOPIC_NAME)
                if guid and len(guid) == 16 and name and len(name) >= 4:
                    length = struct.unpack(inner + "I", name[:4])[0]
                    text = name[4 : 4 + max(length - 1, 0)].decode(
                        "utf-8", "replace")
                    into[guid.hex()] = text
        offset = body + size


def user_data(packet):
    """(kind, writer GUID, sequence number, key hash, payload) per DATA.

    The GUID prefix is the message header's, or the last `INFO_SRC`'s
    (DDSI-RTPS 2.5 §8.3.7.9), which is where a relayed submessage says
    whose it is.
    """
    if packet[:4] != b"RTPS" or len(packet) < 20:
        return
    prefix = packet[8:20]
    offset = 20
    while offset + 4 <= len(packet):
        kind = packet[offset]
        flags = packet[offset + 1]
        endian = "<" if flags & 0x01 else ">"
        to_next = struct.unpack(endian + "H", packet[offset + 2 : offset + 4])[0]
        body = offset + 4
        size = to_next if to_next else len(packet) - body
        end = min(body + size, len(packet))
        if kind == SUBMSG_INFO_SRC and body + 20 <= end:
            prefix = packet[body + 8 : body + 20]
        if kind in (SUBMSG_DATA, SUBMSG_DATA_FRAG) and body + 16 <= end:
            to_inline = struct.unpack(endian + "H", packet[body + 2 : body + 4])[0]
            writer = packet[body + 8 : body + 12]
            sequence = struct.unpack(
                endian + "iI", packet[body + 12 : body + 20]
            ) if body + 20 <= end else (0, 0)
            sequence = (sequence[0] << 32) | sequence[1]
            # `octetsToInlineQos` counts from the start of `readerId`.
            cursor = body + 4 + to_inline
            key_hash = b""
            if flags & 0x02 and cursor < end:
                found, used = parameters(packet[cursor:end], endian)
                key_hash = found.get(PID_KEY_HASH, b"")
                cursor += used
            if writer[3] in (0x02, 0x03):
                has_data = bool(flags & 0x04) or kind == SUBMSG_DATA_FRAG
                has_key = bool(flags & 0x08)
                what = "data" if has_data else ("key" if has_key else "none")
                yield what, prefix + writer, sequence, key_hash, packet[cursor:end]
        offset = body + size


def main():
    parser = argparse.ArgumentParser(add_help=True)
    parser.add_argument("capture")
    parser.add_argument("--domain", type=int, default=None,
                        help="only packets whose port maps to this domain")
    parser.add_argument("--frames", default=None,
                        help="write the carrying UDP payloads here, as hex")
    parser.add_argument("--from-frames", action="store_true",
                        help="the input is a `.frames` file, not a pcap")
    arguments = parser.parse_args()

    if arguments.from_frames:
        with open(arguments.capture, encoding="ascii") as handle:
            packets = [
                (None, bytes.fromhex(line.strip()))
                for line in handle
                if line.strip() and not line.startswith("#")
            ]
    else:
        packets = []
        for _, _, link, frame in pcap_packets(arguments.capture):
            parsed = udp_payload(link, frame)
            if parsed is None:
                continue
            _, dst, body = parsed
            domain = (dst - 7400) // 250 if dst >= 7400 else None
            if arguments.domain is not None and domain != arguments.domain:
                continue
            packets.append((domain, body))

    topics = {}
    for _, body in packets:
        sedp_topics(body, topics)

    frames = []
    rows = 0
    for domain, body in packets:
        carried = False
        # The SEDP announcements go into the frames file too: without
        # them the committed evidence cannot be re-attributed to a topic
        # and the decode would be back to counting writers.
        announces = {}
        sedp_topics(body, announces)
        for what, writer, sequence, key_hash, payload in user_data(body):
            carried = True
            rows += 1
            head = payload[:4]
            print(
                "\t".join(
                    [
                        "" if domain is None else str(domain),
                        what,
                        topics.get(writer.hex(), ""),
                        writer[12:16].hex(),
                        str(sequence),
                        key_hash.hex(),
                        head[:2].hex(),
                        head[2:4].hex(),
                        payload[4:].hex(),
                    ]
                )
            )
        if carried or announces:
            frames.append(body)

    if arguments.frames:
        with open(arguments.frames, "w", encoding="ascii") as handle:
            for body in frames:
                handle.write(body.hex() + "\n")
    if rows == 0:
        print("extract-rtps: no user DATA in the capture", file=sys.stderr)
        raise SystemExit(1)


if __name__ == "__main__":
    main()
