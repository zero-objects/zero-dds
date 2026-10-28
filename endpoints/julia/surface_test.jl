# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 ZeroDDS Contributors
#
# The wire-core operations this SDK grew so that a caller can write every
# construct XTypes 1.3 defines: the 64-bit float, the XCDR1 alignment cap, the
# delimiter, the member header in all eight length codes, the PL_CDR1
# parameter list, the wide character and string, the packed decimal and the
# bitset anchor.
#
# ## The expected octets are foreign
#
# Every multi-member case here is the byte string a *vendor's own* serialiser
# produced, recorded in `crates/ast-layout/tests/fixtures/vendor_bytes.rows.tsv`
# from `tools/byte-gate/cases/<case>` and named in the test. Cyclone DDS, RTI
# Connext, Fast DDS, omniORB and JacORB wrote these; comparing this SDK
# against a reference file this project also wrote would only show that two
# halves of one opinion agree.
#
# Where the vendors disagree the test says which reading this binding takes
# and which it does not, because a wire question with two answers in the
# field is a measurement and not a defect.
#
# Each operation is exercised at the ends of its range and not only in the
# middle, because that is where the defects in these bindings were.
#
# Run with `julia surface_test.jl`.

include("zerodds.jl")
using .ZeroDDS

const FAILURES = String[]

hex(w) = bytes2hex(ZeroDDS.bytes(w))

function check(name, got, want)
    if got != want
        push!(FAILURES, "$name:\n    got  $got\n    want $want")
    end
end

function refuses(name, f)
    try
        f()
        push!(FAILURES, "$name: it was accepted")
    catch e
        e isa ArgumentError || e isa InexactError || e isa BoundsError ||
            push!(FAILURES, "$name: refused with $(typeof(e)), not an ArgumentError")
    end
end

writer(endian, encoding) = ZeroDDS.Writer(endian, encoding)

# --- the alignment cap the encoding sets ---------------------------------
#
# tools/byte-gate/cases/ext_appendable: `@appendable struct Ext { long a;
# double b; short c; }` with a = 7, b = 1.5, c = -3. XTypes 1.3 §7.4.1 puts
# MAXALIGN at eight under XCDR1, so the `double` takes four octets of padding
# there and none under XCDR2 — which is exactly the difference between the
# two vendor rows.

function ext_appendable(endian, encoding, framed)
    w = writer(endian, encoding)
    mark = framed ? ZeroDDS.dheader_begin!(w) : 0
    ZeroDDS.put_u32!(w, 7)
    ZeroDDS.put_f64!(w, 1.5)
    ZeroDDS.put_u16!(w, -3)
    framed && ZeroDDS.dheader_end!(w, mark)
    hex(w)
end

# Cyclone, RTI and Fast DDS all wrote these four, octet for octet.
check("ext_appendable xcdr1 le", ext_appendable(ZeroDDS.LE, ZeroDDS.XCDR1, false),
      "0700000000000000000000000000f83ffdff")
check("ext_appendable xcdr1 be", ext_appendable(ZeroDDS.BE, ZeroDDS.XCDR1, false),
      "00000007000000003ff8000000000000fffd")
check("ext_appendable xcdr2 le", ext_appendable(ZeroDDS.LE, ZeroDDS.XCDR2, true),
      "0e00000007000000000000000000f83ffdff")
check("ext_appendable xcdr2 be", ext_appendable(ZeroDDS.BE, ZeroDDS.XCDR2, true),
      "0000000e000000073ff8000000000000fffd")

# The cap is the encoding's and not a constant: a writer made for XCDR2 must
# not be able to reach eight.
check("max_align xcdr1", ZeroDDS.Writer(ZeroDDS.BE, ZeroDDS.XCDR1).max_align, 8)
check("max_align xcdr2", ZeroDDS.Writer(ZeroDDS.BE, ZeroDDS.XCDR2).max_align, 4)

# --- the 64-bit float ----------------------------------------------------

for (value, want) in [
    (1.5, "000000000000f83f"),                  # the vendors' own sample
    (0.0, "0000000000000000"),
    (-0.0, "0000000000000080"),                 # a signed zero keeps its sign
    (1.7976931348623157e308, "ffffffffffffef7f"),  # MAX
    (5.0e-324, "0100000000000000"),             # the smallest subnormal
    (Inf, "000000000000f07f"),
]
    w = writer(ZeroDDS.LE, ZeroDDS.XCDR2)
    ZeroDDS.put_f64!(w, value)
    check("put_f64!($value)", hex(w), want)
end

# --- DHEADER -------------------------------------------------------------

let w = writer(ZeroDDS.LE, ZeroDDS.XCDR2)
    # §7.4.3.5: three octets of content, so the DHEADER reads 3 and the four
    # octets it occupies are not in it.
    mark = ZeroDDS.dheader_begin!(w)
    ZeroDDS.put_u8!(w, 1)
    ZeroDDS.put_u8!(w, 2)
    ZeroDDS.put_u8!(w, 3)
    ZeroDDS.dheader_end!(w, mark)
    check("dheader counts the body", hex(w), "03000000010203")
