# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 ZeroDDS Contributors
#
# zerodds_wire -- ZeroDDS endpoint wire-core, pure Python (ADR 0013).
#
# The `wire-core` unit of the native Python endpoint SDK: XCDR2 primitives
# byte-identical to the Rust core's zerodds-cdr. Pure stdlib, no C extension,
# conservative style (Python 2.7 / 3.x). Serialization is explicit-endian, so
# the output does not depend on the host byte order; the WIRE byte order is an
# explicit LE/BE parameter (the XCDR encapsulation flag, DDSI-RTPS 2.5 s10.5).
#
# XCDR2 alignment (OMG XTypes 1.3 s7.4.1.1.1): stream-relative, capped at 4.

from __future__ import print_function

import struct

LE = 0
BE = 1

_XCDR2_MAX_ALIGN = 4


def padding_for(pos, align):
    mask = align - 1
    return (align - (pos & mask)) & mask


class Writer(object):
    """Growing XCDR2 write buffer with an explicit wire byte order."""

    def __init__(self, endian):
        self.buf = bytearray()
        self.endian = endian
        self.max_align = _XCDR2_MAX_ALIGN

    def _align(self, align):
        eff = align if align <= self.max_align else self.max_align
        pad = padding_for(len(self.buf), eff)
        if pad:
            self.buf.extend(b"\x00" * pad)

    def put_bytes(self, data):
        self.buf.extend(data)

    def _put_uint(self, align, le_bytes):
        self._align(align)
        if self.endian == BE:
            self.buf.extend(le_bytes[::-1])
        else:
            self.buf.extend(le_bytes)

    def put_u8(self, v):
        self.buf.append(v & 0xFF)

    def put_u16(self, v):
        self._put_uint(2, struct.pack("<H", v & 0xFFFF))

    def put_u32(self, v):
        self._put_uint(4, struct.pack("<I", v & 0xFFFFFFFF))

    def put_u64(self, v):
        self._put_uint(8, struct.pack("<Q", v & 0xFFFFFFFFFFFFFFFF))

    def put_bool(self, v):
        self.put_u8(1 if v else 0)

    def put_f32(self, v):
        # IEEE-754 bit pattern via the u32 path, like the Rust f32::to_bits.
        self._put_uint(4, struct.pack("<f", v))

    def put_f64(self, v):
        self._put_uint(8, struct.pack("<d", v))

    def put_string(self, s):
        raw = s.encode("utf-8") if isinstance(s, type(u"")) else bytes(s)
        self.put_u32(len(raw) + 1)
        self.put_bytes(raw)
        self.put_u8(0)

    def put_seq_u8(self, data):
        self.put_u32(len(data))
        self.put_bytes(bytes(bytearray(data)))

    # --- DHEADER (XCDR2 delimited): @appendable/@mutable struct + collection
    #     with a non-primitive element. u32 (4-aligned) = body byte length. ---
    def dheader_begin(self):
        self._align(4)
        self.buf.extend(b"\x00\x00\x00\x00")
        return len(self.buf)  # body starts here

    def dheader_end(self, body_start):
        length = len(self.buf) - body_start
        le = struct.pack("<I", length & 0xFFFFFFFF)
        if self.endian == BE:
            le = le[::-1]
        self.buf[body_start - 4:body_start] = bytearray(le)

    # --- EMHEADER (XCDR2 @mutable, LC4): u32 = (M<<31)+(LC4<<28)+member_id,
    #     followed by NEXTINT (u32 = body length) + body. ---
    def emheader_begin(self, member_id, must_understand):
        em = (0x80000000 if must_understand else 0) + (4 << 28) \
            + (member_id & 0x0FFFFFFF)
        self.put_u32(em)
        return self.dheader_begin()  # NEXTINT slot + body start

    def emheader_end(self, body_start):
        self.dheader_end(body_start)

    def bytes(self):
        return bytes(self.buf)


class Reader(object):
    """XCDR2 read cursor over a bytes buffer."""

    def __init__(self, data, endian):
        self.buf = bytearray(data)
        self.pos = 0
        self.endian = endian
        self.max_align = _XCDR2_MAX_ALIGN

    def _align(self, align):
        eff = align if align <= self.max_align else self.max_align
        pad = padding_for(self.pos, eff)
        if self.pos + pad > len(self.buf):
            raise ValueError("overrun on align")
        self.pos += pad

    def get_bytes(self, n):
        if self.pos + n > len(self.buf):
            raise ValueError("overrun")
        out = bytes(self.buf[self.pos:self.pos + n])
        self.pos += n
        return out

    def _get_uint(self, align, n):
        self._align(align)
        raw = self.get_bytes(n)
        if self.endian == BE:
            raw = raw[::-1]
        return raw

    def get_u8(self):
        return bytearray(self.get_bytes(1))[0]

    def get_u16(self):
        return struct.unpack("<H", self._get_uint(2, 2))[0]

    def get_u32(self):
        return struct.unpack("<I", self._get_uint(4, 4))[0]

    def get_u64(self):
        return struct.unpack("<Q", self._get_uint(8, 8))[0]

    def get_bool(self):
        return self.get_u8() != 0

    def get_f32(self):
        return struct.unpack("<f", self._get_uint(4, 4))[0]

    def get_f64(self):
        return struct.unpack("<d", self._get_uint(8, 8))[0]

    def get_string(self):
        n = self.get_u32()
        if n == 0:
            raise ValueError("string length must include the NUL")
        raw = self.get_bytes(n)
        if bytearray(raw)[n - 1] != 0:
            raise ValueError("missing NUL terminator")
        return raw[:n - 1].decode("utf-8")

    def get_seq_u8(self):
        n = self.get_u32()
        return bytearray(self.get_bytes(n))

    def dheader_read(self):
        return self.get_u32()

    def emheader_read(self):
        em = self.get_u32()
        member_id = em & 0x0FFFFFFF
        must_understand = (em & 0x80000000) != 0
        nextint = self.get_u32()
        return (member_id, must_understand, nextint)
