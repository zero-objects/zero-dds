# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 ZeroDDS Contributors
#
# zerodds_wire -- ZeroDDS endpoint wire-core, pure Python (ADR 0013).
#
# The `wire-core` unit of the native Python endpoint SDK: the full XCDR
# primitive surface, byte-identical to the Rust core's zerodds-cdr. Pure
# stdlib, no C extension. Serialization is explicit-endian, so the output
# does not depend on the host byte order; the WIRE byte order is an explicit
# LE/BE parameter (the XCDR encapsulation flag, DDSI-RTPS 2.5 s10.5).
#
# The encoding is a constructor argument and has no default. XTypes 1.3
# s7.4.1 puts MAXALIGN at eight under XCDR1 and four under XCDR2, so a
# writer that did not know which one it was in would be writing XCDR2 and
# calling it XCDR -- which is what this SDK did until the cap became this
# argument: `_align` read a constant four, and every `long long` in an
# XCDR1 sample landed four octets early with nothing in the interface to
# say so.
#
# Two things are refused rather than corrected, and both were silent
# before: an integer past the wire width it names (`put_u16(70000)` wrote
# `7011`, the low sixteen bits), and a code point past the form's range
# (`put_char` above U+00FF, `put_wchar` above the BMP). Python has no types
# to enforce the argument, so the check here is the only one there is.

from __future__ import print_function

import struct

LE = 0
BE = 1

# The XCDR version the stream is written in. XCDR1 is PLAIN_CDR/PL_CDR,
# MAXALIGN 8; XCDR2 is PLAIN_CDR2/DELIMITED_CDR/PL_CDR2, MAXALIGN 4.
XCDR1 = 1
XCDR2 = 2

# The member header length codes of XTypes 1.3 s7.4.3.4.2 Table 34. The
# code occupies bits 28..30 of the EMHEADER and decides how the member's
# byte length is recovered: LC0..LC3 give it from the code alone, LC4
# reads it from a NEXTINT after the header, LC5..LC7 reuse the body's own
# leading word.
LC0 = 0
LC1 = 1
LC2 = 2
LC3 = 3
LC4 = 4
LC5 = 5
LC6 = 6
LC7 = 7

# PL_CDR1 parameter ids. XTypes 1.3 s7.4.1.2.
PID_LIST_END_ID = 0x3F02
PID_EXTENDED_ID = 0x3F01
PID_SHORT_MAX_ID = 0x3F00

# Bit 14 of a short parameter id is the must-understand flag; bit 30 of
# the eMemberHeader inside an extended header is the member's own.
_PID_FLAG_MUST_UNDERSTAND = 0x4000
_EM_FLAG_MUST_UNDERSTAND = 0x40000000


def _max_align_of(encoding):
    if encoding == XCDR1:
        return 8
    if encoding == XCDR2:
        return 4
    raise ValueError("Writer takes an endian and an encoding (XCDR1 or XCDR2)")


def padding_for(pos, align):
    mask = align - 1
    return (align - (pos & mask)) & mask


def _check_uint(v, bits, call):
    if isinstance(v, bool) or not isinstance(v, int):
        raise ValueError("%s: %r is not an integer" % (call, v))
    if v < 0 or v >> bits != 0:
        raise ValueError("%s: %r is not a %d-bit value" % (call, v, bits))