end

let w = writer(ZeroDDS.BE, ZeroDDS.XCDR2)
    ZeroDDS.dheader_end!(w, ZeroDDS.dheader_begin!(w))
    check("dheader of an empty body", hex(w), "00000000")
end

# --- the member header, in every length code -----------------------------
#
# tools/byte-gate/cases/ext_mutable: the same three members, `@mutable`.
# Cyclone and RTI wrote the XCDR2 rows identically.

function ext_mutable_xcdr2(endian)
    w = writer(endian, ZeroDDS.XCDR2)
    d = ZeroDDS.dheader_begin!(w)
    m = ZeroDDS.emheader_begin!(w, 0, false, ZeroDDS.LC2)
    ZeroDDS.put_u32!(w, 7)
    ZeroDDS.emheader_end!(w, m, ZeroDDS.LC2)
    m = ZeroDDS.emheader_begin!(w, 1, false, ZeroDDS.LC3)
    ZeroDDS.put_f64!(w, 1.5)
    ZeroDDS.emheader_end!(w, m, ZeroDDS.LC3)
    m = ZeroDDS.emheader_begin!(w, 2, false, ZeroDDS.LC1)
    ZeroDDS.put_u16!(w, -3)
    ZeroDDS.emheader_end!(w, m, ZeroDDS.LC1)
    ZeroDDS.dheader_end!(w, d)
    hex(w)
end

check("ext_mutable xcdr2 le", ext_mutable_xcdr2(ZeroDDS.LE),
      "1a000000000000200700000001000030000000000000f83f02000010fdff")
check("ext_mutable xcdr2 be", ext_mutable_xcdr2(ZeroDDS.BE),
      "0000001a2000000000000007300000013ff800000000000010000002fffd")

# tools/byte-gate/cases/pid_3f00: `@mutable struct PidEdge { @id(16127) long
# a; @id(16128) long b; }`. Cyclone, RTI and Fast DDS agree on the XCDR2 row,
# which is where the member id sits inside the EMHEADER's low 28 bits.
let w = writer(ZeroDDS.LE, ZeroDDS.XCDR2)
    d = ZeroDDS.dheader_begin!(w)
    m = ZeroDDS.emheader_begin!(w, 16127, false, ZeroDDS.LC2)
    ZeroDDS.put_u32!(w, 7)
    ZeroDDS.emheader_end!(w, m, ZeroDDS.LC2)
    m = ZeroDDS.emheader_begin!(w, 16128, false, ZeroDDS.LC2)
    ZeroDDS.put_u32!(w, 9)
    ZeroDDS.emheader_end!(w, m, ZeroDDS.LC2)
    ZeroDDS.dheader_end!(w, d)
    check("pid_3f00 xcdr2 le", hex(w), "10000000ff3e002007000000003f002009000000")
end

# tools/byte-gate/cases/lc_5_6_7: five collection members whose length codes
# are LC5, LC5, LC6, LC7 and — where the two vendors part — LC4 for Cyclone
# and LC3 for Fast DDS. Both are written here, because a binding that could
# only produce one of them could not read the other's wire back.
function lc_5_6_7(tail_code)
    w = writer(ZeroDDS.LE, ZeroDDS.XCDR2)
    d = ZeroDDS.dheader_begin!(w)
    m = ZeroDDS.emheader_begin!(w, 1, false, ZeroDDS.LC5)
    ZeroDDS.put_string!(w, "abc")
    ZeroDDS.emheader_end!(w, m, ZeroDDS.LC5)
    m = ZeroDDS.emheader_begin!(w, 2, false, ZeroDDS.LC5)
    ZeroDDS.put_seq_u8!(w, UInt8[1, 2, 3])
    ZeroDDS.emheader_end!(w, m, ZeroDDS.LC5)
    m = ZeroDDS.emheader_begin!(w, 3, false, ZeroDDS.LC6)
    ZeroDDS.put_u32!(w, 1)
    ZeroDDS.put_u32!(w, 7)
    ZeroDDS.emheader_end!(w, m, ZeroDDS.LC6)
    m = ZeroDDS.emheader_begin!(w, 4, false, ZeroDDS.LC7)
    ZeroDDS.put_u32!(w, 1)
    ZeroDDS.put_f64!(w, 1.5)
    ZeroDDS.emheader_end!(w, m, ZeroDDS.LC7)
    m = ZeroDDS.emheader_begin!(w, 5, false, tail_code)
    ZeroDDS.put_u32!(w, 2)
    ZeroDDS.put_u16!(w, 4)
    ZeroDDS.put_u16!(w, 5)
    ZeroDDS.emheader_end!(w, m, tail_code)
    ZeroDDS.dheader_end!(w, d)
    hex(w)
