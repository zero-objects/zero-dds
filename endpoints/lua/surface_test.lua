-- SPDX-License-Identifier: Apache-2.0
-- Copyright 2026 ZeroDDS Contributors
--
-- The wire-core operations this SDK grew so that a caller can write every
-- construct XTypes 1.3 defines: the 64-bit float, the XCDR1 alignment cap,
-- the delimiter, the member header in all eight length codes, the PL_CDR1
-- parameter list, the wide character and string, the packed decimal and the
-- bitset anchor.
--
-- ## The expected octets are foreign
--
-- Every multi-member case here is the byte string a *vendor's own*
-- serialiser produced, recorded in
-- `crates/ast-layout/tests/fixtures/vendor_bytes.rows.tsv` from
-- `tools/byte-gate/cases/<case>` and named in the test. Cyclone DDS, RTI
-- Connext, Fast DDS, omniORB and JacORB wrote these; comparing this SDK
-- against a reference file this project also wrote would only show that two
-- halves of one opinion agree.
--
-- Where the vendors disagree the test says which reading this binding takes
-- and which it does not, because a wire question with two answers in the
-- field is a measurement and not a defect.
--
-- Each operation is exercised at the ends of its range and not only in the
-- middle. Lua has one number type with two subtypes, so a wrong integer and
-- a rounded float are the same value to everything downstream — which is why
-- the float cases below are here at all.
--
-- Run with `lua surface_test.lua` (5.3+, for string.pack).

package.path = (arg[0]:match("^(.*)[/\\]") or ".") .. "/?.lua;" .. package.path
local z = require("zerodds")

local failures = {}

local function hex(s)
  return (s:gsub(".", function(c) return string.format("%02x", c:byte()) end))
end