def _fixed_octets(value, digits, scale):
    """The packed BCD of an IDL fixed<digits,scale>. CDR s9.3.2.8."""
    if digits < 1 or digits > 31:
        raise ValueError("fixed<%r,%r>: a digit count is 1..31" % (digits, scale))
    if scale < 0 or scale > digits:
        raise ValueError("fixed<%d,%r>: a scale is 0..digits" % (digits, scale))
    text = value
    negative = False
    if text[:1] in ("+", "-"):
        negative = text[:1] == "-"
        text = text[1:]
    if "." in text:
        integral, fraction = text.split(".", 1)
    else:
        integral, fraction = text, ""
    if integral == "" and fraction == "":
        raise ValueError("fixed<%d,%d>: %r has no digits" % (digits, scale, value))
    if not all(c in "0123456789" for c in integral + fraction):
        raise ValueError("fixed<%d,%d>: %r is not a decimal" % (digits, scale, value))
    # Leading zeros carry no value: `000000.001` is one significant
    # integer digit, not six.
    integral = integral.lstrip("0")
    if len(integral) > digits - scale:
        raise ValueError(
            "fixed<%d,%d>: %r needs %d integer digits and the type has %d"
            % (digits, scale, value, len(integral), digits - scale))
    # A trailing zero in the fraction is padding either way.
    while len(fraction) > scale and fraction.endswith("0"):
        fraction = fraction[:-1]
    if len(fraction) > scale:
        raise ValueError(
            "fixed<%d,%d>: %r needs %d fraction digits and the type has %d"
            % (digits, scale, value, len(fraction), scale))
    nibbles = []
    if (digits + 1) % 2 == 1:
        nibbles.append(0)
    nibbles.extend([0] * (digits - scale - len(integral)))
    nibbles.extend(ord(c) - ord("0") for c in integral)
    nibbles.extend(ord(c) - ord("0") for c in fraction)
    nibbles.extend([0] * (scale - len(fraction)))
    # A negative zero goes out positive: there is one zero to write.
    if negative and any(nibbles):
        nibbles.append(0x0D)
    else:
        nibbles.append(0x0C)
    out = bytearray()
    for i in range(0, len(nibbles) - 1, 2):
        out.append((nibbles[i] << 4) | nibbles[i + 1])
    return bytes(out)