end

const LC_567_HEAD =
    "01000050" * "04000000" * "61626300" *
    "02000050" * "03000000" * "01020300" *
    "03000060" * "01000000" * "07000000" *
    "04000070" * "01000000" * "000000000000f83f"

check("lc_5_6_7 xcdr2 le (Cyclone, LC4 tail)", lc_5_6_7(ZeroDDS.LC4),
      "44000000" * LC_567_HEAD * "05000040" * "08000000" * "02000000" * "04000500")
check("lc_5_6_7 xcdr2 le (Fast DDS, LC3 tail)", lc_5_6_7(ZeroDDS.LC3),
      "40000000" * LC_567_HEAD * "05000030" * "02000000" * "04000500")

# The must-understand flag is bit 31 and not bit 30: 0x80000000 | (2 << 28).
let w = writer(ZeroDDS.LE, ZeroDDS.XCDR2)
    m = ZeroDDS.emheader_begin!(w, 0, true, ZeroDDS.LC2)
    ZeroDDS.put_u32!(w, 0)
    ZeroDDS.emheader_end!(w, m, ZeroDDS.LC2)
    check("must-understand is bit 31", hex(w), "000000a000000000")
end

# --- the PL_CDR1 parameter list ------------------------------------------
#
# tools/byte-gate/cases/ext_mutable, XCDR1. RTI's rows are the short-header
# form this binding writes. The three things they pin: the `double`'s
# parameter declares 8 and not 12, because its value aligns against its own
# block; the `short`'s declares 4 and not 2, because the padding that brings
# the next header back to a four-octet boundary is inside the length; and the
# sentinel carries the must-understand flag, so it is 0x7F02.
#
# Cyclone writes the same sample with an extended header on every parameter
# (56 octets against RTI's 32) and Fast DDS declares 2 for the `short` and a
# bare 0x3F02 sentinel. Three readings, all on the wire today; this binding
# takes RTI's.

function ext_mutable_xcdr1(endian)
    w = writer(endian, ZeroDDS.XCDR1)
    p = ZeroDDS.pid_begin!(w, 0, false)
    ZeroDDS.put_u32!(w, 7)
    ZeroDDS.pid_end!(w, p)
    p = ZeroDDS.pid_begin!(w, 1, false)
    ZeroDDS.put_f64!(w, 1.5)
    ZeroDDS.pid_end!(w, p)
    p = ZeroDDS.pid_begin!(w, 2, false)
    ZeroDDS.put_u16!(w, -3)
    ZeroDDS.pid_end!(w, p)
    ZeroDDS.pid_list_end!(w)
    hex(w)
end

check("ext_mutable xcdr1 le (RTI)", ext_mutable_xcdr1(ZeroDDS.LE),
      "000004000700000001000800000000000000f83f02000400fdff0000027f0000")
check("ext_mutable xcdr1 be (RTI)", ext_mutable_xcdr1(ZeroDDS.BE),
      "0000000400000007000100083ff800000000000000020004fffd00007f020000")

# tools/byte-gate/cases/pid_3f00 under XCDR1: RTI and Fast DDS both carry
# @id(16128) in the *short* header, which is what puts 0x3F00 at the top of
# its range inclusive rather than one past it.
let w = writer(ZeroDDS.LE, ZeroDDS.XCDR1)
    p = ZeroDDS.pid_begin!(w, 0x3EFF, false)
    ZeroDDS.put_u32!(w, 7)
    ZeroDDS.pid_end!(w, p)
    p = ZeroDDS.pid_begin!(w, 0x3F00, false)
    ZeroDDS.put_u32!(w, 9)
    ZeroDDS.pid_end!(w, p)
    ZeroDDS.pid_list_end!(w)
    check("pid_3f00 xcdr1 le (RTI)", hex(w), "ff3e040007000000003f040009000000027f0000")
end

# The extended header, for an id past 0x3F00. The marker carries the
# must-understand flag whatever the member says; the member's own flag is bit
# 30 of the eMemberHeader.
let w = writer(ZeroDDS.LE, ZeroDDS.XCDR1)
    p = ZeroDDS.pid_extended_begin!(w, 16383, false)
    ZeroDDS.put_u32!(w, 0xFFFFFFFE)
    ZeroDDS.pid_end!(w, p)
    ZeroDDS.pid_list_end!(w)
    check("extended parameter header", hex(w),
          "017f0800ff3f000004000000feffffff027f0000")
end

let w = writer(ZeroDDS.LE, ZeroDDS.XCDR1)
    p = ZeroDDS.pid_begin!(w, 0, true)
    ZeroDDS.put_u32!(w, 7)
    ZeroDDS.pid_end!(w, p)
    check("the parameter must-understand flag is bit 14", hex(w), "0040040007000000")
end