local function check(name, got, want)
  if got ~= want then
    failures[#failures + 1] = ("%s:\n    got  %s\n    want %s")
      :format(name, tostring(got), tostring(want))
  end
end

local function refuses(name, f)
  local ok = pcall(f)
  if ok then failures[#failures + 1] = name .. ": it was accepted" end
end

local function W(endian, encoding) return z.Writer.new(endian, encoding) end

-- --- the alignment cap the encoding sets ---------------------------------
--
-- tools/byte-gate/cases/ext_appendable: `@appendable struct Ext { long a;
-- double b; short c; }` with a = 7, b = 1.5, c = -3. XTypes 1.3 §7.4.1 puts
-- MAXALIGN at eight under XCDR1, so the `double` takes four octets of
-- padding there and none under XCDR2 — which is exactly the difference
-- between the two vendor rows.

local function extAppendable(endian, encoding, framed)
  local w = W(endian, encoding)
  local mark = framed and w:dheaderBegin() or 0
  w:putU32(7)
  w:putF64(1.5)
  w:putU16(-3)
  if framed then w:dheaderEnd(mark) end
  return hex(w:bytes())
end

-- Cyclone, RTI and Fast DDS all wrote these four, octet for octet.
check("ext_appendable xcdr1 le", extAppendable(z.LE, z.XCDR1, false),
  "0700000000000000000000000000f83ffdff")
check("ext_appendable xcdr1 be", extAppendable(z.BE, z.XCDR1, false),
  "00000007000000003ff8000000000000fffd")
check("ext_appendable xcdr2 le", extAppendable(z.LE, z.XCDR2, true),
  "0e00000007000000000000000000f83ffdff")
check("ext_appendable xcdr2 be", extAppendable(z.BE, z.XCDR2, true),
  "0000000e000000073ff8000000000000fffd")

-- The cap is the encoding's and not a constant.
check("max_align xcdr1", W(z.BE, z.XCDR1).maxAlign, 8)
check("max_align xcdr2", W(z.BE, z.XCDR2).maxAlign, 4)
refuses("a Writer with no encoding", function() return z.Writer.new(z.LE) end)
refuses("a Reader with no encoding", function() return z.Reader.new("", z.LE) end)

-- --- the 64-bit float ----------------------------------------------------

for _, tc in ipairs({
  { 1.5, "000000000000f83f" },                  -- the vendors' own sample
  { 0.0, "0000000000000000" },
  { -0.0, "0000000000000080" },                 -- a signed zero keeps its sign
  { 1.7976931348623157e308, "ffffffffffffef7f" },  -- MAX
  { 5e-324, "0100000000000000" },               -- the smallest subnormal
  { math.huge, "000000000000f07f" },
}) do
  local w = W(z.LE, z.XCDR2)
  w:putF64(tc[1])
  check(("putF64(%s)"):format(tostring(tc[1])), hex(w:bytes()), tc[2])
end

-- --- DHEADER -------------------------------------------------------------

do
  -- §7.4.3.5: three octets of content, so the DHEADER reads 3 and the four
  -- octets it occupies are not in it.
  local w = W(z.LE, z.XCDR2)
  local mark = w:dheaderBegin()
  w:putU8(1); w:putU8(2); w:putU8(3)
  w:dheaderEnd(mark)
  check("dheader counts the body", hex(w:bytes()), "03000000010203")
end

do
  local w = W(z.BE, z.XCDR2)
  w:dheaderEnd(w:dheaderBegin())
  check("dheader of an empty body", hex(w:bytes()), "00000000")
end

-- --- the member header, in every length code -----------------------------
--
-- tools/byte-gate/cases/ext_mutable: the same three members, `@mutable`.
-- Cyclone and RTI wrote the XCDR2 rows identically.

local function extMutableXcdr2(endian)
  local w = W(endian, z.XCDR2)
  local d = w:dheaderBegin()
  local m = w:emheaderBegin(0, false, z.LC2)
  w:putU32(7)
  w:emheaderEnd(m, z.LC2)
  m = w:emheaderBegin(1, false, z.LC3)
  w:putF64(1.5)
  w:emheaderEnd(m, z.LC3)
  m = w:emheaderBegin(2, false, z.LC1)
  w:putU16(-3)
  w:emheaderEnd(m, z.LC1)
  w:dheaderEnd(d)
  return hex(w:bytes())
end

check("ext_mutable xcdr2 le", extMutableXcdr2(z.LE),
  "1a000000000000200700000001000030000000000000f83f02000010fdff")
check("ext_mutable xcdr2 be", extMutableXcdr2(z.BE),
  "0000001a2000000000000007300000013ff800000000000010000002fffd")

-- tools/byte-gate/cases/pid_3f00: `@mutable struct PidEdge { @id(16127) long
-- a; @id(16128) long b; }`. Cyclone, RTI and Fast DDS agree on the XCDR2
-- row, which is where the member id sits inside the EMHEADER's low 28 bits.
do
  local w = W(z.LE, z.XCDR2)
  local d = w:dheaderBegin()
  local m = w:emheaderBegin(16127, false, z.LC2)
  w:putU32(7)
  w:emheaderEnd(m, z.LC2)
  m = w:emheaderBegin(16128, false, z.LC2)
  w:putU32(9)
  w:emheaderEnd(m, z.LC2)
  w:dheaderEnd(d)
  check("pid_3f00 xcdr2 le", hex(w:bytes()), "10000000ff3e002007000000003f002009000000")
end

-- tools/byte-gate/cases/lc_5_6_7: five collection members whose length codes
-- are LC5, LC5, LC6, LC7 and — where the two vendors part — LC4 for Cyclone
-- and LC3 for Fast DDS. Both are written here, because a binding that could
-- only produce one of them could not read the other's wire back.
local function lc567(tailCode)
  local w = W(z.LE, z.XCDR2)
  local d = w:dheaderBegin()
  local m = w:emheaderBegin(1, false, z.LC5)
  w:putString("abc")
  w:emheaderEnd(m, z.LC5)
  m = w:emheaderBegin(2, false, z.LC5)
  w:putSeqU8("\1\2\3")
  w:emheaderEnd(m, z.LC5)
  m = w:emheaderBegin(3, false, z.LC6)
  w:putU32(1); w:putU32(7)
  w:emheaderEnd(m, z.LC6)
  m = w:emheaderBegin(4, false, z.LC7)
  w:putU32(1); w:putF64(1.5)
  w:emheaderEnd(m, z.LC7)
  m = w:emheaderBegin(5, false, tailCode)
  w:putU32(2); w:putU16(4); w:putU16(5)
  w:emheaderEnd(m, tailCode)
  w:dheaderEnd(d)
  return hex(w:bytes())
end

local LC567_HEAD =
  "01000050" .. "04000000" .. "61626300" ..
  "02000050" .. "03000000" .. "01020300" ..
  "03000060" .. "01000000" .. "07000000" ..
  "04000070" .. "01000000" .. "000000000000f83f"

check("lc_5_6_7 xcdr2 le (Cyclone, LC4 tail)", lc567(z.LC4),
  "44000000" .. LC567_HEAD .. "05000040" .. "08000000" .. "02000000" .. "04000500")
check("lc_5_6_7 xcdr2 le (Fast DDS, LC3 tail)", lc567(z.LC3),
  "40000000" .. LC567_HEAD .. "05000030" .. "02000000" .. "04000500")

-- The must-understand flag is bit 31 and not bit 30: 0x80000000 | (2 << 28).
do
  local w = W(z.LE, z.XCDR2)
  local m = w:emheaderBegin(0, true, z.LC2)
  w:putU32(0)
  w:emheaderEnd(m, z.LC2)
  check("must-understand is bit 31", hex(w:bytes()), "000000a000000000")
end

-- --- the PL_CDR1 parameter list ------------------------------------------
--
-- tools/byte-gate/cases/ext_mutable, XCDR1. RTI's rows are the short-header
-- form this binding writes. The three things they pin: the `double`'s
-- parameter declares 8 and not 12, because its value aligns against its own
-- block; the `short`'s declares 4 and not 2, because the padding that brings
-- the next header back to a four-octet boundary is inside the length; and
-- the sentinel carries the must-understand flag, so it is 0x7F02.
--
-- Cyclone writes the same sample with an extended header on every parameter
-- (56 octets against RTI's 32) and Fast DDS declares 2 for the `short` and a
-- bare 0x3F02 sentinel. Three readings, all on the wire today; this binding
-- takes RTI's.

local function extMutableXcdr1(endian)
  local w = W(endian, z.XCDR1)
  local p = w:pidBegin(0, false)
  w:putU32(7)
  w:pidEnd(p)
  p = w:pidBegin(1, false)
  w:putF64(1.5)
  w:pidEnd(p)
  p = w:pidBegin(2, false)
  w:putU16(-3)
  w:pidEnd(p)
  w:pidListEnd()
  return hex(w:bytes())
end

check("ext_mutable xcdr1 le (RTI)", extMutableXcdr1(z.LE),
  "000004000700000001000800000000000000f83f02000400fdff0000027f0000")
check("ext_mutable xcdr1 be (RTI)", extMutableXcdr1(z.BE),
  "0000000400000007000100083ff800000000000000020004fffd00007f020000")

-- tools/byte-gate/cases/pid_3f00 under XCDR1: RTI and Fast DDS both carry
-- @id(16128) in the *short* header, which is what puts 0x3F00 at the top of
-- its range inclusive rather than one past it.
do
  local w = W(z.LE, z.XCDR1)
  local p = w:pidBegin(0x3EFF, false)
  w:putU32(7)
  w:pidEnd(p)
  p = w:pidBegin(0x3F00, false)
  w:putU32(9)
  w:pidEnd(p)
  w:pidListEnd()
  check("pid_3f00 xcdr1 le (RTI)", hex(w:bytes()), "ff3e040007000000003f040009000000027f0000")
end

-- The extended header, for an id past 0x3F00. The marker carries the
-- must-understand flag whatever the member says; the member's own flag is
-- bit 30 of the eMemberHeader.
do
  local w = W(z.LE, z.XCDR1)
  local p = w:pidExtendedBegin(16383, false)
  w:putU32(0xFFFFFFFE)
  w:pidEnd(p)
  w:pidListEnd()
  check("extended parameter header", hex(w:bytes()),
    "017f0800ff3f000004000000feffffff027f0000")
end

do
  local w = W(z.LE, z.XCDR1)
  local p = w:pidBegin(0, true)
  w:putU32(7)
  w:pidEnd(p)
  check("the parameter must-understand flag is bit 14", hex(w:bytes()), "0040040007000000")
end

-- --- the wide character and string ---------------------------------------
--
-- tools/byte-gate/cases/wchar_member: `@final struct WCh { wchar c; octet
-- tail; }` with c = U+0061 and tail = 0xAB. Cyclone and Fast DDS write two
-- octets in both encodings and RTI writes two under XCDR2; RTI's XCDR1
-- `wchar` is four octets, which is the one reading this binding does not
-- take.
for _, tc in ipairs({ { z.LE, "6100ab" }, { z.BE, "0061ab" } }) do
  for _, encoding in ipairs({ z.XCDR1, z.XCDR2 }) do
    local w = W(tc[1], encoding)
    w:putWchar(0x0061)
    w:putU8(0xAB)
    check("wchar_member", hex(w:bytes()), tc[2])
  end
end

-- A `wchar` aligns to two, so a `char` in front of it costs one octet of pad.
do
  local w = W(z.LE, z.XCDR2)
  w:putChar(string.byte("a"))
  w:putWchar(string.byte("z"))
  check("wchar aligns to two", hex(w:bytes()), "61007a00")
end

-- tools/byte-gate/cases/wstring_member: w = "ab", tail = 0xAB. Cyclone and
-- RTI-under-XCDR2 count **octets** (4) where Fast DDS counts code units (2).
-- Under big endian Cyclone leaves the units little-endian — it copies the
-- UTF-16 buffer — and RTI swaps them; this binding swaps, so its BE form is
-- RTI's.
do
  local w = W(z.LE, z.XCDR2)
  w:putWstring({ 0x0061, 0x0062 })
  w:putU8(0xAB)
  check("wstring_member le (Cyclone + RTI)", hex(w:bytes()), "0400000061006200ab")
end
do
  local w = W(z.BE, z.XCDR2)
  w:putWstring({ 0x0061, 0x0062 })
  w:putU8(0xAB)
  check("wstring_member be (RTI)", hex(w:bytes()), "0000000400610062ab")
end

-- tools/byte-gate/cases/wstring_nonbmp: U+1D11E as the surrogate pair
-- D834 DD1E. Cyclone and RTI both write four octets; Fast DDS writes one
-- unit, 0xD11E, which is neither half of the pair.
do
  local w = W(z.LE, z.XCDR2)
  w:putWstring({ 0xD834, 0xDD1E })
  w:putU8(0xAB)
  check("wstring_nonbmp le (Cyclone + RTI)", hex(w:bytes()), "0400000034d81eddab")
end
do
  local w = W(z.BE, z.XCDR2)
  w:putWstring({ 0xD834, 0xDD1E })
  w:putU8(0xAB)
  check("wstring_nonbmp be (RTI)", hex(w:bytes()), "00000004d834dd1eab")
end

do
  local w = W(z.LE, z.XCDR2)
  w:putWstring({})
  check("an empty wstring", hex(w:bytes()), "00000000")
end

-- --- the bitset anchor ---------------------------------------------------
--
-- tools/wire-oracle/cases/bitset_chain: `bitset B3` chains 8 + 8 + 4 bits
-- into one four-octet anchor, inside `@final struct BitChain { octet head;
-- B3 value; octet tail; }` with head = 1, a = 0x11, b = 0x22, c = 0x3 and
-- tail = 0xAB. Only Fast DDS produced octets — Cyclone's and RTI's IDL front
-- ends do not parse `bitset` at all, which is itself the measurement
-- recorded in vendor_bytes.rows.tsv.
for _, tc in ipairs({ { z.LE, "0100000011220300ab" }, { z.BE, "0100000000032211ab" } }) do
  local w = W(tc[1], z.XCDR2)
  w:putU8(1)
  w:putBitset(0x11 | (0x22 << 8) | (0x3 << 16), 4)
  w:putU8(0xAB)
  check("bitset_chain (Fast DDS)", hex(w:bytes()), tc[2])
end

-- tools/wire-oracle/cases/bitset_overwide: `bitfield<3> v` assigned 200. The
-- holder keeps three bits, 200 mod 8 is 0, and the anchor is one octet — so
-- it aligns to one and the `octet` behind it needs no padding.
for _, endian in ipairs({ z.LE, z.BE }) do
  local w = W(endian, z.XCDR2)
  w:putBitset(200 & 0x7, 1)
  w:putU8(0xAB)
  check("bitset_overwide (Fast DDS)", hex(w:bytes()), "00ab")
end

-- An eight-octet anchor is the one place a bitset differs between the two
-- encodings: the anchor's own alignment, capped at four under XCDR2.
do
  local w = W(z.LE, z.XCDR1)
  w:putU8(0xAB)
  w:putBitset(1, 8)
  check("an eight-octet anchor under XCDR1", hex(w:bytes()),
    "ab000000000000000100000000000000")
end
do
  local w = W(z.LE, z.XCDR2)
  w:putU8(0xAB)
  w:putBitset(1, 8)
  check("an eight-octet anchor under XCDR2", hex(w:bytes()), "ab0000000100000000000000")
end

-- --- the packed decimal --------------------------------------------------
--
-- crates/ast-layout/tests/fixtures/vendor_bytes.md §"`fixed<P,S>`, measured
-- against two CORBA ORBs": no DDS generator parses the declaration at all —
-- Cyclone, RTI, Fast DDS and OpenDDS's tao_idl each refuse it — so the
-- oracle is omniORB 4.3.3 and JacORB 3.9, through
-- `tools/wire-oracle/fixed-orbs.sh`. The struct below is theirs, member for
-- member, and the octets are omniORB's.
--
-- JacORB agrees on eight of the nine and writes one octet for `0.00` where
-- omniORB writes three: its `write_fixed` takes a `BigDecimal`, and
-- `new BigDecimal("0.00").precision()` is 1 in Java, so the declared width is
-- gone before the ORB sees the value. This binding takes omniORB's reading —
-- the octet count comes from the declared digits and not from the value.
do
  local w = W(z.LE, z.XCDR2)
  w:putU8(0x77)
  w:putFixed("123.45", 5, 2)
  w:putFixed("-123.45", 5, 2)
  w:putFixed("0.00", 5, 2)
  w:putFixed("12.34", 4, 2)
  w:putFixed("0.123", 3, 3)
  w:putFixed("7", 1, 0)
  w:putFixed("42", 2, 0)
  w:putFixed("1234567890123456789012345678901", 31, 0)
  w:putU8(0xAB)
  check("FixedAll (omniORB 4.3.3)", hex(w:bytes()),
    "77" .. "12345c" .. "12345d" .. "00000c" .. "01234c" .. "123c" .. "7c" ..
    "042c" .. "1234567890123456789012345678901c" .. "ab")
end

-- The octets do not swap: packed BCD is a byte sequence and not an integer,
-- which is why omniORB records the same string under `--endian be`. It is
-- also why the value is a decimal string: `fixed<31,0>` has thirty-one
-- significant digits and Lua's only number holds about sixteen.
do
  local le, be = W(z.LE, z.XCDR1), W(z.BE, z.XCDR2)
  le:putFixed("123.45", 5, 2)
  be:putFixed("123.45", 5, 2)
  check("fixed is independent of the byte order", hex(le:bytes()), hex(be:bytes()))
end

for _, tc in ipairs({
  { "1", 1, 0, "1c" }, { "9", 1, 0, "9c" }, { "-9", 1, 0, "9d" },
  { "-0", 1, 0, "0c" }, { "-0.000", 3, 3, "000c" },
  { "0.123", 3, 3, "123c" }, { "-0.001", 3, 3, "001d" },
  { "7.5", 9, 3, "000007500c" },
  { "-1234567890123456789012345678901", 31, 0, "1234567890123456789012345678901d" },
}) do
  local w = W(z.LE, z.XCDR2)
  w:putFixed(tc[1], tc[2], tc[3])
  check(("putFixed(%s, %d, %d)"):format(tc[1], tc[2], tc[3]), hex(w:bytes()), tc[4])
  check(("putFixed(%s, %d, %d) octet count"):format(tc[1], tc[2], tc[3]),
    #w:bytes(), (tc[2] + 2) // 2)
end

-- `omniidl` refuses the same three shapes, in its own words: `fixed<0,0>` is
-- `Size must be at least 1`, `fixed<2,5>` is `Fixed point scale factor is
-- greater than the number of digits`, `fixed<32,0>` is `Fixed point values
-- may not have more than 31 digits`.
refuses("fixed<0,0>", function() return z.fixedOctets("1", 0, 0) end)
refuses("fixed<32,0>", function() return z.fixedOctets("1", 32, 0) end)
refuses("fixed<2,5>", function() return z.fixedOctets("1.0", 2, 5) end)
refuses("seven integer digits into six", function() return z.fixedOctets("1234567", 6, 0) end)
refuses("four fraction digits into three", function() return z.fixedOctets("1.2345", 6, 3) end)
refuses("not a decimal", function() return z.fixedOctets("twelve", 4, 0) end)
refuses("no digits at all", function() return z.fixedOctets("", 4, 0) end)

-- --- the ends of every integer range -------------------------------------
--
-- `putU16(70000)` used to answer `7011` in nine of the fifteen bindings: a
-- frame of the right length holding a different number. Both spellings of an
-- argument are taken — the unsigned pattern and the signed IDL value — and
-- nothing outside their union is.

for _, tc in ipairs({
  { 8, "putU8",
    { { 0, "00" }, { -1, "ff" }, { -128, "80" }, { 127, "7f" }, { 255, "ff" } },
    { 256, -129, 300, 70000 } },
  { 16, "putU16",
    { { 0, "0000" }, { -1, "ffff" }, { -32768, "0080" }, { 32767, "ff7f" },
      { 65535, "ffff" } },
    { 65536, -32769, 70000 } },
  { 32, "putU32",
    { { 0, "00000000" }, { -1, "ffffffff" }, { -2147483648, "00000080" },
      { 4294967295, "ffffffff" } },
    { 4294967296, -2147483649 } },
  { 64, "putU64",
    { { 0, "0000000000000000" }, { -1, "ffffffffffffffff" },
      { math.mininteger, "0000000000000080" },
      { -2, "feffffffffffffff" },
      { 9007199254740993, "0100000000002000" } },
    {} },
}) do
  local bits, call = tc[1], tc[2]
  for _, ok in ipairs(tc[3]) do
    local w = W(z.LE, z.XCDR2)
    w[call](w, ok[1])
    check(("%s(%d)"):format(call, ok[1]), hex(w:bytes()), ok[2])
  end
  for _, bad in ipairs(tc[4]) do
    refuses(("%s(%d)"):format(call, bad), function()
      local w = W(z.LE, z.XCDR2)
      w[call](w, bad)
    end)
  end
end

-- Lua's `^` returns a float whatever its arguments, so `2^53 + 1` is
-- `9007199254740992.0` — one less than the literal beside it. `string.pack`
-- takes a float whose value is integral without a word, so an integer write
-- that did not look at `math.type` would put a plausible, wrong number on
-- the wire.
check("2^53 + 1 is a float in Lua", math.type(2^53 + 1), "float")
check("2^53 + 1 is not the integer beside it", 2^53 + 1 == 9007199254740993, false)
refuses("putU64 of a float", function() W(z.LE, z.XCDR2):putU64(2^53 + 1) end)
refuses("putU32 of a float", function() W(z.LE, z.XCDR2):putU32(1.0) end)
refuses("putU8 of a float", function() W(z.LE, z.XCDR2):putU8(1.5) end)
do
  local w = W(z.LE, z.XCDR2)
  w:putU64(9007199254740993)  -- the integer literal, exact
  check("putU64(2^53 + 1) as an integer", hex(w:bytes()), "0100000000002000")
end

-- Lua's integer *is* 64 bits, so a 64-bit write has no value outside its
-- range to refuse: 0xFFFFFFFFFFFFFFFF and -1 are the same eight octets and
-- both spellings reach them.
check("0xFFFFFFFFFFFFFFFF is -1 in Lua", 0xFFFFFFFFFFFFFFFF, -1)
do
  local w = W(z.LE, z.XCDR2)
  w:putU64(0xFFFFFFFFFFFFFFFF)
  check("putU64(0xFFFFFFFFFFFFFFFF)", hex(w:bytes()), "ffffffffffffffff")
end

-- `char` and `wchar` at the ends of their code point ranges. U+0161's low
-- octet is 0x61, which is `a`: a truncating writer produces a frame of the
-- right length holding a plausible, wrong character.
do
  local w = W(z.LE, z.XCDR2)
  w:putChar(0x00)
  w:putChar(0xFF)
  check("char at both ends", hex(w:bytes()), "00ff")
end
refuses("char U+0100", function() W(z.LE, z.XCDR2):putChar(0x100) end)
refuses("char U+0161", function() W(z.LE, z.XCDR2):putChar(0x161) end)
refuses("char below zero", function() W(z.LE, z.XCDR2):putChar(-1) end)
do
  local w = W(z.LE, z.XCDR2)
  w:putWchar(0x0000)
  w:putWchar(0xFFFF)
  check("wchar at both ends", hex(w:bytes()), "0000ffff")
end
refuses("wchar U+10000", function() W(z.LE, z.XCDR2):putWchar(0x10000) end)

-- The header fields at the ends of their own ranges.
W(z.LE, z.XCDR2):emheaderBegin(0x0FFFFFFF, false, z.LC2)
refuses("a 29-bit member id",
  function() W(z.LE, z.XCDR2):emheaderBegin(0x10000000, false, z.LC2) end)
refuses("length code 8", function() W(z.LE, z.XCDR2):emheaderBegin(0, false, 8) end)
refuses("length code -1", function() W(z.LE, z.XCDR2):emheaderBegin(0, false, -1) end)
refuses("LC2 over a body that is not four octets", function()
  local w = W(z.LE, z.XCDR2)
  local m = w:emheaderBegin(0, false, z.LC2)
  w:putU16(1)
  w:emheaderEnd(m, z.LC2)
end)
W(z.LE, z.XCDR1):pidBegin(0x3F00, false)
refuses("a short header past 0x3F00", function() W(z.LE, z.XCDR1):pidBegin(0x3F01, false) end)
refuses("a bitset anchor of three", function() W(z.LE, z.XCDR2):putBitset(0, 3) end)
refuses("a holder wider than its anchor", function() W(z.LE, z.XCDR2):putBitset(256, 1) end)

-- --- the tail ------------------------------------------------------------
--
-- tools/byte-gate/cases/tail_pad: `@final struct Odd { long a; short b; }`
-- with a = 1, b = 2. All four vendors stop at six octets in both encodings —
-- a `@final` aggregate is not padded out to its own alignment.
for _, tc in ipairs({ { z.LE, "010000000200" }, { z.BE, "000000010002" } }) do
  for _, encoding in ipairs({ z.XCDR1, z.XCDR2 }) do
    local w = W(tc[1], encoding)
    w:putU32(1)
    w:putU16(2)
    check("tail_pad", hex(w:bytes()), tc[2])
  end
end

-- --- the reading direction -----------------------------------------------
--
-- Every write has a reader, and the vendor's own octets go in: a writer with
-- no matching reader is not an endpoint binding.

for _, encoding in ipairs({ z.XCDR1, z.XCDR2 }) do
  for _, endian in ipairs({ z.LE, z.BE }) do
    local w = W(endian, encoding)
    w:putU8(0xFF)
    w:putBool(true)
    w:putChar(0xFF)
    w:putU16(0xFFFF)
    w:putU32(0xFFFFFFFF)
    w:putU64(-1)
    w:putF32(-3.5)
    w:putF64(1.5)
    w:putWchar(0xFFFF)
    w:putString("gr\xc3\xbc\xc3\x9fe")
    w:putSeqU8("\xDE\xAD")
    w:putWstring({ 0xD83D, 0xDE00 })
    w:putBitset(0xBC614E00000FFFFF, 8)
    w:putFixed("-000000.001", 9, 3)

    local buf = w:bytes()
    local r = z.Reader.new(buf, endian, encoding)
    check("round trip u8", r:getU8(), 0xFF)
    check("round trip bool", r:getBool(), true)
    check("round trip char", r:getChar(), 0xFF)
    check("round trip u16", r:getU16(), 0xFFFF)
    check("round trip u32", r:getU32(), 0xFFFFFFFF)
    check("round trip u64", r:getU64(), -1)
    check("round trip f32", r:getF32(), -3.5)
    check("round trip f64", r:getF64(), 1.5)
    check("round trip wchar", r:getWchar(), 0xFFFF)
    check("round trip string", r:getString(), "gr\xc3\xbc\xc3\x9fe")
    check("round trip seq_u8", r:getSeqU8(), "\xDE\xAD")
    local units = r:getWstring()
    check("round trip wstring units", #units, 2)
    check("round trip wstring[1]", units[1], 0xD83D)
    check("round trip wstring[2]", units[2], 0xDE00)
    check("round trip bitset", r:getBitset(8), 0xBC614E00000FFFFF)
    check("round trip fixed", r:getFixed(9, 3), "-000000.001")
    check("round trip consumed everything", r.pos, #buf)
  end
end

-- The vendors' own octets, straight in. `ext_mutable` under XCDR2 is
-- Cyclone's and RTI's identical row; reading it back is what makes this a
-- binding and not a writer.
local function unhex(s)
  return (s:gsub("%x%x", function(pair) return string.char(tonumber(pair, 16)) end))
end

do
  local bytes = unhex("1a000000000000200700000001000030000000000000f83f02000010fdff")
  local r = z.Reader.new(bytes, z.LE, z.XCDR2)
  check("vendor dheader", r:dheaderRead(), 26)
  for _, want in ipairs({ { 0, z.LC2, 4 }, { 1, z.LC3, 8 }, { 2, z.LC1, 2 } }) do
    local id, mu, lc, extent = r:emheaderRead()
    check("vendor emheader id", id, want[1])
    check("vendor emheader lc", lc, want[2])
    check("vendor emheader extent", extent, want[3])
    check("vendor emheader mu", mu, false)
    if lc == z.LC2 then check("vendor member a", r:getU32(), 7)
    elseif lc == z.LC3 then check("vendor member b", r:getF64(), 1.5)
    else check("vendor member c", r:getU16(), 0xFFFD) end
  end
  check("vendor xcdr2 fully consumed", r.pos, #bytes)
end

-- RTI's XCDR1 row for the same sample, through the parameter-list reader.
do
  local bytes = unhex("000004000700000001000800000000000000f83f02000400fdff0000027f0000")
  local r = z.Reader.new(bytes, z.LE, z.XCDR1)
  for _, want in ipairs({ { 0, 4 }, { 1, 8 }, { 2, 4 } }) do
    local id, mu, length, listEnd = r:pidRead()
    check("vendor pid end", listEnd, false)
    check("vendor pid id", id, want[1])
    check("vendor pid length", length, want[2])
    check("vendor pid mu", mu, false)
    local body = r.pos
    if id == 0 then check("vendor parameter a", r:getU32(), 7)
    elseif id == 1 then check("vendor parameter b", r:getF64(), 1.5)
    else check("vendor parameter c", r:getU16(), 0xFFFD) end
    r:pidSkip(body, length)
  end
  local _, _, _, listEnd = r:pidRead()
  check("vendor sentinel ends the list", listEnd, true)
  check("vendor xcdr1 fully consumed", r.pos, #bytes)
end

-- Cyclone's lc_5_6_7 row, whose LC5, LC6 and LC7 members take their length
-- from the body's own leading word.
do
  local r = z.Reader.new(unhex(lc567(z.LC4)), z.LE, z.XCDR2)
  r:dheaderRead()
  for _, want in ipairs({ { 1, z.LC5, 8 }, { 2, z.LC5, 7 }, { 3, z.LC6, 8 },
                          { 4, z.LC7, 12 }, { 5, z.LC4, 8 } }) do
    local id, _, lc, extent = r:emheaderRead()
    check("lc_5_6_7 read id", id, want[1])
    check("lc_5_6_7 read lc", lc, want[2])
    check("lc_5_6_7 read extent", extent, want[3])
    r.pos = r.pos + extent
  end
end

if #failures == 0 then
  print("endpoints/lua: the surface holds, against Cyclone, RTI, Fast DDS and omniORB")
else
  for _, f in ipairs(failures) do io.stderr:write(f, "\n") end
  io.stderr:write(("%d failure(s)\n"):format(#failures))
  os.exit(1)
end
