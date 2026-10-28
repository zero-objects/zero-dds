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
# Connext, Fast DDS, omniORB and JacORB wrote these; comparing this SDK against
# a reference file this project also wrote would only show that two halves of
# one opinion agree.
#
# Where the vendors disagree the test says which reading this binding takes and
# which it does not, because a wire question with two answers in the field is a
# measurement and not a defect.
#
# Each operation is exercised at the ends of its range and not only in the
# middle. Elixir's integers are arbitrary precision, so nothing in the language
# refuses a value too wide for its holder; the guards are the only place that
# difference is visible, and the cases below are what makes them a measurement.
#
# Run with `elixir -r lib/zerodds.ex surface_test.exs`.

alias ZeroDDS.Wire

defmodule Surface do
  @moduledoc false

  def start, do: Agent.start_link(fn -> [] end, name: __MODULE__)

  def check(name, got, want) do
    if got != want do
      Agent.update(__MODULE__, fn acc ->
        ["#{name}:\n    got  #{inspect(got)}\n    want #{inspect(want)}" | acc]
      end)
    end
  end

  def refuses(name, fun) do
    try do
      fun.()
      Agent.update(__MODULE__, fn acc -> ["#{name}: it was accepted" | acc] end)
    rescue
      ArgumentError -> :ok
      FunctionClauseError -> :ok
    end
  end

  def failures, do: Agent.get(__MODULE__, &Enum.reverse/1)
end

{:ok, _} = Surface.start()

hex = fn bin -> Base.encode16(bin, case: :lower) end
unhex = fn s -> Base.decode16!(s, case: :lower) end
w_of = fn endian, encoding -> Wire.writer(endian, encoding) end

# --- the alignment cap the encoding sets ---------------------------------
#
# tools/byte-gate/cases/ext_appendable: `@appendable struct Ext { long a;
# double b; short c; }` with a = 7, b = 1.5, c = -3. XTypes 1.3 §7.4.1 puts
# MAXALIGN at eight under XCDR1, so the `double` takes four octets of padding
# there and none under XCDR2 — which is exactly the difference between the two
# vendor rows.

ext_appendable = fn endian, encoding, framed ->
  w = w_of.(endian, encoding)
  {w, mark} = if framed, do: Wire.dheader_begin(w), else: {w, 0}

  w =
    w
    |> Wire.put_u32(7)
    |> Wire.put_f64(1.5)
    |> Wire.put_u16(-3)

  w = if framed, do: Wire.dheader_end(w, mark), else: w
  hex.(Wire.bytes(w))
end

# Cyclone, RTI and Fast DDS all wrote these four, octet for octet.
Surface.check("ext_appendable xcdr1 le", ext_appendable.(:little, :xcdr1, false),
  "0700000000000000000000000000f83ffdff")

Surface.check("ext_appendable xcdr1 be", ext_appendable.(:big, :xcdr1, false),
  "00000007000000003ff8000000000000fffd")

Surface.check("ext_appendable xcdr2 le", ext_appendable.(:little, :xcdr2, true),
  "0e00000007000000000000000000f83ffdff")

Surface.check("ext_appendable xcdr2 be", ext_appendable.(:big, :xcdr2, true),
  "0000000e000000073ff8000000000000fffd")

# The cap is the encoding's and not a constant.
Surface.check("max_align xcdr1", Wire.writer(:big, :xcdr1).max_align, 8)
Surface.check("max_align xcdr2", Wire.writer(:big, :xcdr2).max_align, 4)
Surface.refuses("a writer with no encoding", fn -> Wire.writer(:little, nil) end)
Surface.refuses("a reader with no encoding", fn -> Wire.reader(<<>>, :little, nil) end)

# --- the 64-bit float ----------------------------------------------------
#
# The BEAM has no infinity and no NaN, so those two of binary64's values have
# no Elixir literal and are not here. The rest of the range is.

for {value, want} <- [
      # the vendors' own sample
      {1.5, "000000000000f83f"},
      {0.0, "0000000000000000"},
      # a signed zero keeps its sign
      {-0.0, "0000000000000080"},
      # MAX
      {1.7976931348623157e308, "ffffffffffffef7f"},
      # the smallest subnormal
      {5.0e-324, "0100000000000000"}
    ] do
  w = Wire.put_f64(w_of.(:little, :xcdr2), value)
  Surface.check("put_f64(#{value})", hex.(Wire.bytes(w)), want)