# --- the wide character and string ---------------------------------------
#
# tools/byte-gate/cases/wchar_member: `@final struct WCh { wchar c; octet
# tail; }` with c = U+0061 and tail = 0xAB. Cyclone and Fast DDS write two
# octets in both encodings and RTI writes two under XCDR2; RTI's XCDR1
# `wchar` is four octets, which is the one reading this binding does not
# take.

for (endian, want) in [(ZeroDDS.LE, "6100ab"), (ZeroDDS.BE, "0061ab")]
    for encoding in (ZeroDDS.XCDR1, ZeroDDS.XCDR2)
        w = writer(endian, encoding)
        ZeroDDS.put_wchar!(w, 0x0061)
        ZeroDDS.put_u8!(w, 0xAB)
        check("wchar_member $endian $encoding", hex(w), want)
    end
end

# A `wchar` aligns to two, so a `char` in front of it costs one octet of pad.
let w = writer(ZeroDDS.LE, ZeroDDS.XCDR2)
    ZeroDDS.put_char!(w, UInt32('a'))
    ZeroDDS.put_wchar!(w, UInt32('z'))
    check("wchar aligns to two", hex(w), "61007a00")
end

# tools/byte-gate/cases/wstring_member: w = "ab", tail = 0xAB. Cyclone and
# RTI-under-XCDR2 count **octets** (4) where Fast DDS counts code units (2).
# Under big endian Cyclone leaves the units little-endian — it copies the
# UTF-16 buffer — and RTI swaps them; this binding swaps, so its BE form is
# RTI's.
let w = writer(ZeroDDS.LE, ZeroDDS.XCDR2)
    ZeroDDS.put_wstring!(w, UInt16[0x0061, 0x0062])
    ZeroDDS.put_u8!(w, 0xAB)
    check("wstring_member le (Cyclone + RTI)", hex(w), "0400000061006200ab")
end
let w = writer(ZeroDDS.BE, ZeroDDS.XCDR2)
    ZeroDDS.put_wstring!(w, UInt16[0x0061, 0x0062])
    ZeroDDS.put_u8!(w, 0xAB)
    check("wstring_member be (RTI)", hex(w), "0000000400610062ab")
end

# tools/byte-gate/cases/wstring_nonbmp: U+1D11E as the surrogate pair
# D834 DD1E. Cyclone and RTI both write four octets; Fast DDS writes one
# unit, 0xD11E, which is neither half of the pair.
let w = writer(ZeroDDS.LE, ZeroDDS.XCDR2)
    ZeroDDS.put_wstring!(w, UInt16[0xD834, 0xDD1E])
    ZeroDDS.put_u8!(w, 0xAB)
    check("wstring_nonbmp le (Cyclone + RTI)", hex(w), "0400000034d81eddab")
end
let w = writer(ZeroDDS.BE, ZeroDDS.XCDR2)
    ZeroDDS.put_wstring!(w, UInt16[0xD834, 0xDD1E])
    ZeroDDS.put_u8!(w, 0xAB)
    check("wstring_nonbmp be (RTI)", hex(w), "00000004d834dd1eab")
end

# The empty one is a length of zero and nothing after it.
let w = writer(ZeroDDS.LE, ZeroDDS.XCDR2)
    ZeroDDS.put_wstring!(w, UInt16[])
    check("an empty wstring", hex(w), "00000000")
end

# --- the bitset anchor ---------------------------------------------------
#
# tools/wire-oracle/cases/bitset_chain: `bitset B3` chains 8 + 8 + 4 bits into
# one four-octet anchor, inside `@final struct BitChain { octet head; B3
# value; octet tail; }` with head = 1, a = 0x11, b = 0x22, c = 0x3 and
# tail = 0xAB. Only Fast DDS produced octets — Cyclone's and RTI's IDL front
# ends do not parse `bitset` at all, which is itself the measurement recorded
# in vendor_bytes.rows.tsv.
for (endian, want) in [(ZeroDDS.LE, "0100000011220300ab"),
                       (ZeroDDS.BE, "0100000000032211ab")]
    w = writer(endian, ZeroDDS.XCDR2)
    ZeroDDS.put_u8!(w, 1)
    ZeroDDS.put_bitset!(w, UInt64(0x11) | (UInt64(0x22) << 8) | (UInt64(0x3) << 16), 4)
    ZeroDDS.put_u8!(w, 0xAB)
    check("bitset_chain $endian (Fast DDS)", hex(w), want)
end

# tools/wire-oracle/cases/bitset_overwide: `bitfield<3> v` assigned 200. The
# holder keeps three bits, 200 mod 8 is 0, and the anchor is one octet — so
# it aligns to one and the `octet` behind it needs no padding.
for endian in (ZeroDDS.LE, ZeroDDS.BE)
    w = writer(endian, ZeroDDS.XCDR2)
    ZeroDDS.put_bitset!(w, UInt64(200 & 0x7), 1)
    ZeroDDS.put_u8!(w, 0xAB)
    check("bitset_overwide $endian (Fast DDS)", hex(w), "00ab")
