# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 ZeroDDS Contributors
#
# The wire-core operations this SDK grew so that a caller can write every
# construct XTypes 1.3 defines: the 64-bit float, the XCDR1 alignment cap,
# the delimiter, the member header in all eight length codes, the PL_CDR1
# parameter list, the wide character and string, the packed decimal and the
# bitset anchor.
#
# The expected octets are not this file's opinion. Every multi-member case
# is the byte string tests/interop-results/<case>/zerodds-rust/
# <encoding>.<endian>.bin already holds, named in the test, so a change
# here that agreed with itself and not with the reference column fails.
#
# Each operation is exercised at the ends of its range and not only in the
# middle, because that is where the last four defects in these bindings
# were.

from __future__ import print_function

import sys

import zerodds_wire as z

failures = []


def check(name, got, want):
    if got == want:
        print("ok %s" % name)
    else:
        failures.append(name)
        print("FAIL %s: got %s want %s" % (name, got, want))


def check_true(name, value):
    check(name, bool(value), True)


def refused(fn):
    try:
        fn()
        return False
    except ValueError:
        return True


def hx(w):
    return bytes(w.buf).hex()


# --- the alignment cap the encoding sets ---------------------------------

# XTypes 1.3 s7.4.1. One octet, then a `long long`: under XCDR1 the 64-bit
# value aligns to 8 and takes seven octets of padding; under XCDR2 the cap
# is 4 and it takes three.
w = z.Writer(z.LE, z.XCDR1)
w.put_u8(0xAB)
w.put_u64(0x0102030405060708)
check("xcdr1 aligns eight", hx(w), "ab000000000000000807060504030201")
w = z.Writer(z.LE, z.XCDR2)
w.put_u8(0xAB)
w.put_u64(0x0102030405060708)
check("xcdr2 aligns four", hx(w), "ab0000000807060504030201")
# The cap is the encoding's and not a constant.
check("the encoding decides max_align",
      (z.Writer(z.BE, z.XCDR1).max_align, z.Writer(z.BE, z.XCDR2).max_align),
      (8, 4))

# --- the 64-bit float ----------------------------------------------------

# Pi as tests/interop-corpus/struct-mutable carries it, and the two ends of
# the exponent range either side of it. IEEE 754 gives negative zero its
# own bit pattern and s7.4.1.1.1 puts the pattern on the wire.
for value, want in [
    (3.141592653589793, "182d4454fb210940"),
    (0.0, "0000000000000000"),
    (-0.0, "0000000000000080"),
    (1.7976931348623157e308, "ffffffffffffef7f"),
    (5e-324, "0100000000000000"),
]:
    w = z.Writer(z.LE, z.XCDR2)
    w.put_f64(value)
    check("f64 %r" % value, hx(w), want)

# --- DHEADER -------------------------------------------------------------

# XTypes 1.3 s7.4.3.5. Three octets of content, so the DHEADER reads 3 and
# the four octets it occupies are not in it.
w = z.Writer(z.LE, z.XCDR2)
mark = w.dheader_begin()
w.put_u8(1)
w.put_u8(2)
w.put_u8(3)
w.dheader_end(mark)
check("dheader counts the body and not itself", hx(w), "03000000010203")
w = z.Writer(z.BE, z.XCDR2)
w.dheader_end(w.dheader_begin())
check("an empty dheader body is zero", hx(w), "00000000")

# --- the member header, in every length code ----------------------------

# tests/interop-results/struct-mutable/zerodds-rust/plain-cdr2.le.bin --
# @mutable struct Sample { long a; double b; short c; } with a = -1234567,
# b = pi, c = -2. LC2, LC3 and LC1: the length is in the code and no
# NEXTINT is written for any of them.
w = z.Writer(z.LE, z.XCDR2)
d = w.dheader_begin()
m = w.emheader_begin(0, False, z.LC2)
w.put_u32(0xFFED2979)
w.emheader_end(m, z.LC2)
m = w.emheader_begin(1, False, z.LC3)
w.put_f64(3.141592653589793)
w.emheader_end(m, z.LC3)
m = w.emheader_begin(2, False, z.LC1)
w.put_u16(0xFFFE)
w.emheader_end(m, z.LC1)
w.dheader_end(d)
check("struct-mutable xcdr2 matches the rust column", hx(w),
      "1a000000" + "00000020" + "7929edff" +
      "01000030" + "182d4454fb210940" + "02000010" + "feff")