end

# --- DHEADER -------------------------------------------------------------

# §7.4.3.5: three octets of content, so the DHEADER reads 3 and the four
# octets it occupies are not in it.
{w, mark} = Wire.dheader_begin(w_of.(:little, :xcdr2))

w =
  w
  |> Wire.put_u8(1)
  |> Wire.put_u8(2)
  |> Wire.put_u8(3)
  |> Wire.dheader_end(mark)

Surface.check("dheader counts the body", hex.(Wire.bytes(w)), "03000000010203")

{e, emark} = Wire.dheader_begin(w_of.(:big, :xcdr2))
Surface.check("dheader of an empty body", hex.(Wire.bytes(Wire.dheader_end(e, emark))), "00000000")

# --- the member header, in every length code -----------------------------
#
# tools/byte-gate/cases/ext_mutable: the same three members, `@mutable`.
# Cyclone and RTI wrote the XCDR2 rows identically.

ext_mutable_xcdr2 = fn endian ->
  w = w_of.(endian, :xcdr2)
  {w, d} = Wire.dheader_begin(w)
  {w, m} = Wire.emheader_begin(w, 0, false, Wire.lc2())
  w = Wire.emheader_end(Wire.put_u32(w, 7), m, Wire.lc2())
  {w, m} = Wire.emheader_begin(w, 1, false, Wire.lc3())
  w = Wire.emheader_end(Wire.put_f64(w, 1.5), m, Wire.lc3())
  {w, m} = Wire.emheader_begin(w, 2, false, Wire.lc1())
  w = Wire.emheader_end(Wire.put_u16(w, -3), m, Wire.lc1())
  hex.(Wire.bytes(Wire.dheader_end(w, d)))
end

Surface.check("ext_mutable xcdr2 le", ext_mutable_xcdr2.(:little),
  "1a000000000000200700000001000030000000000000f83f02000010fdff")

Surface.check("ext_mutable xcdr2 be", ext_mutable_xcdr2.(:big),
  "0000001a2000000000000007300000013ff800000000000010000002fffd")

# tools/byte-gate/cases/pid_3f00: `@mutable struct PidEdge { @id(16127) long a;
# @id(16128) long b; }`. Cyclone, RTI and Fast DDS agree on the XCDR2 row,
# which is where the member id sits inside the EMHEADER's low 28 bits.
{w, d} = Wire.dheader_begin(w_of.(:little, :xcdr2))
{w, m} = Wire.emheader_begin(w, 16_127, false, Wire.lc2())
w = Wire.emheader_end(Wire.put_u32(w, 7), m, Wire.lc2())
{w, m} = Wire.emheader_begin(w, 16_128, false, Wire.lc2())
w = Wire.emheader_end(Wire.put_u32(w, 9), m, Wire.lc2())

Surface.check("pid_3f00 xcdr2 le", hex.(Wire.bytes(Wire.dheader_end(w, d))),
  "10000000ff3e002007000000003f002009000000")

# tools/byte-gate/cases/lc_5_6_7: five collection members whose length codes
# are LC5, LC5, LC6, LC7 and — where the two vendors part — LC4 for Cyclone and
# LC3 for Fast DDS. Both are written here, because a binding that could only
# produce one of them could not read the other's wire back.
lc_5_6_7 = fn tail_code ->
  w = w_of.(:little, :xcdr2)
  {w, d} = Wire.dheader_begin(w)
  {w, m} = Wire.emheader_begin(w, 1, false, Wire.lc5())
  w = Wire.emheader_end(Wire.put_string(w, "abc"), m, Wire.lc5())
  {w, m} = Wire.emheader_begin(w, 2, false, Wire.lc5())
  w = Wire.emheader_end(Wire.put_seq_u8(w, <<1, 2, 3>>), m, Wire.lc5())
  {w, m} = Wire.emheader_begin(w, 3, false, Wire.lc6())
  w = Wire.emheader_end(w |> Wire.put_u32(1) |> Wire.put_u32(7), m, Wire.lc6())
  {w, m} = Wire.emheader_begin(w, 4, false, Wire.lc7())
  w = Wire.emheader_end(w |> Wire.put_u32(1) |> Wire.put_f64(1.5), m, Wire.lc7())
  {w, m} = Wire.emheader_begin(w, 5, false, tail_code)

  w =
    Wire.emheader_end(
      w |> Wire.put_u32(2) |> Wire.put_u16(4) |> Wire.put_u16(5),
      m,
      tail_code
    )

  hex.(Wire.bytes(Wire.dheader_end(w, d)))