end

# An eight-octet anchor is the one place a bitset differs between the two
# encodings: the anchor's own alignment, capped at four under XCDR2.
let w = writer(ZeroDDS.LE, ZeroDDS.XCDR1)
    ZeroDDS.put_u8!(w, 0xAB)
    ZeroDDS.put_bitset!(w, UInt64(1), 8)
    check("an eight-octet anchor under XCDR1", hex(w), "ab000000000000000100000000000000")
end
let w = writer(ZeroDDS.LE, ZeroDDS.XCDR2)
    ZeroDDS.put_u8!(w, 0xAB)
    ZeroDDS.put_bitset!(w, UInt64(1), 8)
    check("an eight-octet anchor under XCDR2", hex(w), "ab0000000100000000000000")
end

# --- the packed decimal --------------------------------------------------
#
# crates/ast-layout/tests/fixtures/vendor_bytes.md §"`fixed<P,S>`, measured
# against two CORBA ORBs": no DDS generator parses the declaration at all —
# Cyclone, RTI, Fast DDS and OpenDDS's tao_idl each refuse it — so the oracle
# is omniORB 4.3.3 and JacORB 3.9, through `tools/wire-oracle/fixed-orbs.sh`.
# The struct below is theirs, member for member, and the octets are
# omniORB's.
#
# JacORB agrees on eight of the nine and writes one octet for `0.00` where
# omniORB writes three: its `write_fixed` takes a `BigDecimal`, and
# `new BigDecimal("0.00").precision()` is 1 in Java, so the declared width is
# gone before the ORB sees the value. This binding takes omniORB's reading —
# the octet count comes from the declared digits and not from the value.
let w = writer(ZeroDDS.LE, ZeroDDS.XCDR2)
    ZeroDDS.put_u8!(w, 0x77)
    ZeroDDS.put_fixed!(w, "123.45", 5, 2)
    ZeroDDS.put_fixed!(w, "-123.45", 5, 2)
    ZeroDDS.put_fixed!(w, "0.00", 5, 2)
    ZeroDDS.put_fixed!(w, "12.34", 4, 2)
    ZeroDDS.put_fixed!(w, "0.123", 3, 3)
    ZeroDDS.put_fixed!(w, "7", 1, 0)
    ZeroDDS.put_fixed!(w, "42", 2, 0)
    ZeroDDS.put_fixed!(w, "1234567890123456789012345678901", 31, 0)
    ZeroDDS.put_u8!(w, 0xAB)
    check("FixedAll (omniORB 4.3.3)", hex(w),
          "77" * "12345c" * "12345d" * "00000c" * "01234c" * "123c" * "7c" *
          "042c" * "1234567890123456789012345678901c" * "ab")
end

# The octets do not swap: packed BCD is a byte sequence and not an integer,
# which is why omniORB records the same string under `--endian be`.
let le = writer(ZeroDDS.LE, ZeroDDS.XCDR1), be = writer(ZeroDDS.BE, ZeroDDS.XCDR2)
    ZeroDDS.put_fixed!(le, "123.45", 5, 2)
    ZeroDDS.put_fixed!(be, "123.45", 5, 2)
    check("fixed is independent of the byte order", hex(le), hex(be))
end

# A negative zero goes out positive: there is one zero to write.
for (value, digits, scale, want) in [
    ("1", 1, 0, "1c"), ("9", 1, 0, "9c"), ("-9", 1, 0, "9d"),
    ("-0", 1, 0, "0c"), ("-0.000", 3, 3, "000c"),
    ("0.123", 3, 3, "123c"), ("-0.001", 3, 3, "001d"),
    ("7.5", 9, 3, "000007500c"),
    ("-1234567890123456789012345678901", 31, 0,
     "1234567890123456789012345678901d"),
]
    w = writer(ZeroDDS.LE, ZeroDDS.XCDR2)
    ZeroDDS.put_fixed!(w, value, digits, scale)
    check("put_fixed!($value, $digits, $scale)", hex(w), want)
    check("put_fixed!($value, $digits, $scale) octet count",
          length(ZeroDDS.bytes(w)), (digits + 2) ÷ 2)
end

# `omniidl` refuses the same three shapes, in its own words: `fixed<0,0>` is
# `Size must be at least 1`, `fixed<2,5>` is `Fixed point scale factor is
# greater than the number of digits`, `fixed<32,0>` is `Fixed point values
# may not have more than 31 digits`.
refuses("fixed<0,0>", () -> ZeroDDS.fixed_octets("1", 0, 0))
refuses("fixed<32,0>", () -> ZeroDDS.fixed_octets("1", 32, 0))
refuses("fixed<2,5>", () -> ZeroDDS.fixed_octets("1.0", 2, 5))
refuses("seven integer digits into six", () -> ZeroDDS.fixed_octets("1234567", 6, 0))
refuses("four fraction digits into three", () -> ZeroDDS.fixed_octets("1.2345", 6, 3))
refuses("not a decimal", () -> ZeroDDS.fixed_octets("twelve", 4, 0))
refuses("no digits at all", () -> ZeroDDS.fixed_octets("", 4, 0))