# s7.4.3.4.2. LC4 reserves a NEXTINT of its own; LC5 does not, because the
# member's leading word already is the length.
w = z.Writer(z.LE, z.XCDR2)
m = w.emheader_begin(2, False, z.LC4)
w.put_u16(0xFFFF)
w.put_u16(0x0002)
w.emheader_end(m, z.LC4)
check("lc4 writes a nextint", hx(w), "02000040" + "04000000" + "ffff0200")
w = z.Writer(z.LE, z.XCDR2)
m = w.emheader_begin(0, False, z.LC5)
w.put_string("abc")
w.emheader_end(m, z.LC5)
check("lc5 reuses the body's own word", hx(w),
      "00000050" + "04000000" + "61626300")

# 0x80000000 | (2 << 28) | 0 = 0xA0000000.
w = z.Writer(z.LE, z.XCDR2)
m = w.emheader_begin(0, True, z.LC2)
w.put_u32(0)
w.emheader_end(m, z.LC2)
check("must-understand is bit thirty-one", hx(w), "000000a0" + "00000000")

# The top of the range: 0x0FFFFFFF fits and 0x10000000 does not. A
# truncating header would put a different member's id on the wire.
w = z.Writer(z.LE, z.XCDR2)
w.emheader_begin(0x0FFFFFFF, False, z.LC2)
check_true("member id 2^28 - 1 fits", True)
check_true("a 29-bit member id is refused", refused(
    lambda: z.Writer(z.LE, z.XCDR2).emheader_begin(0x10000000, False, z.LC2)))


def _lc_mismatch():
    lw = z.Writer(z.LE, z.XCDR2)
    lm = lw.emheader_begin(0, False, z.LC2)
    lw.put_u16(1)
    lw.emheader_end(lm, z.LC2)


check_true("a length code that disagrees with the body is refused",
           refused(_lc_mismatch))

# --- the integer writers refuse a value past their width -----------------

# The defect class this stops: put_u16(70000) wrote 0x1170, the low
# sixteen bits, and the frame carried a plausible wrong value.
check_true("put_u16 refuses 70000",
           refused(lambda: z.Writer(z.LE, z.XCDR2).put_u16(70000)))
check_true("put_u8 refuses 256",
           refused(lambda: z.Writer(z.LE, z.XCDR2).put_u8(256)))
check_true("put_u32 refuses 2^32",
           refused(lambda: z.Writer(z.LE, z.XCDR2).put_u32(1 << 32)))
check_true("put_u64 refuses 2^64",
           refused(lambda: z.Writer(z.LE, z.XCDR2).put_u64(1 << 64)))

# --- the PL_CDR1 parameter list -----------------------------------------

# tests/interop-results/struct-mutable/zerodds-rust/pl-cdr1.le.bin. The
# three things this pins: the double's parameter declares 8 and not 12,
# because its value aligns against its own block; the short's declares 4
# and not 2, because the padding that brings the next header back to a
# four-octet boundary is inside the length; and the sentinel carries the
# must-understand flag, so it is 0x7F02 and not 0x3F02.
w = z.Writer(z.LE, z.XCDR1)
p = w.pid_begin(0, False)
w.put_u32(0xFFED2979)
w.pid_end(p)
p = w.pid_begin(1, False)
w.put_f64(3.141592653589793)
w.pid_end(p)
p = w.pid_begin(2, False)
w.put_u16(0xFFFE)
w.pid_end(p)
w.pid_list_end()
check("struct-mutable xcdr1 matches the rust column", hx(w),
      "00000400" + "7929edff" + "01000800" + "182d4454fb210940" +
      "02000400" + "feff0000" + "027f0000")

# The same case's pl-cdr1.be.bin. Both halves of the header go out in the
# stream's order.
w = z.Writer(z.BE, z.XCDR1)
p = w.pid_begin(0, False)
w.put_u32(0xFFED2979)
w.pid_end(p)
p = w.pid_begin(1, False)
w.put_f64(3.141592653589793)
w.pid_end(p)
p = w.pid_begin(2, False)
w.put_u16(0xFFFE)
w.pid_end(p)
w.pid_list_end()
check("struct-mutable xcdr1 big endian", hx(w),
      "00000004" + "ffed2979" + "00010008" + "400921fb54442d18" +
      "00020004" + "fffe0000" + "7f020000")