end

lc_567_head =
  "01000050" <>
    "04000000" <>
    "61626300" <>
    "02000050" <>
    "03000000" <>
    "01020300" <>
    "03000060" <>
    "01000000" <> "07000000" <> "04000070" <> "01000000" <> "000000000000f83f"

Surface.check("lc_5_6_7 xcdr2 le (Cyclone, LC4 tail)", lc_5_6_7.(Wire.lc4()),
  "44000000" <> lc_567_head <> "05000040" <> "08000000" <> "02000000" <> "04000500")

Surface.check("lc_5_6_7 xcdr2 le (Fast DDS, LC3 tail)", lc_5_6_7.(Wire.lc3()),
  "40000000" <> lc_567_head <> "05000030" <> "02000000" <> "04000500")

# The must-understand flag is bit 31 and not bit 30: 0x80000000 | (2 <<< 28).
{w, m} = Wire.emheader_begin(w_of.(:little, :xcdr2), 0, true, Wire.lc2())

Surface.check("must-understand is bit 31",
  hex.(Wire.bytes(Wire.emheader_end(Wire.put_u32(w, 0), m, Wire.lc2()))),
  "000000a000000000")

# --- the PL_CDR1 parameter list ------------------------------------------
#
# tools/byte-gate/cases/ext_mutable, XCDR1. RTI's rows are the short-header
# form this binding writes. The three things they pin: the `double`'s parameter
# declares 8 and not 12, because its value aligns against its own block; the
# `short`'s declares 4 and not 2, because the padding that brings the next
# header back to a four-octet boundary is inside the length; and the sentinel
# carries the must-understand flag, so it is 0x7F02.
#
# Cyclone writes the same sample with an extended header on every parameter
# (56 octets against RTI's 32) and Fast DDS declares 2 for the `short` and a
# bare 0x3F02 sentinel. Three readings, all on the wire today; this binding
# takes RTI's.

ext_mutable_xcdr1 = fn endian ->
  w = w_of.(endian, :xcdr1)
  {w, p} = Wire.pid_begin(w, 0, false)
  w = Wire.pid_end(Wire.put_u32(w, 7), p)
  {w, p} = Wire.pid_begin(w, 1, false)
  w = Wire.pid_end(Wire.put_f64(w, 1.5), p)
  {w, p} = Wire.pid_begin(w, 2, false)
  w = Wire.pid_end(Wire.put_u16(w, -3), p)
  hex.(Wire.bytes(Wire.pid_list_end(w)))
end

Surface.check("ext_mutable xcdr1 le (RTI)", ext_mutable_xcdr1.(:little),
  "000004000700000001000800000000000000f83f02000400fdff0000027f0000")

Surface.check("ext_mutable xcdr1 be (RTI)", ext_mutable_xcdr1.(:big),
  "0000000400000007000100083ff800000000000000020004fffd00007f020000")

# tools/byte-gate/cases/pid_3f00 under XCDR1: RTI and Fast DDS both carry
# @id(16128) in the *short* header, which is what puts 0x3F00 at the top of its
# range inclusive rather than one past it.
{w, p} = Wire.pid_begin(w_of.(:little, :xcdr1), 0x3EFF, false)
w = Wire.pid_end(Wire.put_u32(w, 7), p)
{w, p} = Wire.pid_begin(w, 0x3F00, false)
w = Wire.pid_end(Wire.put_u32(w, 9), p)

Surface.check("pid_3f00 xcdr1 le (RTI)", hex.(Wire.bytes(Wire.pid_list_end(w))),
  "ff3e040007000000003f040009000000027f0000")

# The extended header, for an id past 0x3F00. The marker carries the
# must-understand flag whatever the member says; the member's own flag is bit
# 30 of the eMemberHeader.
{w, p} = Wire.pid_extended_begin(w_of.(:little, :xcdr1), 16_383, false)
w = Wire.pid_end(Wire.put_u32(w, 0xFFFFFFFE), p)

Surface.check("extended parameter header", hex.(Wire.bytes(Wire.pid_list_end(w))),
  "017f0800ff3f000004000000feffffff027f0000")

{w, p} = Wire.pid_begin(w_of.(:little, :xcdr1), 0, true)