# --- the ends of every integer range -------------------------------------
#
# `put_u16!(w, 70000)` used to answer `7011` in nine of the fifteen bindings:
# a frame of the right length holding a different number. Both spellings of
# an argument are taken — the unsigned pattern and the signed IDL value — and
# nothing outside their union is.

for (bits, put, ok, refused) in [
    (8, ZeroDDS.put_u8!,
     [(0, "00"), (-1, "ff"), (-128, "80"), (127, "7f"), (255, "ff")],
     [256, -129, 300, 70000]),
    (16, ZeroDDS.put_u16!,
     [(0, "0000"), (-1, "ffff"), (-32768, "0080"), (32767, "ff7f"), (65535, "ffff")],
     [65536, -32769, 70000]),
    (32, ZeroDDS.put_u32!,
     [(0, "00000000"), (-1, "ffffffff"), (Int(typemin(Int32)), "00000080"),
      (Int(typemax(UInt32)), "ffffffff")],
     [Int(typemax(UInt32)) + 1, Int(typemin(Int32)) - 1]),
    (64, ZeroDDS.put_u64!,
     [(0, "0000000000000000"), (-1, "ffffffffffffffff"),
      (typemin(Int64), "0000000000000080"),
      (typemax(UInt64), "ffffffffffffffff"),
      (9007199254740993, "0100000000002000")],
     [Int128(typemax(UInt64)) + 1, Int128(typemin(Int64)) - 1]),
]
    for (value, want) in ok
        w = writer(ZeroDDS.LE, ZeroDDS.XCDR2)
        put(w, value)
        check("put_u20 20 12 61 79 80 81 98 701 33 100 204 250 395 398 399 400bits)!($value)", hex(w), want)
    end
    for value in refused
        refuses("put_u20 20 12 61 79 80 81 98 701 33 100 204 250 395 398 399 400bits)!($value)", () -> put(writer(ZeroDDS.LE, ZeroDDS.XCDR2), value))
    end
end

# Julia's `^` returns a float whatever its arguments, so `2^53 + 1` is
# `9007199254740992.0` — one less than the literal beside it. An integer
# write that took it would put a plausible, wrong number on the wire, so a
# non-integer is refused before the range is looked at.
check("2^53 + 1 is not the integer it reads as", Float64(2)^53 + 1 == 9007199254740993, false)
refuses("put_u64! of a float", () -> ZeroDDS.put_u64!(writer(ZeroDDS.LE, ZeroDDS.XCDR2), 2.0^53 + 1))
let w = writer(ZeroDDS.LE, ZeroDDS.XCDR2)
    ZeroDDS.put_u64!(w, 9007199254740993)   # the Int64 literal, exact
    check("put_u64!(2^53 + 1) as an integer", hex(w), "0100000000002000")
end

# The wire quantities are typed at their own width and never at Julia's
# `Int`, which is `Int32` on a 32-bit target: `pattern` does its arithmetic in
# `Int128`, so the two bounds of a 64-bit write are the same numbers whatever
# `Int` happens to be, and a `UInt64` argument reaches the octets whole.
let w = writer(ZeroDDS.LE, ZeroDDS.XCDR2)
    ZeroDDS.put_u64!(w, typemax(UInt64))
    check("typemax(UInt64) does not route through Int", hex(w), "ffffffffffffffff")
end
check("the range check is not the platform's width",
      ZeroDDS.pattern(typemax(UInt64), 64), typemax(UInt64))

# `char` and `wchar` at the ends of their code point ranges. U+0161's low
# octet is 0x61, which is `a`: a truncating writer produces a frame of the
# right length holding a plausible, wrong character.
let w = writer(ZeroDDS.LE, ZeroDDS.XCDR2)
    ZeroDDS.put_char!(w, 0x00)
    ZeroDDS.put_char!(w, 0xFF)
    check("char at both ends", hex(w), "00ff")
end
refuses("char U+0100", () -> ZeroDDS.put_char!(writer(ZeroDDS.LE, ZeroDDS.XCDR2), 0x100))
refuses("char U+0161", () -> ZeroDDS.put_char!(writer(ZeroDDS.LE, ZeroDDS.XCDR2), 0x161))
refuses("char below zero", () -> ZeroDDS.put_char!(writer(ZeroDDS.LE, ZeroDDS.XCDR2), -1))
let w = writer(ZeroDDS.LE, ZeroDDS.XCDR2)
    ZeroDDS.put_wchar!(w, 0x0000)
    ZeroDDS.put_wchar!(w, 0xFFFF)
    check("wchar at both ends", hex(w), "0000ffff")
