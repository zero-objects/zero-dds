#!/usr/bin/env python
# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 ZeroDDS Contributors
#
# Byte-identity test for the pure-Python endpoint wire-core (ADR 0013 P3).
# Encodes the fixed SensorReading in LE and BE and compares to the Rust
# goldens; then round-trips. Same test vector as endpoints/c/test/*.
#
# usage: python test_byte_identity.py <golden_le.bin> <golden_be.bin>

from __future__ import print_function

import sys

import zerodds_wire as zw

# The fixed test vector -- MUST match the C test + the Rust golden generator.
SAMPLE = {
    "id": 0xA1B2C3D4,
    "kind": 0x1234,
    "flags": 0x5A,
    "value": 3.5,
    "stamp": 0x0102030405060708,
    "label": u"bay-12",
    "raw": bytearray([0xDE, 0xAD, 0xBE, 0xEF]),
}


def encode(endian):
    w = zw.Writer(endian)
    w.put_u32(SAMPLE["id"])
    w.put_u16(SAMPLE["kind"])
    w.put_u8(SAMPLE["flags"])
    w.put_f32(SAMPLE["value"])
    w.put_u64(SAMPLE["stamp"])
    w.put_string(SAMPLE["label"])
    w.put_seq_u8(SAMPLE["raw"])
    return w.bytes()


def decode(data, endian):
    r = zw.Reader(data, endian)
    return {
        "id": r.get_u32(),
        "kind": r.get_u16(),
        "flags": r.get_u8(),
        "value": r.get_f32(),
        "stamp": r.get_u64(),
        "label": r.get_string(),
        "raw": r.get_seq_u8(),
    }


def check(endian, golden_path, tag):
    f = open(golden_path, "rb")
    golden = f.read()
    f.close()
    out = encode(endian)
    if out != golden:
        print("%s: MISMATCH: py=%s golden=%s"
              % (tag, out.encode("hex") if hasattr(out, "encode") else out.hex(),
                 golden.encode("hex") if hasattr(golden, "encode") else golden.hex()))
        return 1
    print("%s: %d bytes byte-identical to Rust golden" % (tag, len(out)))
    d = decode(out, endian)
    if (d["id"] != SAMPLE["id"] or d["kind"] != SAMPLE["kind"]
            or d["flags"] != SAMPLE["flags"] or d["value"] != SAMPLE["value"]
            or d["stamp"] != SAMPLE["stamp"] or d["label"] != SAMPLE["label"]
            or d["raw"] != SAMPLE["raw"]):
        print("%s: round-trip mismatch" % tag)
        return 1
    print("%s: round-trip decode ok" % tag)
    return 0


def main():
    if len(sys.argv) < 3:
        print("usage: %s <golden_le.bin> <golden_be.bin>" % sys.argv[0])
        return 2
    rc = 0
    rc |= check(zw.LE, sys.argv[1], "LE")
    rc |= check(zw.BE, sys.argv[2], "BE")
    if rc == 0:
        print("ALL OK")
    return rc


if __name__ == "__main__":
    sys.exit(main())