class Writer(object):
    """Growing XCDR write buffer with an explicit byte order and encoding."""

    def __init__(self, endian, encoding):
        self.buf = bytearray()
        self.endian = endian
        self.max_align = _max_align_of(encoding)
        # The position alignment is measured from. Zero for the stream,
        # and moved to the start of a PL_CDR1 parameter's value for the
        # length of that parameter -- s7.4.1.2 aligns a parameter's value
        # against its own block, not against the stream.
        self._origin = 0

    def _align(self, align):
        eff = align if align <= self.max_align else self.max_align
        pad = padding_for(len(self.buf) - self._origin, eff)
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

    def _patch_u32(self, at, v):
        le = struct.pack("<I", v & 0xFFFFFFFF)
        if self.endian == BE:
            le = le[::-1]
        self.buf[at:at + 4] = bytearray(le)

    def _patch_u16(self, at, v):
        le = struct.pack("<H", v & 0xFFFF)
        if self.endian == BE:
            le = le[::-1]
        self.buf[at:at + 2] = bytearray(le)

    def put_u8(self, v):
        _check_uint(v, 8, "put_u8")
        self.buf.append(v)

    def put_u16(self, v):
        _check_uint(v, 16, "put_u16")
        self._put_uint(2, struct.pack("<H", v))

    def put_u32(self, v):
        _check_uint(v, 32, "put_u32")
        self._put_uint(4, struct.pack("<I", v))

    def put_u64(self, v):
        _check_uint(v, 64, "put_u64")
        self._put_uint(8, struct.pack("<Q", v))

    def put_bool(self, v):
        # 0 or 1, never any other pattern. XTypes 1.3 s7.4.1.1.1 Table 30.
        self.buf.append(1 if v else 0)

    def put_f32(self, v):
        # IEEE-754 bit pattern via the u32 path, like the Rust f32::to_bits.
        self._put_uint(4, struct.pack("<f", v))

    def put_f64(self, v):
        self._put_uint(8, struct.pack("<d", v))

    def put_char(self, code_point):
        # A code point above U+00FF is refused and not truncated: U+0161's
        # low octet is 0x61, which is `a` -- a truncating writer produces a
        # frame of the right length holding a plausible, wrong character.
        _check_uint(code_point, 32, "put_char")
        if code_point > 0xFF:
            raise ValueError(
                "expected a character below U+0100, found U+%04X" % code_point)
        self.buf.append(code_point)

    def put_wchar(self, code_point):
        # One two-octet code unit, 2-aligned, in the stream's byte order.
        # No GIOP length octet, no byte-order mark. A code point above
        # U+FFFF is refused: a four-octet form would zero-extend a code
        # *unit*, not a code point.
        _check_uint(code_point, 32, "put_wchar")
        if code_point > 0xFFFF:
            raise ValueError(
                "expected a BMP character, found U+%04X" % code_point)
        self._put_uint(2, struct.pack("<H", code_point))

    def put_string(self, s):
        raw = s.encode("utf-8") if isinstance(s, type(u"")) else bytes(s)
        self.put_u32(len(raw) + 1)
        self.put_bytes(raw)
        self.buf.append(0)

    def put_wstring(self, units):
        # The length word counts octets and not code units, there is no
        # terminator and no byte-order mark, and each unit goes out two
        # octets wide in the stream's own byte order.
        for unit in units:
            _check_uint(unit, 16, "put_wstring")
        self.put_u32(len(units) * 2)
        for unit in units:
            self._put_uint(2, struct.pack("<H", unit))

    def put_seq_u8(self, data):
        self.put_u32(len(data))
        self.put_bytes(bytes(bytearray(data)))

    def put_bitset(self, bits, anchor):
        # The anchor word: the holder the declared bits pack into, written
        # at the anchor's own width and alignment. XTypes 1.3 s7.3.1.4.
        _check_uint(bits, 64, "put_bitset")
        if anchor == 1:
            self.put_u8(bits)
        elif anchor == 2:
            self.put_u16(bits)
        elif anchor == 4:
            self.put_u32(bits)
        elif anchor == 8:
            self.put_u64(bits)
        else:
            raise ValueError(
                "a bitset anchor is one, two, four or eight octets, not %r"
                % (anchor,))

    def put_fixed(self, value, digits, scale):
        # Packed BCD, alignment one, byte-order independent. CDR s9.3.2.8.
        self.put_bytes(_fixed_octets(value, digits, scale))

    # --- DHEADER (XCDR2 delimited): @appendable/@mutable struct + collection
    #     with a non-primitive element. u32 (4-aligned) = body byte length. ---
    def dheader_begin(self):
        self.put_u32(0)
        return len(self.buf)  # body starts here

    def dheader_end(self, body_start):
        # The DHEADER itself is not counted -- s7.4.3.5 measures the
        # content that follows it.
        self._patch_u32(body_start - 4, len(self.buf) - body_start)

    # --- EMHEADER (XCDR2 @mutable): u32 = (M<<31)+(LC<<28)+member_id. ---
    def emheader_begin(self, member_id, must_understand, length_code):
        # A wider id is refused, not masked: the masked value is a
        # DIFFERENT member's id.
        _check_uint(member_id, 32, "emheader_begin")
        if member_id > 0x0FFFFFFF:
            raise ValueError(
                "member id %d does not fit the 28 bits of an EMHEADER"
                % member_id)
        if length_code < LC0 or length_code > LC7:
            raise ValueError(
                "length code %r is not one of LC0..LC7" % (length_code,))
        header = (length_code << 28) | member_id
        if must_understand:
            header |= 0x80000000
        self.put_u32(header)
        if length_code == LC4:
            self.put_u32(0)  # the NEXTINT slot
        return len(self.buf)

    def emheader_end(self, body_start, length_code):
        # The length code comes back rather than being remembered, so a
        # caller that paired an LC4 begin with a differently-coded end gets
        # a refusal instead of a NEXTINT patched over the member's body.
        written = len(self.buf) - body_start
        if length_code == LC4:
            self._patch_u32(body_start - 4, written)
        elif length_code in (LC0, LC1, LC2, LC3):
            want = 1 << length_code
            if written != want:
                raise ValueError(
                    "LC%d frames %d octets and the member wrote %d"
                    % (length_code, want, written))
        else:
            if written < 4:
                raise ValueError(
                    "LC%d takes its length from the body's first four octets "
                    "and the member wrote %d" % (length_code, written))

    # --- the PL_CDR1 parameter list (XTypes 1.3 s7.4.1.2) ---
    def pid_begin(self, pid, must_understand):
        # Short form: u16 id, u16 byte length, both in the stream's order.
        # The value aligns against its own block, so the origin moves for
        # the length of the parameter and pid_end restores it.
        _check_uint(pid, 16, "pid_begin")
        if pid > PID_SHORT_MAX_ID:
            raise ValueError(
                "member id %d needs the extended parameter header" % pid)
        self._align(4)
        header = pid
        if must_understand:
            header |= _PID_FLAG_MUST_UNDERSTAND
        self._put_uint(2, struct.pack("<H", header))
        at = len(self.buf)
        self.buf.extend(b"\x00\x00")
        saved = self._origin
        self._origin = len(self.buf)
        return (at, False, saved)

    def pid_extended_begin(self, member_id, must_understand):
        # The twelve-octet form for an id above 0x3F00 or a long value:
        # [PID_EXTENDED | must-understand : u16][8 : u16]
        # [eMemberHeader : u32][length : u32]. The flag on the marker is
        # the framing's, whatever the member says; the member's own flag is
        # bit 30 of the eMemberHeader.
        _check_uint(member_id, 32, "pid_extended_begin")
        if member_id > 0x0FFFFFFF:
            raise ValueError(
                "member id %d does not fit the 28 bits of an eMemberHeader"
                % member_id)
        self._align(4)
        marker = PID_EXTENDED_ID | _PID_FLAG_MUST_UNDERSTAND
        self._put_uint(2, struct.pack("<H", marker))
        self._put_uint(2, struct.pack("<H", 8))
        header = member_id
        if must_understand:
            header |= _EM_FLAG_MUST_UNDERSTAND
        self._put_uint(4, struct.pack("<I", header))
        at = len(self.buf)
        self.buf.extend(b"\x00\x00\x00\x00")
        saved = self._origin
        self._origin = len(self.buf)
        return (at, True, saved)

    def pid_end(self, header):
        # The declared length counts the value's octets AND the padding
        # that brings the next header back to a four-octet boundary: a
        # reader skips `length` from the end of the header and has to land
        # on the next parameter.
        at, extended, saved_origin = header
        width = 4 if extended else 2
        length = len(self.buf) - at - width
        past = (len(self.buf) - saved_origin) % 4
        if past:
            length += 4 - past
        if extended:
            self._patch_u32(at, length)
        else:
            if length > 0xFFFF:
                raise ValueError(
                    "a short parameter header describes at most 65535 octets, "
                    "not %d" % length)
            self._patch_u16(at, length)
        self._origin = saved_origin
        self._align(4)

    def pid_list_end(self):
        # The sentinel, with the must-understand flag set -- 0x7F02 on the
        # wire, which is what Cyclone and RTI write.
        self._align(4)
        self._put_uint(2, struct.pack(
            "<H", PID_LIST_END_ID | _PID_FLAG_MUST_UNDERSTAND))
        self._put_uint(2, struct.pack("<H", 0))

    def bytes(self):
        return bytes(self.buf)