# tests/interop-results/annotation-id-sparse/zerodds-rust/pl-cdr1.le.bin.
# The id 16383 is past 0x3F00 and forces the long form even though the
# value is four octets. The marker carries the must-understand flag and the
# member does not.
w = z.Writer(z.LE, z.XCDR1)
p = w.pid_begin(0, False)
w.put_u32(0xFFED2979)
w.pid_end(p)
p = w.pid_begin(1, False)
w.put_u32(0x7FFFFFFF)
w.pid_end(p)
p = w.pid_extended_begin(16383, False)
w.put_u32(0xFFFFFFFE)
w.pid_end(p)
p = w.pid_extended_begin(268435455, False)
w.put_u32(0x00000001)
w.pid_end(p)
w.pid_list_end()
check("the extended parameter header matches the rust column", hx(w),
      "00000400" + "7929edff" + "01000400" + "ffffff7f" +
      "017f0800" + "ff3f0000" + "04000000" + "feffffff" +
      "017f0800" + "ffffff0f" + "04000000" + "01000000" + "027f0000")

# tests/interop-results/annotation-must-understand -- the id 0 with the
# flag reads 0040 in a little-endian stream.
w = z.Writer(z.LE, z.XCDR1)
p = w.pid_begin(0, True)
w.put_u32(0xFFED2979)
w.pid_end(p)
check("the parameter must-understand flag is bit fourteen", hx(w),
      "00400400" + "7929edff")

# 0x3F00 is the last id the short form carries, inclusive.
z.Writer(z.LE, z.XCDR1).pid_begin(0x3F00, False)
check_true("an id past the short range is refused by the short header",
           refused(lambda: z.Writer(z.LE, z.XCDR1).pid_begin(0x3F01, False)))

# tests/interop-results/annotation-optional-absent/zerodds-rust/
# pl-cdr1.le.bin. The header is written either way and declares zero.
w = z.Writer(z.LE, z.XCDR1)
w.put_u32(0xFFED2979)
w.pid_end(w.pid_begin(1, False))
w.put_u32(0xFFFFFFFE)
check("an absent optional parameter keeps its header", hx(w),
      "7929edff" + "01000000" + "feffffff")

# --- the wide character and string --------------------------------------

# tests/interop-results/primitive-wchar -- a-umlaut is U+00E4, U+1234 the
# Ethiopic syllable see.
w = z.Writer(z.LE, z.XCDR2)
w.put_wchar(0x00E4)
w.put_wchar(0x1234)
check("wchar is two octets in the stream's order (le)", hx(w), "e4003412")
w = z.Writer(z.BE, z.XCDR2)
w.put_wchar(0x00E4)
w.put_wchar(0x1234)
check("wchar is two octets in the stream's order (be)", hx(w), "00e41234")

# tests/interop-results/char-and-wchar -- one octet of padding before the
# wide one.
w = z.Writer(z.LE, z.XCDR2)
w.put_char(ord("a"))
w.put_wchar(ord("z"))
check("wchar aligns to two", hx(w), "61007a00")

# U+FFFF is the last one that fits and U+10000 the first that does not.
z.Writer(z.LE, z.XCDR2).put_wchar(0xFFFF)
check_true("a code point past the bmp has no wchar form", refused(
    lambda: z.Writer(z.LE, z.XCDR2).put_wchar(0x10000)))

# U+0161's low octet is 0x61, which is `a`.
z.Writer(z.LE, z.XCDR2).put_char(0xFF)
check_true("a code point past latin-1 has no char form", refused(
    lambda: z.Writer(z.LE, z.XCDR2).put_char(0x0161)))

# tests/interop-results/wstring-unbounded-utf8 -- seven code units and the
# length word reads 14, not 7. No terminator, no byte-order mark.
w = z.Writer(z.LE, z.XCDR2)
w.put_wstring([0x00E4, 0x00F6, 0x00FC, 0x0020, 0x1234, 0x0020, 0x005A])
check("wstring length counts octets", hx(w),
      "0e000000" + "e400" + "f600" + "fc00" + "2000" + "3412" + "2000" +
      "5a00")
w = z.Writer(z.LE, z.XCDR2)
w.put_wstring([])
check("the empty wstring is a length of zero", hx(w), "00000000")

# --- the bitset anchor ---------------------------------------------------

