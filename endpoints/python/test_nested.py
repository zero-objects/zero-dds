#!/usr/bin/env python
# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 ZeroDDS Contributors
#
# Byte-identity test for the Python DHEADER path: @appendable Outer with a
# nested @appendable Inner + sequence<Inner> + string (ADR 0013). Same vector
# as endpoints/golden-gen encode_nested.
#
# usage: python test_nested.py <golden_nested_le.bin> <golden_nested_be.bin>

from __future__ import print_function

import sys

import zerodds_wire as zw

MANY = [(0xAAAA, 0xBBBBCCCC), (0xDDDD, 0xEEEEFFFF)]


def encode_inner(w, a, b):
    bs = w.dheader_begin()
    w.put_u16(a)
    w.put_u32(b)
    w.dheader_end(bs)


def encode(endian):
    w = zw.Writer(endian)
    bs = w.dheader_begin()               # outer @appendable
    w.put_u32(0xCAFEBABE)
    encode_inner(w, 0x1111, 0x22223333)  # nested
    cbs = w.dheader_begin()              # sequence<Inner> collection DHEADER
    w.put_u32(len(MANY))
    for a, b in MANY:
        encode_inner(w, a, b)
    w.dheader_end(cbs)
    w.put_string(u"nested")
    w.dheader_end(bs)
    return w.bytes()


def decode_inner(r):
    r.dheader_read()
    a = r.get_u16()
    b = r.get_u32()
    return (a, b)


def decode(data, endian):
    r = zw.Reader(data, endian)
    r.dheader_read()
    d = {"id": r.get_u32(), "one": decode_inner(r)}
    r.dheader_read()
    n = r.get_u32()
    d["many"] = [decode_inner(r) for _ in range(n)]
    d["label"] = r.get_string()
    return d


def check(endian, path, tag):
    f = open(path, "rb")
    golden = f.read()
    f.close()
    out = encode(endian)
    if out != golden:
        print("%s: MISMATCH len py=%d golden=%d" % (tag, len(out), len(golden)))
        return 1
    print("%s: %d bytes byte-identical to Rust golden (DHEADER/nested/seq)"
          % (tag, len(out)))
    d = decode(out, endian)
    if (d["id"] != 0xCAFEBABE or d["one"] != (0x1111, 0x22223333)
            or d["many"] != MANY or d["label"] != u"nested"):
        print("%s: round-trip mismatch" % tag)
        return 1
    print("%s: round-trip decode ok" % tag)
    return 0


def main():
    if len(sys.argv) < 3:
        print("usage: %s <nested_le> <nested_be>" % sys.argv[0])
        return 2
    rc = check(zw.LE, sys.argv[1], "LE") | check(zw.BE, sys.argv[2], "BE")
    if rc == 0:
        print("ALL OK")
    return rc


if __name__ == "__main__":
    sys.exit(main())