Surface.check("the parameter must-understand flag is bit 14",
  hex.(Wire.bytes(Wire.pid_end(Wire.put_u32(w, 7), p))), "0040040007000000")

# --- the wide character and string ---------------------------------------
#
# tools/byte-gate/cases/wchar_member: `@final struct WCh { wchar c; octet
# tail; }` with c = U+0061 and tail = 0xAB. Cyclone and Fast DDS write two
# octets in both encodings and RTI writes two under XCDR2; RTI's XCDR1 `wchar`
# is four octets, which is the one reading this binding does not take.
for {endian, want} <- [{:little, "6100ab"}, {:big, "0061ab"}],
    encoding <- [:xcdr1, :xcdr2] do
  w = w_of.(endian, encoding) |> Wire.put_wchar(0x0061) |> Wire.put_u8(0xAB)
  Surface.check("wchar_member #{endian} #{encoding}", hex.(Wire.bytes(w)), want)
end

# A `wchar` aligns to two, so a `char` in front of it costs one octet of pad.
w = w_of.(:little, :xcdr2) |> Wire.put_char(?a) |> Wire.put_wchar(?z)
Surface.check("wchar aligns to two", hex.(Wire.bytes(w)), "61007a00")

# tools/byte-gate/cases/wstring_member: w = "ab", tail = 0xAB. Cyclone and
# RTI-under-XCDR2 count **octets** (4) where Fast DDS counts code units (2).
# Under big endian Cyclone leaves the units little-endian — it copies the
# UTF-16 buffer — and RTI swaps them; this binding swaps, so its BE form is
# RTI's.
#
# tools/byte-gate/cases/wstring_nonbmp: U+1D11E as the surrogate pair
# D834 DD1E. Cyclone and RTI both write four octets; Fast DDS writes one unit,
# 0xD11E, which is neither half of the pair.
for {endian, units, want, name} <- [
      {:little, [0x0061, 0x0062], "0400000061006200ab", "wstring_member le (Cyclone + RTI)"},
      {:big, [0x0061, 0x0062], "0000000400610062ab", "wstring_member be (RTI)"},
      {:little, [0xD834, 0xDD1E], "0400000034d81eddab", "wstring_nonbmp le (Cyclone + RTI)"},
      {:big, [0xD834, 0xDD1E], "00000004d834dd1eab", "wstring_nonbmp be (RTI)"}
    ] do
  w = w_of.(endian, :xcdr2) |> Wire.put_wstring(units) |> Wire.put_u8(0xAB)
  Surface.check(name, hex.(Wire.bytes(w)), want)
end

Surface.check("an empty wstring", hex.(Wire.bytes(Wire.put_wstring(w_of.(:little, :xcdr2), []))),
  "00000000")

# --- the bitset anchor ---------------------------------------------------
#
# tools/wire-oracle/cases/bitset_chain: `bitset B3` chains 8 + 8 + 4 bits into
# one four-octet anchor, inside `@final struct BitChain { octet head; B3 value;
# octet tail; }` with head = 1, a = 0x11, b = 0x22, c = 0x3 and tail = 0xAB.
# Only Fast DDS produced octets — Cyclone's and RTI's IDL front ends do not
# parse `bitset` at all, which is itself the measurement recorded in
# vendor_bytes.rows.tsv.
for {endian, want} <- [{:little, "0100000011220300ab"}, {:big, "0100000000032211ab"}] do
  w =
    w_of.(endian, :xcdr2)
    |> Wire.put_u8(1)
    |> Wire.put_bitset(Bitwise.bor(0x11, Bitwise.bor(Bitwise.bsl(0x22, 8), Bitwise.bsl(0x3, 16))), 4)
    |> Wire.put_u8(0xAB)

  Surface.check("bitset_chain #{endian} (Fast DDS)", hex.(Wire.bytes(w)), want)
end

# tools/wire-oracle/cases/bitset_overwide: `bitfield<3> v` assigned 200. The
# holder keeps three bits, 200 mod 8 is 0, and the anchor is one octet — so it
# aligns to one and the `octet` behind it needs no padding.
for endian <- [:little, :big] do
  w = w_of.(endian, :xcdr2) |> Wire.put_bitset(Bitwise.band(200, 0x7), 1) |> Wire.put_u8(0xAB)
  Surface.check("bitset_overwide #{endian} (Fast DDS)", hex.(Wire.bytes(w)), "00ab")