end
refuses("wchar U+10000", () -> ZeroDDS.put_wchar!(writer(ZeroDDS.LE, ZeroDDS.XCDR2), 0x10000))

# The header fields at the ends of their own ranges.
ZeroDDS.emheader_begin!(writer(ZeroDDS.LE, ZeroDDS.XCDR2), 0x0FFFFFFF, false, ZeroDDS.LC2)
refuses("a 29-bit member id",
        () -> ZeroDDS.emheader_begin!(writer(ZeroDDS.LE, ZeroDDS.XCDR2), 0x10000000, false, ZeroDDS.LC2))
refuses("length code 8",
        () -> ZeroDDS.emheader_begin!(writer(ZeroDDS.LE, ZeroDDS.XCDR2), 0, false, 8))
refuses("length code -1",
        () -> ZeroDDS.emheader_begin!(writer(ZeroDDS.LE, ZeroDDS.XCDR2), 0, false, -1))
refuses("LC2 over a body that is not four octets", function ()
    w = writer(ZeroDDS.LE, ZeroDDS.XCDR2)
    m = ZeroDDS.emheader_begin!(w, 0, false, ZeroDDS.LC2)
    ZeroDDS.put_u16!(w, 1)
    ZeroDDS.emheader_end!(w, m, ZeroDDS.LC2)
end)
ZeroDDS.pid_begin!(writer(ZeroDDS.LE, ZeroDDS.XCDR1), 0x3F00, false)
refuses("a short header past 0x3F00",
        () -> ZeroDDS.pid_begin!(writer(ZeroDDS.LE, ZeroDDS.XCDR1), 0x3F01, false))
refuses("a bitset anchor of three",
        () -> ZeroDDS.put_bitset!(writer(ZeroDDS.LE, ZeroDDS.XCDR2), UInt64(0), 3))
refuses("a holder wider than its anchor",
        () -> ZeroDDS.put_bitset!(writer(ZeroDDS.LE, ZeroDDS.XCDR2), UInt64(256), 1))

# --- the tail ------------------------------------------------------------
#
# tools/byte-gate/cases/tail_pad: `@final struct Odd { long a; short b; }`
# with a = 1, b = 2. All four vendors stop at six octets in both encodings —
# a `@final` aggregate is not padded out to its own alignment.
for (endian, want) in [(ZeroDDS.LE, "010000000200"), (ZeroDDS.BE, "000000010002")]
    for encoding in (ZeroDDS.XCDR1, ZeroDDS.XCDR2)
        w = writer(endian, encoding)
        ZeroDDS.put_u32!(w, 1)
        ZeroDDS.put_u16!(w, 2)
        check("tail_pad $endian $encoding", hex(w), want)
    end
end

# --- the reading direction -----------------------------------------------
#
# Every write has a reader, and the vendor's own octets go in: a writer with
# no matching reader is not an endpoint binding.

for encoding in (ZeroDDS.XCDR1, ZeroDDS.XCDR2)
    for endian in (ZeroDDS.LE, ZeroDDS.BE)
        w = writer(endian, encoding)
        ZeroDDS.put_u8!(w, 0xFF)
        ZeroDDS.put_bool!(w, true)
        ZeroDDS.put_char!(w, 0xFF)
        ZeroDDS.put_u16!(w, 0xFFFF)
        ZeroDDS.put_u32!(w, 0xFFFFFFFF)
        ZeroDDS.put_u64!(w, 0xFFFFFFFFFFFFFFFF)
        ZeroDDS.put_f32!(w, -3.5)
        ZeroDDS.put_f64!(w, 1.5)
        ZeroDDS.put_wchar!(w, 0xFFFF)
        ZeroDDS.put_string!(w, "grüße")
        ZeroDDS.put_seq_u8!(w, UInt8[0xDE, 0xAD])
        ZeroDDS.put_wstring!(w, UInt16[0xD83D, 0xDE00])
        ZeroDDS.put_bitset!(w, 0xBC614E00000FFFFF, 8)
        ZeroDDS.put_fixed!(w, "-000000.001", 9, 3)

        r = ZeroDDS.Reader(ZeroDDS.bytes(w), endian, encoding)
        check("round trip u8 $endian $encoding", ZeroDDS.get_u8(r), 0xFF)
        check("round trip bool $endian $encoding", ZeroDDS.get_bool(r), true)
        check("round trip char $endian $encoding", ZeroDDS.get_char(r), UInt32(0xFF))
        check("round trip u16 $endian $encoding", ZeroDDS.get_u16(r), 0xFFFF)
        check("round trip u32 $endian $encoding", ZeroDDS.get_u32(r), 0xFFFFFFFF)
        check("round trip u64 $endian $encoding", ZeroDDS.get_u64(r), 0xFFFFFFFFFFFFFFFF)
        check("round trip f32 $endian $encoding", ZeroDDS.get_f32(r), -3.5f0)
        check("round trip f64 $endian $encoding", ZeroDDS.get_f64(r), 1.5)
        check("round trip wchar $endian $encoding", ZeroDDS.get_wchar(r), UInt32(0xFFFF))
        check("round trip string $endian $encoding", ZeroDDS.get_string(r), "grüße")
        check("round trip seq_u8 $endian $encoding", ZeroDDS.get_seq_u8(r), UInt8[0xDE, 0xAD])
        check("round trip wstring $endian $encoding", ZeroDDS.get_wstring(r),
              UInt16[0xD83D, 0xDE00])
        check("round trip bitset $endian $encoding", ZeroDDS.get_bitset(r, 8),
              0xBC614E00000FFFFF)
        check("round trip fixed $endian $encoding", ZeroDDS.get_fixed(r, 9, 3), "-000000.001")
        check("round trip consumed everything $endian $encoding",
              r.pos, length(ZeroDDS.bytes(w)))
    end
