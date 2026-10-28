#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 ZeroDDS Contributors
#
# Every GIOP `Request` body in a classic-`pcap` capture, as a row.
#
#   extract-giop.py <capture.pcap> [--operation put] [--frames out.frames]
#
# The row is
#
#   stream  operation  request  order  body
#
# `body` is the request's parameters, from the 8-octet-aligned offset
# GIOP 1.2 §15.4.2 puts them at, to the end of the message. That is the
# same octet string a CORBA probe's `--emit payload` prints, because CDR
# alignment inside a GIOP message is counted from the start of the
# message header and 8 divides that offset — so a wire row and an
# in-memory row are directly comparable.
#
# `order` is `le` or `be` out of the message header's flag byte
# (§15.4.1). Nothing in the CDR stream itself carries it, which is why
# it is a column: JacORB's is big-endian on a little-endian host and no
# in-memory probe would ever say so.
#
# ## Reassembly
#
# A GIOP message can span TCP segments and two messages can share one.
# The payloads of each direction of each connection are concatenated in
# sequence-number order and the message boundaries are taken from the
# GIOP header's own length field, not from the segment boundaries. A
# parser that read one message per segment would silently drop every
# request after the first in a coalesced write.
import argparse
import struct
import sys


def pcap_packets(path):
    with open(path, "rb") as handle:
        header = handle.read(24)
        if len(header) != 24:
            raise SystemExit(f"{path}: not a pcap file")
        magic = header[:4]
        if magic in (b"\xd4\xc3\xb2\xa1", b"\x4d\x3c\xb2\xa1"):
            endian = "<"
        elif magic in (b"\xa1\xb2\xc3\xd4", b"\xa1\xb2\x3c\x4d"):
            endian = ">"
        else:
            raise SystemExit(f"{path}: not a classic pcap — capture with `-F pcap`")
        link = struct.unpack(endian + "I", header[20:24])[0]
        while True:
            record = handle.read(16)
            if len(record) < 16:
                return
            _, _, captured, original = struct.unpack(endian + "IIII", record)
            data = handle.read(captured)
            if len(data) < captured:
                return
            if captured < original:
                raise SystemExit(f"{path}: packet truncated — capture with `-s 0`")
            yield link, data


def tcp_payload(link, frame):
    if link == 1:
        if len(frame) < 14:
            return None
        rest, offset = struct.unpack(">H", frame[12:14])[0], 14
    elif link == 113:
        if len(frame) < 16:
            return None
        rest, offset = struct.unpack(">H", frame[14:16])[0], 16
    elif link == 276:
        if len(frame) < 20:
            return None
        rest, offset = struct.unpack(">H", frame[0:2])[0], 20
    elif link == 101:
        rest, offset = 0x0800, 0
    else:
        return None
    if rest != 0x0800 or len(frame) < offset + 20:
        return None
    ihl = (frame[offset] & 0x0F) * 4
    if frame[offset + 9] != 6:
        return None
    total = struct.unpack(">H", frame[offset + 2 : offset + 4])[0]
    source = frame[offset + 12 : offset + 16]
    target = frame[offset + 16 : offset + 20]
    tcp = offset + ihl
    if len(frame) < tcp + 20:
        return None
    sport, dport = struct.unpack(">HH", frame[tcp : tcp + 4])
    sequence = struct.unpack(">I", frame[tcp + 4 : tcp + 8])[0]
    data_offset = (frame[tcp + 12] >> 4) * 4
    body = frame[tcp + data_offset : offset + total]
    if not body:
        return None
    key = (source, sport, target, dport)
    return key, sequence, body


def align(offset, width):
    return offset + (-offset) % width


def requests(stream, wanted_operation):
    """(operation, request id, order, body) for each Request in a stream."""
    offset = 0
    while offset + 12 <= len(stream):
        if stream[offset : offset + 4] != b"GIOP":
            offset += 1
            continue
        flags = stream[offset + 6]
        kind = stream[offset + 7]
        order = "<" if flags & 0x01 else ">"
        size = struct.unpack(order + "I", stream[offset + 8 : offset + 12])[0]
        message = stream[offset : offset + 12 + size]
        offset += 12 + size
        if kind != 0 or len(message) < 12 + size:
            continue
        cursor = 12
        request_id = struct.unpack(order + "I", message[cursor : cursor + 4])[0]
        cursor += 4
        cursor += 4                                    # flags + 3 reserved
        cursor += 2                                    # AddressingDisposition
        cursor = align(cursor, 4)
        key_length = struct.unpack(order + "I", message[cursor : cursor + 4])[0]
        cursor += 4 + key_length
        cursor = align(cursor, 4)
        name_length = struct.unpack(order + "I", message[cursor : cursor + 4])[0]
        cursor += 4
        operation = message[cursor : cursor + max(name_length - 1, 0)].decode(
            "latin1")
        cursor += name_length
        cursor = align(cursor, 4)
        contexts = struct.unpack(order + "I", message[cursor : cursor + 4])[0]
        cursor += 4
        for _ in range(contexts):
            cursor += 4
            length = struct.unpack(order + "I", message[cursor : cursor + 4])[0]
            cursor += 4 + length
            cursor = align(cursor, 4)
        if wanted_operation and operation != wanted_operation:
            continue
        cursor = align(cursor, 8)
        yield operation, request_id, ("le" if flags & 0x01 else "be"), message[cursor:]


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("capture")
    parser.add_argument("--operation", default="put")
    parser.add_argument("--frames", default=None)
    parser.add_argument("--from-frames", action="store_true")
    arguments = parser.parse_args()

    streams = {}
    if arguments.from_frames:
        with open(arguments.capture, encoding="ascii") as handle:
            for index, line in enumerate(handle):
                line = line.strip()
                if line and not line.startswith("#"):
                    streams.setdefault(("frames",), []).append((index, bytes.fromhex(line)))
    else:
        for link, frame in pcap_packets(arguments.capture):
            parsed = tcp_payload(link, frame)
            if parsed is None:
                continue
            key, sequence, body = parsed
            streams.setdefault(key, []).append((sequence, body))

    rows = 0
    frames = []
    for index, (key, pieces) in enumerate(sorted(streams.items(), key=str)):
        # By sequence number and de-duplicated: a retransmission is the
        # same octets and counting it twice would make the alternation
        # check fail on a correct run.
        seen = {}
        for sequence, body in pieces:
            seen.setdefault(sequence, body)
        stream = b"".join(seen[s] for s in sorted(seen))
        if not stream:
            continue
        for operation, request_id, order, body in requests(stream, arguments.operation):
            rows += 1
            print("\t".join([str(index), operation, str(request_id), order, body.hex()]))
        if b"GIOP" in stream:
            frames.append(stream)

    if arguments.frames:
        with open(arguments.frames, "w", encoding="ascii") as handle:
            for stream in frames:
                handle.write(stream.hex() + "\n")
    if rows == 0:
        print("extract-giop: no matching GIOP request in the capture",
              file=sys.stderr)
        raise SystemExit(1)


if __name__ == "__main__":
    main()
