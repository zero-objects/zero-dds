#!/usr/bin/env python
# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 ZeroDDS Contributors
#
# Byte-identity test for the Python EMHEADER / @mutable path (ADR 0013).
# Same vector as endpoints/golden-gen encode_mutable.
#
# usage: python test_mutable.py <golden_mutable_le.bin> <golden_mutable_be.bin>

from __future__ import print_function

import sys

import zerodds_wire as zw


def encode(endian):
    w = zw.Writer(endian)
    bs = w.dheader_begin()                 # @mutable struct DHEADER
    e = w.emheader_begin(10, False); w.put_u32(0xDEADBEEF); w.emheader_end(e)
    e = w.emheader_begin(20, False); w.put_string(u"mut"); w.emheader_end(e)
    e = w.emheader_begin(30, False); w.put_u16(0x0777); w.emheader_end(e)
    w.dheader_end(bs)
    return w.bytes()


def decode(data, endian):
    r = zw.Reader(data, endian)
    dh = r.dheader_read()
    start = r.pos
    out = {}
    while r.pos - start < dh:
        member_id, _mu, nextint = r.emheader_read()
        if member_id == 10:
            out["x"] = r.get_u32()
        elif member_id == 20:
            out["s"] = r.get_string()
        elif member_id == 30:
            out["k"] = r.get_u16()
        else:
            r.get_bytes(nextint)  # skip unknown
    return out


def check(endian, path, tag):
    f = open(path, "rb")
    golden = f.read()
    f.close()
    out = encode(endian)
    if out != golden:
        print("%s: MISMATCH len py=%d golden=%d" % (tag, len(out), len(golden)))
        return 1
    print("%s: %d bytes byte-identical to Rust golden (EMHEADER/@mutable)"
          % (tag, len(out)))
    d = decode(out, endian)
    if d.get("x") != 0xDEADBEEF or d.get("s") != u"mut" or d.get("k") != 0x0777:
        print("%s: round-trip mismatch %r" % (tag, d))
        return 1
    print("%s: round-trip decode ok" % tag)
    return 0


def main():
    if len(sys.argv) < 3:
        print("usage: %s <mutable_le> <mutable_be>" % sys.argv[0])
        return 2
    rc = check(zw.LE, sys.argv[1], "LE") | check(zw.BE, sys.argv[2], "BE")
    if rc == 0:
        print("ALL OK")
    return rc


if __name__ == "__main__":
    sys.exit(main())