end

# The vendors' own octets, straight in. `ext_mutable` under XCDR2 is Cyclone's
# and RTI's identical row; reading it back is what makes this a binding and
# not a writer.
let bytes = hex2bytes("1a000000000000200700000001000030000000000000f83f02000010fdff")
    r = ZeroDDS.Reader(bytes, ZeroDDS.LE, ZeroDDS.XCDR2)
    check("vendor dheader", ZeroDDS.dheader_read(r), UInt32(26))
    for (want_id, want_lc, want_extent) in
        [(0, ZeroDDS.LC2, 4), (1, ZeroDDS.LC3, 8), (2, ZeroDDS.LC1, 2)]
        id, mu, lc, extent = ZeroDDS.emheader_read(r)
        check("vendor emheader id", Int(id), want_id)
        check("vendor emheader lc", lc, want_lc)
        check("vendor emheader extent", Int(extent), want_extent)
        check("vendor emheader mu", mu, false)
        if want_lc == ZeroDDS.LC2
            check("vendor member a", ZeroDDS.get_u32(r), UInt32(7))
        elseif want_lc == ZeroDDS.LC3
            check("vendor member b", ZeroDDS.get_f64(r), 1.5)
        else
            check("vendor member c", ZeroDDS.get_u16(r), 0xFFFD)
        end
    end
    check("vendor xcdr2 fully consumed", r.pos, length(bytes))
end

# RTI's XCDR1 row for the same sample, through the parameter-list reader.
let bytes = hex2bytes("000004000700000001000800000000000000f83f02000400fdff0000027f0000")
    r = ZeroDDS.Reader(bytes, ZeroDDS.LE, ZeroDDS.XCDR1)
    for (want_id, want_len) in [(0, 4), (1, 8), (2, 4)]
        id, mu, len, list_end = ZeroDDS.pid_read(r)
        check("vendor pid end", list_end, false)
        check("vendor pid id", Int(id), want_id)
        check("vendor pid length", Int(len), want_len)
        check("vendor pid mu", mu, false)
        body = r.pos
        if want_id == 0
            check("vendor parameter a", ZeroDDS.get_u32(r), UInt32(7))
        elseif want_id == 1
            check("vendor parameter b", ZeroDDS.get_f64(r), 1.5)
        else
            check("vendor parameter c", ZeroDDS.get_u16(r), 0xFFFD)
        end
        ZeroDDS.pid_skip!(r, body, len)
    end
    _, _, _, list_end = ZeroDDS.pid_read(r)
    check("vendor sentinel ends the list", list_end, true)
    check("vendor xcdr1 fully consumed", r.pos, length(bytes))
end

# Cyclone's lc_5_6_7 row, whose LC5, LC6 and LC7 members take their length
# from the body's own leading word.
let bytes = hex2bytes(lc_5_6_7(ZeroDDS.LC4))
    r = ZeroDDS.Reader(bytes, ZeroDDS.LE, ZeroDDS.XCDR2)
    ZeroDDS.dheader_read(r)
    for (want_id, want_lc, want_extent) in [(1, ZeroDDS.LC5, 8), (2, ZeroDDS.LC5, 7),
                                            (3, ZeroDDS.LC6, 8), (4, ZeroDDS.LC7, 12),
                                            (5, ZeroDDS.LC4, 8)]
        id, _, lc, extent = ZeroDDS.emheader_read(r)
        check("lc_5_6_7 read id", Int(id), want_id)
        check("lc_5_6_7 read lc", lc, want_lc)
        check("lc_5_6_7 read extent", Int(extent), want_extent)
        r.pos += Int(extent)
    end
end

if isempty(FAILURES)
    println("endpoints/julia: the surface holds, against Cyclone, RTI, Fast DDS and omniORB")
else
    for f in FAILURES
        println(stderr, f)
    end
    println(stderr, "$(length(FAILURES)) failure(s)")
    exit(1)
end
