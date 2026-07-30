#!/usr/bin/env python
# SPDX-License-Identifier: Apache-2.0
# Python XRCE + serial framing byte-identical to crates/xrce (ADR 0013).
# usage: python test_endpoint.py <golden_dir>
from __future__ import print_function
import sys
import zerodds_wire as zw
import zerodds_endpoint as ze


def sample_bytes():
    w = zw.Writer(zw.LE)
    w.put_u32(0xA1B2C3D4); w.put_u16(0x1234); w.put_u8(0x5A); w.put_f32(3.5)
    w.put_u64(0x0102030405060708); w.put_string(u"bay-12")
    w.put_seq_u8(bytearray([0xDE, 0xAD, 0xBE, 0xEF]))
    return w.bytes()


def load(d, n):
    f = open(d + "/" + n, "rb"); b = f.read(); f.close(); return b


def main():
    d = sys.argv[1] if len(sys.argv) > 1 else "."
    rc = 0
    body = sample_bytes()
    # WRITE_DATA
    frame = ze.xrce_write_frame(ze.XRCE_SESSION_NOKEY, ze.XRCE_STREAM_BEST_EFFORT, 1, body)
    if frame != load(d, "golden_xrce_le.bin"):
        print("WRITE_DATA mismatch"); rc = 1
    else:
        print("XRCE WRITE_DATA byte-identical (%d bytes)" % len(frame))
    # serial
    if ze.serial_frame(frame) != load(d, "golden_serial_le.bin"):
        print("serial mismatch"); rc = 1
    else:
        print("serial byte-identical (%d bytes)" % len(ze.serial_frame(frame)))
    # DATA receive
    b = ze.xrce_read_frame(load(d, "golden_data_le.bin"))
    if b != body:
        print("DATA body mismatch"); rc = 1
    else:
        print("DATA receive: body ok")
    # serial deframe round-trip
    if ze.serial_deframe(ze.serial_frame(frame)) != frame:
        print("deframe mismatch"); rc = 1
    else:
        print("serial deframe+crc round-trip ok")
    # reliable
    first, last, stream = ze.heartbeat_read(load(d, "golden_heartbeat_le.bin"))
    if (first, last, stream) != (1, 3, 0x80):
        print("heartbeat vals %r" % ((first, last, stream),)); rc = 1
    else:
        print("HEARTBEAT parsed: first=%d last=%d" % (first, last))
    an = ze.acknack_frame(0x80, 0x00, 1, 1, 0, 0, 0x80)
    if an != load(d, "golden_acknack_le.bin"):
        print("ACKNACK mismatch"); rc = 1
    else:
        print("ACKNACK byte-identical (%d bytes)" % len(an))
    rc |= negative_frame_vectors()
    if rc == 0:
        print("ALL OK")
    return rc


def negative_frame_vectors():
    """Self-contained (no goldens): reader bounds the body to the declared
    submessage_length and rejects malformed frames; writer refuses oversize."""
    rc = 0
    first = ze.xrce_write_frame(0x80, 0x01, 1, bytearray([0xAA, 0xBB, 0xCC]))
    second = ze.xrce_write_frame(0x80, 0x01, 2, bytearray([0xDD, 0xEE]))
    if ze.xrce_read_frame(bytes(first) + bytes(second)) != bytes(bytearray([0xAA, 0xBB, 0xCC])):
        print("appended-submessage leak"); rc = 1
    else:
        print("appended submessage bounded out")
    overlong = bytearray(first); overlong[6] = 0xFF; overlong[7] = 0xFF
    try:
        ze.xrce_read_frame(bytes(overlong)); print("over-long not rejected"); rc = 1
    except ValueError:
        print("over-long length rejected")
    try:
        ze.xrce_read_frame(b"\x80\x01\x00\x00\x07"); print("truncated not rejected"); rc = 1
    except ValueError:
        print("truncated header rejected")
    try:
        ze.xrce_write_frame(0x80, 0x01, 1, bytearray(0x10000)); print("writer >0xFFFF not refused"); rc = 1
    except ValueError:
        print("writer >0xFFFF refused")
    return rc


if __name__ == "__main__":
    sys.exit(main())