end

# An eight-octet anchor is the one place a bitset differs between the two
# encodings: the anchor's own alignment, capped at four under XCDR2.
for {encoding, want} <- [
      {:xcdr1, "ab000000000000000100000000000000"},
      {:xcdr2, "ab0000000100000000000000"}
    ] do
  w = w_of.(:little, encoding) |> Wire.put_u8(0xAB) |> Wire.put_bitset(1, 8)
  Surface.check("an eight-octet anchor under #{encoding}", hex.(Wire.bytes(w)), want)
end

# --- the packed decimal --------------------------------------------------
#
# crates/ast-layout/tests/fixtures/vendor_bytes.md §"`fixed<P,S>`, measured
# against two CORBA ORBs": no DDS generator parses the declaration at all —
# Cyclone, RTI, Fast DDS and OpenDDS's tao_idl each refuse it — so the oracle
# is omniORB 4.3.3 and JacORB 3.9, through `tools/wire-oracle/fixed-orbs.sh`.
# The struct below is theirs, member for member, and the octets are omniORB's.
#
# JacORB agrees on eight of the nine and writes one octet for `0.00` where
# omniORB writes three: its `write_fixed` takes a `BigDecimal`, and
# `new BigDecimal("0.00").precision()` is 1 in Java, so the declared width is
# gone before the ORB sees the value. This binding takes omniORB's reading —
# the octet count comes from the declared digits and not from the value.
w =
  w_of.(:little, :xcdr2)
  |> Wire.put_u8(0x77)
  |> Wire.put_fixed("123.45", 5, 2)
  |> Wire.put_fixed("-123.45", 5, 2)
  |> Wire.put_fixed("0.00", 5, 2)
  |> Wire.put_fixed("12.34", 4, 2)
  |> Wire.put_fixed("0.123", 3, 3)
  |> Wire.put_fixed("7", 1, 0)
  |> Wire.put_fixed("42", 2, 0)
  |> Wire.put_fixed("1234567890123456789012345678901", 31, 0)
  |> Wire.put_u8(0xAB)

Surface.check("FixedAll (omniORB 4.3.3)", hex.(Wire.bytes(w)),
  "77" <>
    "12345c" <>
    "12345d" <>
    "00000c" <>
    "01234c" <>
    "123c" <> "7c" <> "042c" <> "1234567890123456789012345678901c" <> "ab")

# The octets do not swap: packed BCD is a byte sequence and not an integer,
# which is why omniORB records the same string under `--endian be`. It is also
# why the value is a decimal string: `fixed<31,0>` has thirty-one significant
# digits and the BEAM's float holds about sixteen.
Surface.check("fixed is independent of the byte order",
  hex.(Wire.bytes(Wire.put_fixed(w_of.(:little, :xcdr1), "123.45", 5, 2))),
  hex.(Wire.bytes(Wire.put_fixed(w_of.(:big, :xcdr2), "123.45", 5, 2))))

for {value, digits, scale, want} <- [
      {"1", 1, 0, "1c"},
      {"9", 1, 0, "9c"},
      {"-9", 1, 0, "9d"},
      {"-0", 1, 0, "0c"},
      {"-0.000", 3, 3, "000c"},
      {"0.123", 3, 3, "123c"},
      {"-0.001", 3, 3, "001d"},
      {"7.5", 9, 3, "000007500c"},
      {"-1234567890123456789012345678901", 31, 0, "1234567890123456789012345678901d"}
    ] do
  w = Wire.put_fixed(w_of.(:little, :xcdr2), value, digits, scale)
  Surface.check("put_fixed(#{value}, #{digits}, #{scale})", hex.(Wire.bytes(w)), want)

  Surface.check("put_fixed(#{value}, #{digits}, #{scale}) octet count",
    byte_size(Wire.bytes(w)), div(digits + 2, 2))
end

# `omniidl` refuses the same three shapes, in its own words: `fixed<0,0>` is
# `Size must be at least 1`, `fixed<2,5>` is `Fixed point scale factor is
# greater than the number of digits`, `fixed<32,0>` is `Fixed point values may
# not have more than 31 digits`.
for {name, value, digits, scale} <- [
      {"fixed<0,0>", "1", 0, 0},
      {"fixed<32,0>", "1", 32, 0},
      {"fixed<2,5>", "1.0", 2, 5},
      {"seven integer digits into six", "1234567", 6, 0},
      {"four fraction digits into three", "1.2345", 6, 3},
      {"not a decimal", "twelve", 4, 0},
      {"no digits at all", "", 4, 0}
    ] do
  Surface.refuses(name, fn -> Wire.fixed_octets(value, digits, scale) end)