for name, bits, anchor, endian, want in [
    # bitset-basic: bitfield<3> low = 5, bitfield<5> high = 17.
    ("bitset-basic", 5 | 17 << 3, 1, z.LE, "8d"),
    # bitset-typed-fields: 5, 1, 4001 over 3 + 1 + 12 bits.
    ("bitset-typed-fields", 5 | 1 << 3 | 4001 << 4, 2, z.LE, "1dfa"),
    ("bitset-typed-fields be", 5 | 1 << 3 | 4001 << 4, 2, z.BE, "fa1d"),
    # bitset-multiword: 20 + 20 + 24 bits into one eight-octet anchor.
    ("bitset-multiword", 0xFFFFF | 0xBC614E << 40, 8, z.LE,
     "ffff0f00004e61bc"),
    ("bitset-multiword be", 0xFFFFF | 0xBC614E << 40, 8, z.BE,
     "bc614e00000fffff"),
]:
    w = z.Writer(endian, z.XCDR2)
    w.put_bitset(bits, anchor)
    check(name, hx(w), want)

# The one place a bitset differs between the two encodings: the anchor's
# own alignment, capped at four under XCDR2.
w = z.Writer(z.LE, z.XCDR1)
w.put_u8(0xAB)
w.put_bitset(1, 8)
check("an eight-octet anchor aligns eight under xcdr1", hx(w),
      "ab00000000000000" + "0100000000000000")
w = z.Writer(z.LE, z.XCDR2)
w.put_u8(0xAB)
w.put_bitset(1, 8)
check("an eight-octet anchor aligns four under xcdr2", hx(w),
      "ab000000" + "0100000000000000")
check_true("an anchor that is not one, two, four or eight is refused",
           refused(lambda: z.Writer(z.LE, z.XCDR2).put_bitset(0, 3)))

# --- the packed decimal --------------------------------------------------

# tests/interop-results/fixed-basic -- fixed<9,3> with "123456.789" and
# "-000000.001". P + 1 = 10 is even, so there is no leading zero nibble and
# each value is five octets.
w = z.Writer(z.LE, z.XCDR2)
w.put_fixed("123456.789", 9, 3)
w.put_fixed("-000000.001", 9, 3)
w.put_u8(0xAB)
check("fixed matches the rust column", hx(w),
      "123456789c" + "000000001d" + "ab")

# No multi-octet integer is written, so nothing consults the stream's
# order.
le = z.Writer(z.LE, z.XCDR1)
le.put_fixed("123456.789", 9, 3)
be = z.Writer(z.BE, z.XCDR2)
be.put_fixed("123456.789", 9, 3)
check("fixed is independent of the byte order", hx(le), hx(be))

