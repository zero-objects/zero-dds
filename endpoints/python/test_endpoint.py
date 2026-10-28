#!/usr/bin/env python
# SPDX-License-Identifier: Apache-2.0
# Python XRCE + serial framing byte-identical to crates/xrce (ADR 0013).
# usage: python test_endpoint.py <golden_dir>
from __future__ import print_function
import sys
import zerodds_wire as zw
import zerodds_endpoint as ze


def sample_bytes():
    w = zw.Writer(zw.LE, zw.XCDR2)
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
    frame = ze.xrce_write_frame(ze.XRCE_SESSION_NOKEY, ze.XRCE_STREAM_BEST_EFFORT, 1,
                                ze.xrce_object_id(0x001, ze.OBJK_DATAWRITER), body)
    if frame != load(d, "golden_xrce_le.bin"):
        print("WRITE_DATA mismatch"); rc = 1
    else:
        print("XRCE WRITE_DATA byte-identical (%d bytes)" % len(frame))
    # serial
    if ze.serial_frame(1, 0, frame) != load(d, "golden_serial_le.bin"):
        print("serial mismatch"); rc = 1
    else:
        print("serial byte-identical (%d bytes)" % len(ze.serial_frame(1, 0, frame)))
    # DATA receive
    b = ze.xrce_read_frame(load(d, "golden_data_le.bin"))
    if b != body:
        print("DATA body mismatch"); rc = 1
    else:
        print("DATA receive: body ok")
    # serial deframe round-trip
    if ze.serial_deframe(ze.serial_frame(1, 0, frame)) != (1, 0, frame):
        print("deframe mismatch"); rc = 1
    else:
        print("serial deframe+crc round-trip ok")
    if ze.fcs16(b"123456789") != 0x906E:
        print("fcs16 check value"); rc = 1
    bad = bytearray(ze.serial_frame(1, 0, b"\x10\x20\x30"))
    bad[-1] ^= 1
    for label, frame_bad in (("wrong frameCheck", bytes(bad)),
                             ("0x7E inside the frame",
                              bytes(bad[:6] + bytearray([0x7E]) + bad[7:]))):
        try:
            ze.serial_deframe(frame_bad)
            print("a %s was taken" % label); rc = 1
        except ValueError:
            print("%s refused" % label)
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
    first = ze.xrce_write_frame(0x80, 0x01, 1, ze.xrce_object_id(0x001, ze.OBJK_DATAWRITER),
                                bytearray([0xAA, 0xBB, 0xCC]))
    second = ze.xrce_write_frame(0x80, 0x01, 2, ze.xrce_object_id(0x001, ze.OBJK_DATAWRITER),
                                 bytearray([0xDD, 0xEE]))
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
        ze.xrce_write_frame(0x80, 0x01, 1, ze.xrce_object_id(0x001, ze.OBJK_DATAWRITER), bytearray(0x10000)); print("writer >0xFFFF not refused"); rc = 1
    except ValueError:
        print("writer >0xFFFF refused")
    # The bound is the body's (BaseObjectRequest + sample), not the field's one
    # off it: a body of exactly 0xFFFF still frames.
    maximal = ze.xrce_write_frame(0x80, 0x01, 1, ze.xrce_object_id(0x001, ze.OBJK_DATAWRITER), bytearray(0xFFFF - ze.XRCE_BASE_SIZE))
    if len(maximal) != 8 + 0xFFFF or (maximal[6] | (maximal[7] << 8)) != 0xFFFF:
        print("writer maximal body wrong"); rc = 1
    else:
        print("writer maximal body framed")
    return rc


if __name__ == "__main__":
    sys.exit(main())