end

# --- the ends of every integer range -------------------------------------
#
# `put_u16(w, 70000)` used to answer `7011`: `<<70000::little-16>>` is
# `<<0x70, 0x11>>` and the BEAM says nothing. Both spellings of an argument are
# taken — the unsigned pattern and the signed IDL value — and nothing outside
# their union is.

for {bits, fun, ok, refused} <- [
      {8, &Wire.put_u8/2,
       [{0, "00"}, {-1, "ff"}, {-128, "80"}, {127, "7f"}, {255, "ff"}],
       [256, -129, 300, 70_000]},
      {16, &Wire.put_u16/2,
       [{0, "0000"}, {-1, "ffff"}, {-32_768, "0080"}, {32_767, "ff7f"}, {65_535, "ffff"}],
       [65_536, -32_769, 70_000]},
      {32, &Wire.put_u32/2,
       [{0, "00000000"}, {-1, "ffffffff"}, {-2_147_483_648, "00000080"},
        {4_294_967_295, "ffffffff"}],
       [4_294_967_296, -2_147_483_649]},
      {64, &Wire.put_u64/2,
       [{0, "0000000000000000"}, {-1, "ffffffffffffffff"},
        {-9_223_372_036_854_775_808, "0000000000000080"},
        {18_446_744_073_709_551_615, "ffffffffffffffff"},
        {9_007_199_254_740_993, "0100000000002000"}],
       [18_446_744_073_709_551_616, -9_223_372_036_854_775_809]}
    ] do
  for {value, want} <- ok do
    Surface.check("put_u#{bits}(#{value})", hex.(Wire.bytes(fun.(w_of.(:little, :xcdr2), value))), want)
  end

  for value <- refused do
    Surface.refuses("put_u#{bits}(#{value})", fn -> fun.(w_of.(:little, :xcdr2), value) end)
  end
end

# The BEAM's integers have no upper bound at all, so a bitstring field is the
# only thing that ever narrows one — silently. These two are what the guard is
# for: without it the first is eight zero octets and the second is `0x2C`.
Surface.check("2^64 truncates to zero in a bitstring", <<18_446_744_073_709_551_616::little-64>>,
  <<0, 0, 0, 0, 0, 0, 0, 0>>)

Surface.check("300 truncates to 0x2C in a bitstring", <<300::8>>, <<0x2C>>)

# 2^53 + 1 is exact as an integer and rounds as a float. Elixir keeps both
# apart, and the write takes the integer.
Surface.check("2^53 + 1 rounds as a float", trunc(9_007_199_254_740_993.0), 9_007_199_254_740_992)
Surface.refuses("put_u64 of a float", fn -> Wire.put_u64(w_of.(:little, :xcdr2), 9.007199254740993e15) end)

# `char` and `wchar` at the ends of their code point ranges. U+0161's low octet
# is 0x61, which is `a`: a truncating writer produces a frame of the right
# length holding a plausible, wrong character.
w = w_of.(:little, :xcdr2) |> Wire.put_char(0x00) |> Wire.put_char(0xFF)
Surface.check("char at both ends", hex.(Wire.bytes(w)), "00ff")
Surface.refuses("char U+0100", fn -> Wire.put_char(w_of.(:little, :xcdr2), 0x100) end)
Surface.refuses("char U+0161", fn -> Wire.put_char(w_of.(:little, :xcdr2), 0x161) end)
Surface.refuses("char below zero", fn -> Wire.put_char(w_of.(:little, :xcdr2), -1) end)

w = w_of.(:little, :xcdr2) |> Wire.put_wchar(0x0000) |> Wire.put_wchar(0xFFFF)
Surface.check("wchar at both ends", hex.(Wire.bytes(w)), "0000ffff")
Surface.refuses("wchar U+10000", fn -> Wire.put_wchar(w_of.(:little, :xcdr2), 0x10000) end)

# The header fields at the ends of their own ranges.
Wire.emheader_begin(w_of.(:little, :xcdr2), 0x0FFFFFFF, false, Wire.lc2())

Surface.refuses("a 29-bit member id", fn ->
  Wire.emheader_begin(w_of.(:little, :xcdr2), 0x10000000, false, Wire.lc2())
end)