for value, digits, scale, want in [
    # An odd digit count pairs the sign with a prepended zero nibble.
    ("1", 1, 0, "1c"),
    ("9", 1, 0, "9c"),
    ("-9", 1, 0, "9d"),
    # P + 1 odd -> a leading zero nibble. P = 2 gives two octets.
    ("12", 2, 0, "012c"),
    # scale == digits: the value is all fraction.
    ("0.123", 3, 3, "123c"),
    ("-0.001", 3, 3, "001d"),
    # A negative zero is written positive: there is one zero to write.
    ("-0", 1, 0, "0c"),
    ("-0.000", 3, 3, "000c"),
    # The widest an IDL fixed goes.
    ("1234567890123456789012345678901", 31, 0,
     "1234567890123456789012345678901c"),
    ("-1234567890123456789012345678901", 31, 0,
     "1234567890123456789012345678901d"),
    # Padding on both sides at once.
    ("7.5", 9, 3, "000007500c"),
]:
    w = z.Writer(z.LE, z.XCDR2)
    w.put_fixed(value, digits, scale)
    check("fixed %r as fixed<%d,%d>" % (value, digits, scale), hx(w), want)
    check("fixed %r octet count" % value, len(w.buf), (digits + 2) // 2)

for value, digits, scale in [
    ("1234567", 6, 0),  # seven integer digits into six
    ("1.2345", 6, 3),   # four fraction digits into three
    ("1.0", 2, 3),      # a scale wider than the digit count
    ("twelve", 4, 0),   # not a decimal
    ("", 4, 0),         # no digits at all
]:
    check_true("a value wider than fixed<%d,%d> is refused (%r)"
               % (digits, scale, value),
               refused(lambda v=value, d=digits, s=scale:
                       z.Writer(z.LE, z.XCDR2).put_fixed(v, d, s)))

# --- the reading direction ----------------------------------------------

for encoding in (z.XCDR1, z.XCDR2):
    for endian in (z.LE, z.BE):
        w = z.Writer(endian, encoding)
        w.put_u8(0xFF)
        w.put_bool(True)
        w.put_char(0xFF)
        w.put_u16(0xFFFF)
        w.put_u32(0xFFFFFFFF)
        w.put_u64(0xFFFFFFFFFFFFFFFF)
        w.put_f32(-3.5)
        w.put_f64(3.141592653589793)
        w.put_wchar(0xFFFF)
        w.put_string(u"grüße")
        w.put_seq_u8(b"\xde\xad")
        w.put_wstring([0xD83D, 0xDE00])
        w.put_bitset(0xBC614E00000FFFFF, 8)
        w.put_fixed("-000000.001", 9, 3)
        r = z.Reader(w.bytes(), endian, encoding)
        got = (r.get_u8(), r.get_bool(), r.get_char(), r.get_u16(),
               r.get_u32(), r.get_u64(), r.get_f32(), r.get_f64(),
               r.get_wchar(), r.get_string(), bytes(r.get_seq_u8()),
               r.get_wstring(), r.get_bitset(8), r.get_fixed(9, 3))
        want = (0xFF, True, 0xFF, 0xFFFF, 0xFFFFFFFF, 0xFFFFFFFFFFFFFFFF,
                -3.5, 3.141592653589793, 0xFFFF, u"grüße",
                b"\xde\xad", [0xD83D, 0xDE00], 0xBC614E00000FFFFF,
                "-000000.001")
        check("every write reads back (encoding=%d endian=%d)"
              % (encoding, endian), got, want)
        check("the reader stops at the end (encoding=%d endian=%d)"
              % (encoding, endian), r.pos, len(w.buf))

# The framing reads back: one member per length code.
w = z.Writer(z.LE, z.XCDR2)
d = w.dheader_begin()
for lc in (z.LC0, z.LC1, z.LC2, z.LC3, z.LC4):
    m = w.emheader_begin(lc, lc == z.LC4, lc)
    if lc == z.LC0:
        w.put_u8(1)
    elif lc == z.LC1:
        w.put_u16(2)
    elif lc == z.LC2:
        w.put_u32(3)
    elif lc == z.LC3:
        w.put_u64(4)
    else:
        w.put_u16(5)
        w.put_u16(6)
    w.emheader_end(m, lc)
m = w.emheader_begin(5, False, z.LC5)
w.put_string("abc")
w.emheader_end(m, z.LC5)
w.dheader_end(d)
r = z.Reader(w.bytes(), z.LE, z.XCDR2)
check("the dheader reads the body length", r.dheader_read(), len(w.buf) - 4)
framing_ok = True
for want_lc, want_extent, want_mu in [(0, 1, False), (1, 2, False),
                                      (2, 4, False), (3, 8, False),
                                      (4, 4, True), (5, 8, False)]:
    member_id, mu, lc, extent = r.emheader_read()
    if (member_id, mu, lc, extent) != (want_lc, want_mu, want_lc,
                                       want_extent):
        framing_ok = False
    r.pos += extent
check_true("the framing reads back in every length code", framing_ok)

# The parameter list reads back, short and extended.
w = z.Writer(z.LE, z.XCDR1)
p = w.pid_begin(0, False)
w.put_u32(0xFFED2979)
w.pid_end(p)
p = w.pid_begin(1, True)
w.put_f64(3.141592653589793)
w.pid_end(p)
p = w.pid_extended_begin(16383, True)
w.put_u16(0xFFFE)
w.pid_end(p)
w.pid_list_end()
r = z.Reader(w.bytes(), z.LE, z.XCDR1)
pid_ok = True
for want_id, want_mu, want_len in [(0, False, 4), (1, True, 8),
                                   (16383, True, 4)]:
    member_id, mu, length, end = r.pid_read()
    if end or (member_id, mu, length) != (want_id, want_mu, want_len):
        pid_ok = False
    r.pid_skip(r.pos, length)
check_true("the parameter list reads back", pid_ok)
check_true("the sentinel ends the list", r.pid_read()[3])
check("the reader stops at the list end", r.pos, len(w.buf))

if failures:
    print("%d surface checks FAILED" % len(failures), file=sys.stderr)
    sys.exit(1)
print("ALL OK")