class Reader(object):
    """XCDR read cursor over a bytes buffer."""

    def __init__(self, data, endian, encoding):
        self.buf = bytearray(data)
        self.pos = 0
        self.endian = endian
        self.max_align = _max_align_of(encoding)
        # Mirrors the writer's: a PL_CDR1 parameter's value aligns against
        # its own block.
        self._origin = 0

    def _align(self, align):
        eff = align if align <= self.max_align else self.max_align
        pad = padding_for(self.pos - self._origin, eff)
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

    def get_char(self):
        return self.get_u8()

    def get_wchar(self):
        return self.get_u16()

    def get_string(self):
        n = self.get_u32()
        if n == 0:
            raise ValueError("string length must include the NUL")
        raw = self.get_bytes(n)
        if bytearray(raw)[n - 1] != 0:
            raise ValueError("missing NUL terminator")
        return raw[:n - 1].decode("utf-8")

    def get_wstring(self):
        # The length word counts octets, so the unit count is half of it.
        octets = self.get_u32()
        return [self.get_u16() for _ in range(octets // 2)]

    def get_seq_u8(self):
        n = self.get_u32()
        return bytearray(self.get_bytes(n))

    def get_bitset(self, anchor):
        if anchor == 1:
            return self.get_u8()
        if anchor == 2:
            return self.get_u16()
        if anchor == 4:
            return self.get_u32()
        if anchor == 8:
            return self.get_u64()
        raise ValueError(
            "a bitset anchor is one, two, four or eight octets, not %r"
            % (anchor,))

    def get_fixed(self, digits, scale):
        # Back into the decimal it was written from: a leading `-` when
        # the sign nibble is 0xD, `digits - scale` integer digits, and a
        # `.` with `scale` fraction digits where the scale is not zero.
        octets = (digits + 2) // 2
        nibbles = []
        for _ in range(octets):
            b = self.get_u8()
            nibbles.append(b >> 4)
            nibbles.append(b & 0x0F)
        sign = nibbles[-1]
        digits_only = nibbles[len(nibbles) - 1 - digits:len(nibbles) - 1]
        out = "-" if sign == 0x0D else ""
        for i, n in enumerate(digits_only):
            if scale != 0 and i == digits - scale:
                out += "."
            out += chr(ord("0") + n)
        return out

    def dheader_read(self):
        return self.get_u32()

    def emheader_read(self):
        # Returns (member_id, must_understand, length_code, extent), the
        # extent measured from the position this call leaves the cursor
        # at. LC0..LC3 take it from the code; LC4 consumes the NEXTINT;
        # LC5..LC7 peek the body's own leading word without consuming it.
        em = self.get_u32()
        member_id = em & 0x0FFFFFFF
        must_understand = (em & 0x80000000) != 0
        length_code = (em >> 28) & 0x7
        if length_code <= LC3:
            extent = 1 << length_code
        elif length_code == LC4:
            extent = self.get_u32()
        else:
            at = self.pos
            word = self.get_u32()
            self.pos = at
            if length_code == LC5:
                extent = 4 + word
            elif length_code == LC6:
                extent = 4 + 4 * word
            else:
                extent = 4 + 8 * word
        return (member_id, must_understand, length_code, extent)

    def pid_read(self):
        # Returns (member_id, must_understand, length, list_end). The
        # extended form is recognised by the marker PID_EXTENDED, and the
        # must-understand answer then comes from bit 30 of the
        # eMemberHeader and never from the marker.
        raw = self.get_u16()
        short = raw & 0x3FFF
        if short == PID_LIST_END_ID:
            self.get_u16()
            return (0, False, 0, True)
        if short == PID_EXTENDED_ID:
            self.get_u16()  # the slength, always 8
            header = self.get_u32()
            length = self.get_u32()
            self._origin = self.pos
            return (header & 0x0FFFFFFF,
                    (header & _EM_FLAG_MUST_UNDERSTAND) != 0, length, False)
        length = self.get_u16()
        self._origin = self.pos
        return (short, (raw & _PID_FLAG_MUST_UNDERSTAND) != 0, length, False)

    def pid_skip(self, body_start, length):
        # Past the value and the padding, back on the stream's alignment.
        self.pos = body_start + length
        self._origin = 0
        while self.pos % 4 != 0:
            self.pos += 1