Surface.refuses("length code 8", fn -> Wire.emheader_begin(w_of.(:little, :xcdr2), 0, false, 8) end)
Surface.refuses("length code -1", fn -> Wire.emheader_begin(w_of.(:little, :xcdr2), 0, false, -1) end)

Surface.refuses("LC2 over a body that is not four octets", fn ->
  {w, m} = Wire.emheader_begin(w_of.(:little, :xcdr2), 0, false, Wire.lc2())
  Wire.emheader_end(Wire.put_u16(w, 1), m, Wire.lc2())
end)

Wire.pid_begin(w_of.(:little, :xcdr1), 0x3F00, false)

Surface.refuses("a short header past 0x3F00", fn ->
  Wire.pid_begin(w_of.(:little, :xcdr1), 0x3F01, false)
end)

Surface.refuses("a bitset anchor of three", fn -> Wire.put_bitset(w_of.(:little, :xcdr2), 0, 3) end)

Surface.refuses("a holder wider than its anchor", fn ->
  Wire.put_bitset(w_of.(:little, :xcdr2), 256, 1)
end)

# --- the tail ------------------------------------------------------------
#
# tools/byte-gate/cases/tail_pad: `@final struct Odd { long a; short b; }` with
# a = 1, b = 2. All four vendors stop at six octets in both encodings — a
# `@final` aggregate is not padded out to its own alignment.
for {endian, want} <- [{:little, "010000000200"}, {:big, "000000010002"}],
    encoding <- [:xcdr1, :xcdr2] do
  w = w_of.(endian, encoding) |> Wire.put_u32(1) |> Wire.put_u16(2)
  Surface.check("tail_pad #{endian} #{encoding}", hex.(Wire.bytes(w)), want)
end

# --- the reading direction -----------------------------------------------
#
# Every write has a reader, and the vendor's own octets go in: a writer with no
# matching reader is not an endpoint binding.

for encoding <- [:xcdr1, :xcdr2], endian <- [:little, :big] do
  w =
    w_of.(endian, encoding)
    |> Wire.put_u8(0xFF)
    |> Wire.put_bool(true)
    |> Wire.put_char(0xFF)
    |> Wire.put_u16(0xFFFF)
    |> Wire.put_u32(0xFFFFFFFF)
    |> Wire.put_u64(0xFFFFFFFFFFFFFFFF)
    |> Wire.put_f32(-3.5)
    |> Wire.put_f64(1.5)
    |> Wire.put_wchar(0xFFFF)
    |> Wire.put_string("grüße")
    |> Wire.put_seq_u8(<<0xDE, 0xAD>>)
    |> Wire.put_wstring([0xD83D, 0xDE00])
    |> Wire.put_bitset(0xBC614E00000FFFFF, 8)
    |> Wire.put_fixed("-000000.001", 9, 3)

  buf = Wire.bytes(w)
  r = Wire.reader(buf, endian, encoding)
  {v, r} = Wire.get_u8(r)
  Surface.check("round trip u8", v, 0xFF)
  {v, r} = Wire.get_bool(r)
  Surface.check("round trip bool", v, true)
  {v, r} = Wire.get_char(r)
  Surface.check("round trip char", v, 0xFF)
  {v, r} = Wire.get_u16(r)
  Surface.check("round trip u16", v, 0xFFFF)
  {v, r} = Wire.get_u32(r)
  Surface.check("round trip u32", v, 0xFFFFFFFF)
  {v, r} = Wire.get_u64(r)
  Surface.check("round trip u64", v, 0xFFFFFFFFFFFFFFFF)
  {v, r} = Wire.get_f32(r)
  Surface.check("round trip f32", v, -3.5)
  {v, r} = Wire.get_f64(r)
  Surface.check("round trip f64", v, 1.5)
  {v, r} = Wire.get_wchar(r)
  Surface.check("round trip wchar", v, 0xFFFF)
  {v, r} = Wire.get_string(r)
  Surface.check("round trip string", v, "grüße")
  {v, r} = Wire.get_seq_u8(r)
  Surface.check("round trip seq_u8", v, <<0xDE, 0xAD>>)
  {v, r} = Wire.get_wstring(r)
  Surface.check("round trip wstring", v, [0xD83D, 0xDE00])
  {v, r} = Wire.get_bitset(r, 8)
  Surface.check("round trip bitset", v, 0xBC614E00000FFFFF)
  {v, r} = Wire.get_fixed(r, 9, 3)
  Surface.check("round trip fixed", v, "-000000.001")
  Surface.check("round trip consumed everything", r.pos, byte_size(buf))
end

# The vendors' own octets, straight in. `ext_mutable` under XCDR2 is Cyclone's
# and RTI's identical row; reading it back is what makes this a binding and not
# a writer.
buf = unhex.("1a000000000000200700000001000030000000000000f83f02000010fdff")
r = Wire.reader(buf, :little, :xcdr2)
{body, r} = Wire.dheader_read(r)
Surface.check("vendor dheader", body, 26)

r =
  Enum.reduce([{0, Wire.lc2(), 4}, {1, Wire.lc3(), 8}, {2, Wire.lc1(), 2}], r, fn
    {want_id, want_lc, want_extent}, r ->
      {id, mu, lc, extent, r} = Wire.emheader_read(r)
      Surface.check("vendor emheader id", id, want_id)
      Surface.check("vendor emheader lc", lc, want_lc)
      Surface.check("vendor emheader extent", extent, want_extent)
      Surface.check("vendor emheader mu", mu, false)

      cond do
        lc == Wire.lc2() ->
          {v, r} = Wire.get_u32(r)
          Surface.check("vendor member a", v, 7)
          r

        lc == Wire.lc3() ->
          {v, r} = Wire.get_f64(r)
          Surface.check("vendor member b", v, 1.5)
          r

        true ->
          {v, r} = Wire.get_u16(r)
          Surface.check("vendor member c", v, 0xFFFD)
          r
      end
  end)

Surface.check("vendor xcdr2 fully consumed", r.pos, byte_size(buf))

# RTI's XCDR1 row for the same sample, through the parameter-list reader.
buf = unhex.("000004000700000001000800000000000000f83f02000400fdff0000027f0000")
r = Wire.reader(buf, :little, :xcdr1)

r =
  Enum.reduce([{0, 4}, {1, 8}, {2, 4}], r, fn {want_id, want_len}, r ->
    {id, mu, length, list_end, r} = Wire.pid_read(r)
    Surface.check("vendor pid end", list_end, false)
    Surface.check("vendor pid id", id, want_id)
    Surface.check("vendor pid length", length, want_len)
    Surface.check("vendor pid mu", mu, false)
    body_start = r.pos

    r =
      cond do
        id == 0 ->
          {v, r} = Wire.get_u32(r)
          Surface.check("vendor parameter a", v, 7)
          r

        id == 1 ->
          {v, r} = Wire.get_f64(r)
          Surface.check("vendor parameter b", v, 1.5)
          r

        true ->
          {v, r} = Wire.get_u16(r)
          Surface.check("vendor parameter c", v, 0xFFFD)
          r
      end

    Wire.pid_skip(r, body_start, length)
  end)

{_, _, _, list_end, r} = Wire.pid_read(r)
Surface.check("vendor sentinel ends the list", list_end, true)
Surface.check("vendor xcdr1 fully consumed", r.pos, byte_size(buf))

# Cyclone's lc_5_6_7 row, whose LC5, LC6 and LC7 members take their length from
# the body's own leading word.
r = Wire.reader(unhex.(lc_5_6_7.(Wire.lc4())), :little, :xcdr2)
{_, r} = Wire.dheader_read(r)

Enum.reduce(
  [
    {1, Wire.lc5(), 8},
    {2, Wire.lc5(), 7},
    {3, Wire.lc6(), 8},
    {4, Wire.lc7(), 12},
    {5, Wire.lc4(), 8}
  ],
  r,
  fn {want_id, want_lc, want_extent}, r ->
    {id, _, lc, extent, r} = Wire.emheader_read(r)
    Surface.check("lc_5_6_7 read id", id, want_id)
    Surface.check("lc_5_6_7 read lc", lc, want_lc)
    Surface.check("lc_5_6_7 read extent", extent, want_extent)
    %{r | pos: r.pos + extent}
  end
)

case Surface.failures() do
  [] ->
    IO.puts("endpoints/elixir: the surface holds, against Cyclone, RTI, Fast DDS and omniORB")

  items ->
    Enum.each(items, &IO.puts(:stderr, &1))
    IO.puts(:stderr, "#{length(items)} failure(s)")
    System.halt(1)
end
