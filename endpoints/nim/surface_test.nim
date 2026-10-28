# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 ZeroDDS Contributors
#
# The Nim endpoint SDK's XCDR surface, measured against octets four other
# vendors wrote.
#
# ## Why the expected octets are not ours
#
# A writer compared against a reference file this repository generated is two
# readings of one opinion. Every wire expectation in this file is a row of
# `crates/ast-layout/tests/fixtures/vendor_bytes.rows.tsv` — produced by
# `tools/byte-gate/gate.sh` from Eclipse Cyclone DDS, RTI Connext, eProsima Fast
# DDS and OpenDDS running their own code generators over
# `tools/byte-gate/cases/<case>/type.idl` and `fill.inc` — or a
# `tests/interop-results/<case>/<vendor>/` file written by omniORB or Fast DDS.
# The vendor and the case are named at every expectation, and where the vendors
# disagree the divergence is named too rather than averaged away.
#
# The closing block asks every new operation what it does at 0, at −1, at the
# minimum, at the maximum and one past it.

import std/strutils
import ./zerodds

var checks = 0

proc hexBytes(s: string): seq[byte] =
  result = newSeq[byte](s.len div 2)
  for i in 0 ..< result.len:
    result[i] = byte(parseHexInt(s[2 * i .. 2 * i + 1]))

proc toHexText(b: seq[byte]): string =
  result = ""
  for x in b:
    result.add(toHex(int(x), 2).toLowerAscii)

proc wire(w: Writer, want: string, label: string) =
  let got = toHexText(w.bytes())
  doAssert got == want, label & ": wrote " & got & ", the vendor wrote " & want
  inc checks

template expectRefusal(label: string, body: untyped) =
  ## A refusal is measured by running the write and requiring the exception, not
  ## by inspecting the buffer afterwards: a binding that wrote the wrong octets
  ## and then raised would pass an inspection of the exception alone.
  var refused = false
  try:
    body
  except WireError:
    refused = true
  doAssert refused, label & ": the refusal did not happen"
  inc checks

# -- tail_pad: @final struct Odd { long a; short b; }; a = 1, b = 2 ------------
#
# Cyclone, RTI and Fast DDS all write six octets under both encodings and both
# byte orders. The case exists because a writer that rounded the stream up to its
# own alignment would write eight, and every one of the three says it does not.

proc tailPad() =
  for (endian, want) in [(eLE, "010000000200"), (eBE, "000000010002")]:
    for encoding in [eXcdr1, eXcdr2]:
      var w = initWriter(endian, encoding)
      w.putU32(1'u32)
      w.putU16(2'u16)
      wire(w, want, "tail_pad " & $endian & " " & $encoding)
      var r = initReader(w.bytes(), endian, encoding)
      doAssert r.getU32() == 1'u32
      doAssert r.getU16() == 2'u16

# -- ext_appendable: @appendable struct Ext { long a; double b; short c; } -----
#
# a = 7, b = 1.5, c = −3. The two encodings differ in both of the things this
# commit added: under XCDR1 there is no DHEADER and the `double` aligns to eight
# (offset 4..8 is padding); under XCDR2 a DHEADER of 14 precedes the body and the
# `double` aligns to four. Cyclone, RTI and Fast DDS agree on all four rows.

proc extAppendable() =
  for (endian, want) in [(eLE, "0700000000000000000000000000f83ffdff"),
                         (eBE, "00000007000000003ff8000000000000fffd")]:
    var w = initWriter(endian, eXcdr1)
    w.putU32(7'u32)
    w.putF64(1.5'f64)
    w.putU16(0xFFFD'u16)
    wire(w, want, "ext_appendable xcdr1 " & $endian)
    var r = initReader(w.bytes(), endian, eXcdr1)
    doAssert r.getU32() == 7'u32
    doAssert r.getF64() == 1.5
    doAssert r.getU16() == 0xFFFD'u16

  for (endian, want) in [(eLE, "0e00000007000000000000000000f83ffdff"),
                         (eBE, "0000000e000000073ff8000000000000fffd")]:
    var w = initWriter(endian, eXcdr2)
    let body = w.dheaderBegin()
    w.putU32(7'u32)
    w.putF64(1.5'f64)
    w.putU16(0xFFFD'u16)
    w.dheaderEnd(body)
    wire(w, want, "ext_appendable xcdr2 " & $endian)
    var r = initReader(w.bytes(), endian, eXcdr2)
    doAssert r.dheaderRead() == 14'u32
    doAssert r.getU32() == 7'u32
    doAssert r.getF64() == 1.5
    doAssert r.getU16() == 0xFFFD'u16

# -- ext_mutable under PL_CDR2 -------------------------------------------------
#
# Cyclone, RTI and Fast DDS write the identical thirty octets. The three length
# codes are the three member widths: LC2 for the `long`, LC3 for the `double`,
# LC1 for the `short`, and none of the three reserves a NEXTINT.

proc extMutableXcdr2() =
  for (endian, want) in [
      (eLE, "1a000000000000200700000001000030000000000000f83f02000010fdff"),
      (eBE, "0000001a2000000000000007300000013ff800000000000010000002fffd")]:
    var w = initWriter(endian, eXcdr2)
    let body = w.dheaderBegin()
    var m = w.emHeaderBegin(0'u32, false, lc2)
    w.putU32(7'u32)
    w.emHeaderEnd(m, lc2)
    m = w.emHeaderBegin(1'u32, false, lc3)
    w.putF64(1.5'f64)
    w.emHeaderEnd(m, lc3)
    m = w.emHeaderBegin(2'u32, false, lc1)
    w.putU16(0xFFFD'u16)
    w.emHeaderEnd(m, lc1)
    w.dheaderEnd(body)
    wire(w, want, "ext_mutable xcdr2 " & $endian)

    # And back: the extent each code announces is the member's own width, with
    # nothing consumed beyond the header.
    var r = initReader(w.bytes(), endian, eXcdr2)
    doAssert r.dheaderRead() == 26'u32
    var member = r.emHeaderRead()
    doAssert member.id == 0'u32 and member.lc == lc2 and member.extent == 4'u32
    doAssert r.getU32() == 7'u32
    member = r.emHeaderRead()
    doAssert member.id == 1'u32 and member.extent == 8'u32
    doAssert r.getF64() == 1.5
    member = r.emHeaderRead()
    doAssert member.id == 2'u32 and member.extent == 2'u32
    doAssert r.getU16() == 0xFFFD'u16

# The same sample under PL_CDR1, in the short parameter header RTI writes. Fast
# DDS differs in two places: it closes the list with the bare 0x3F02 where RTI
# and Cyclone set the must-understand bit and write 0x7F02, and it declares two
# octets for the `short` where RTI declares four. This SDK writes the majority
# form on both.
#
# The `double` is the reason the alignment origin exists: its header sits at
# offset 8..12 and the value follows at 12 with no padding, because section
# 7.4.1.2 aligns a parameter's value against its own block. Measured from the
# stream the value would land at 16 and the header would declare twelve; all
# three vendors declare eight.

proc extMutableXcdr1Short() =
  for (endian, want) in [
      (eLE, "000004000700000001000800000000000000f83f02000400fdff0000027f0000"),
      (eBE, "0000000400000007000100083ff800000000000000020004fffd00007f020000")]:
    var w = initWriter(endian, eXcdr1)
    var p = w.pidBegin(0'u16, false)
    w.putU32(7'u32)
    w.pidEnd(p)
    p = w.pidBegin(1'u16, false)
    w.putF64(1.5'f64)
    w.pidEnd(p)
    p = w.pidBegin(2'u16, false)
    w.putU16(0xFFFD'u16)
    w.pidEnd(p)
    w.pidListEnd()
    wire(w, want, "ext_mutable xcdr1 short pid " & $endian)

    var r = initReader(w.bytes(), endian, eXcdr1)
    var param = r.pidRead()
    doAssert param.id == 0'u32 and param.length == 4'u32
    doAssert r.getU32() == 7'u32
    r.pidSkip(4, param.length)
    param = r.pidRead()
    doAssert param.id == 1'u32 and param.length == 8'u32
    doAssert r.getF64() == 1.5
    r.pidSkip(12, param.length)
    param = r.pidRead()
    doAssert param.id == 2'u32 and param.length == 4'u32
    doAssert r.getU16() == 0xFFFD'u16
    r.pidSkip(24, param.length)
    doAssert r.pidRead().listEnd

# The same sample again, in the twelve-octet extended parameter header Cyclone
# writes for every mutable member under PL_CDR1. Fifty-six octets, and the
# must-understand bit is on the PID_EXTENDED marker (0x7F01) while the member's
# own eMemberHeader carries none — which is what makes reading the flag off the
# marker wrong.

proc extMutableXcdr1Extended() =
  for (endian, want) in [
      (eLE, "017f0800000000000400000007000000017f08000100000008000000" &
            "000000000000f83f017f08000200000004000000fdff0000027f0000"),
      (eBE, "7f0100080000000000000004000000077f01000800000001000000" &
            "083ff80000000000007f0100080000000200000004fffd00007f020000")]:
    var w = initWriter(endian, eXcdr1)
    var p = w.pidExtendedBegin(0'u32, false)
    w.putU32(7'u32)
    w.pidEnd(p)
    p = w.pidExtendedBegin(1'u32, false)
    w.putF64(1.5'f64)
    w.pidEnd(p)
    p = w.pidExtendedBegin(2'u32, false)
    w.putU16(0xFFFD'u16)
    w.pidEnd(p)
    w.pidListEnd()
    wire(w, want, "ext_mutable xcdr1 extended pid " & $endian)

    var r = initReader(w.bytes(), endian, eXcdr1)
    let first = r.pidRead()
    doAssert first.id == 0'u32
    doAssert not first.mustUnderstand, "the marker's flag is not the member's"
    doAssert first.length == 4'u32
    doAssert r.getU32() == 7'u32

# -- pid_3f00 ------------------------------------------------------------------
#
# @mutable struct PidEdge { @id(16127) long a; @id(16128) long b; }. 16128 is
# 0x3F00, the highest id a short parameter header can carry: 0x3F01 is
# PID_EXTENDED and 0x3F02 is PID_LIST_END. RTI writes both ids in the short form,
# which is the boundary this case exists to hold.

proc pid3f00() =
  for (endian, want) in [(eLE, "ff3e040007000000003f040009000000027f0000"),
                         (eBE, "3eff0004000000073f000004000000097f020000")]:
    var w = initWriter(endian, eXcdr1)
    var p = w.pidBegin(0x3EFF'u16, false)
    w.putU32(7'u32)
    w.pidEnd(p)
    p = w.pidBegin(0x3F00'u16, false)
    w.putU32(9'u32)
    w.pidEnd(p)
    w.pidListEnd()
    wire(w, want, "pid_3f00 xcdr1 " & $endian)

  for (endian, want) in [(eLE, "10000000ff3e002007000000003f002009000000"),
                         (eBE, "0000001020003eff0000000720003f0000000009")]:
    var w = initWriter(endian, eXcdr2)
    let body = w.dheaderBegin()
    var m = w.emHeaderBegin(0x3EFF'u32, false, lc2)
    w.putU32(7'u32)
    w.emHeaderEnd(m, lc2)
    m = w.emHeaderBegin(0x3F00'u32, false, lc2)
    w.putU32(9'u32)
    w.emHeaderEnd(m, lc2)
    w.dheaderEnd(body)
    wire(w, want, "pid_3f00 xcdr2 " & $endian)

# -- lc_5_6_7 ------------------------------------------------------------------
#
# @mutable struct M { @id(1) string s; @id(2) sequence<octet> b;
#                     @id(3) sequence<long> l; @id(4) sequence<double> d;
#                     @id(5) sequence<short> h; };
#
# Cyclone's seventy-two octets. LC5, LC6 and LC7 take the member's own leading
# word as the length — LC6 counting four-octet elements and LC7 eight — and
# reserve no NEXTINT; only `sequence<short>` gets LC4 and a NEXTINT of 8. A
# writer that emitted a NEXTINT for LC5..LC7 as well would put every following
# member four octets late, which is the defect this case holds.
#
# RTI did not compile the case (`fill.inc:6:54: 'struct DDS_OctetSeq' has no
# member named '_buffer'`) and Fast DDS writes LC4 where Cyclone writes LC6 for
# `sequence<long>`; Cyclone's rows are the ones reproduced.

proc lc567() =
  for (endian, want) in [
      (eLE, "440000000100005004000000616263000200005003000000010203" &
            "000300006001000000070000000400007001000000000000000000f83f" &
            "05000040080000000200000004000500"),
      (eBE, "00000044500000010000000461626300500000020000000301020300" &
            "60000003000000010000000770000004000000013ff8000000000000" &
            "40000005000000080000000200040005")]:
    var w = initWriter(endian, eXcdr2)
    let body = w.dheaderBegin()
    var m = w.emHeaderBegin(1'u32, false, lc5)
    w.putString("abc")
    w.emHeaderEnd(m, lc5)
    m = w.emHeaderBegin(2'u32, false, lc5)
    w.putSeqU8(@[1'u8, 2'u8, 3'u8])
    w.emHeaderEnd(m, lc5)
    m = w.emHeaderBegin(3'u32, false, lc6)
    w.putU32(1'u32)
    w.putU32(7'u32)
    w.emHeaderEnd(m, lc6)
    m = w.emHeaderBegin(4'u32, false, lc7)
    w.putU32(1'u32)
    w.putF64(1.5'f64)
    w.emHeaderEnd(m, lc7)
    m = w.emHeaderBegin(5'u32, false, lc4)
    w.putU32(2'u32)
    w.putU16(4'u16)
    w.putU16(5'u16)
    w.emHeaderEnd(m, lc4)
    w.dheaderEnd(body)
    wire(w, want, "lc_5_6_7 xcdr2 " & $endian)

    # Reading Cyclone's own octets: each code's extent counts what the spec says
    # it counts, and LC5..LC7 leave the leading word in place.
    var r = initReader(w.bytes(), endian, eXcdr2)
    doAssert r.dheaderRead() == 68'u32
    var member = r.emHeaderRead()
    doAssert member.lc == lc5 and member.extent == 8'u32 # 4 + 4
    doAssert r.getString() == "abc"
    member = r.emHeaderRead()
    doAssert member.extent == 7'u32 # 4 + 3
    doAssert r.getSeqU8() == @[1'u8, 2'u8, 3'u8]
    member = r.emHeaderRead()
    doAssert member.lc == lc6 and member.extent == 8'u32 # 4 + 4×1
    doAssert r.getU32() == 1'u32
    doAssert r.getU32() == 7'u32
    member = r.emHeaderRead()
    doAssert member.lc == lc7 and member.extent == 12'u32 # 4 + 8×1
    doAssert r.getU32() == 1'u32
    doAssert r.getF64() == 1.5
    member = r.emHeaderRead()
    doAssert member.lc == lc4 and member.extent == 8'u32

# -- the wide character and the wide string ------------------------------------
#
# wchar_member: @final struct WCh { wchar c; octet tail; }, c = U+0061,
# tail = 0xAB. Cyclone and Fast DDS write two octets under both encodings and RTI
# does under XCDR2. RTI's XCDR1 `wchar_t` is four octets wide (`61000000ab`) —
# the one vendor answer this SDK does not reproduce, and it is RTI alone.
#
# wstring_member / wstring_nonbmp: @final struct WStr { wstring w; octet tail; }.
# The little-endian rows are Cyclone's under both encodings and RTI's under
# XCDR2: the length word counts four octets for two code units, there is no
# terminator and no byte-order mark. Fast DDS writes the unit *count* there
# (`02000000`) and truncates U+1D11E to one unit (`010000001ed1`); neither is
# reproduced. The big-endian row is RTI's under XCDR2 alone — Cyclone byte-swaps
# the length word but leaves the code units little-endian
# (`0000000461006200ab`), which no reading of section 7.4.1 supports.

proc wide() =
  for (endian, want) in [(eLE, "6100ab"), (eBE, "0061ab")]:
    for encoding in [eXcdr1, eXcdr2]:
      var w = initWriter(endian, encoding)
      w.putWchar(0x61'u32)
      w.putU8(0xAB'u8)
      wire(w, want, "wchar_member " & $endian & " " & $encoding)
      var r = initReader(w.bytes(), endian, encoding)
      doAssert r.getWchar() == 0x61'u32
      doAssert r.getU8() == 0xAB'u8

  var le = initWriter(eLE, eXcdr2)
  le.putWstring(@[0x61'u16, 0x62'u16])
  le.putU8(0xAB'u8)
  wire(le, "0400000061006200ab", "wstring_member le") # cyclone, rti xcdr2

  var be = initWriter(eBE, eXcdr2)
  be.putWstring(@[0x61'u16, 0x62'u16])
  be.putU8(0xAB'u8)
  wire(be, "0000000400610062ab", "wstring_member be") # rti xcdr2

  var r = initReader(be.bytes(), eBE, eXcdr2)
  doAssert r.getWstring() == @[0x61'u16, 0x62'u16]
  doAssert r.getU8() == 0xAB'u8

  var nonbmp = initWriter(eLE, eXcdr2)
  nonbmp.putWstring(@[0xD834'u16, 0xDD1E'u16])
  nonbmp.putU8(0xAB'u8)
  wire(nonbmp, "0400000034d81eddab", "wstring_nonbmp le") # cyclone, rti xcdr2

  var nonbmpBe = initWriter(eBE, eXcdr2)
  nonbmpBe.putWstring(@[0xD834'u16, 0xDD1E'u16])
  nonbmpBe.putU8(0xAB'u8)
  wire(nonbmpBe, "00000004d834dd1eab", "wstring_nonbmp be") # rti xcdr2

  var rb = initReader(nonbmp.bytes(), eLE, eXcdr2)
  doAssert rb.getWstring() == @[0xD834'u16, 0xDD1E'u16]

# -- bitset --------------------------------------------------------------------
#
# `bitset Packed { bitfield<3> low; bitfield<5> high; }` with low = 5 and
# high = 17 packs into one octet: 5 or (17 shl 3) = 0x8D. Fast DDS wrote `8dab`
# for all four configurations of `tests/interop-corpus/bitset-basic` (Cyclone,
# RTI and OpenDDS have no `bitset` in their IDL front ends and left the cell
# empty).
#
# `bitset Wide { bitfield<20> first; bitfield<20> second; bitfield<24> third; }`
# with first = 1048575, second = 0, third = 12345678 packs into eight octets:
# 0xBC614E00000FFFFF. The anchor's width is the alignment, which is what makes it
# a separate call from `putU64`.

proc bitsets() =
  var one = initWriter(eLE, eXcdr1)
  one.putBitset(uint64(5 or (17 shl 3)), 1)
  one.putU8(0xAB'u8)
  wire(one, "8dab", "bitset-basic")
  var r1 = initReader(one.bytes(), eLE, eXcdr1)
  doAssert r1.getBitset(1) == 0x8D'u64

  for (endian, want) in [(eLE, "ffff0f00004e61bcab"), (eBE, "bc614e00000fffffab")]:
    for encoding in [eXcdr1, eXcdr2]:
      var w = initWriter(endian, encoding)
      w.putBitset(0xBC614E00000FFFFF'u64, 8)
      w.putU8(0xAB'u8)
      wire(w, want, "bitset-multiword " & $endian & " " & $encoding)
      var r = initReader(w.bytes(), endian, encoding)
      doAssert r.getBitset(8) == 0xBC614E00000FFFFF'u64

# -- fixed<9,3> ----------------------------------------------------------------
#
# `tests/interop-corpus/fixed-basic`: amount = "123456.789",
# debt = "-000000.001", tail = 0xAB. omniORB wrote `123456789c 000000001d ab` in
# both byte orders — BCD has no byte order, which is the property the two
# identical files state. JacORB writes `123456789c 1dab`: it drops the leading
# zero octets of the negative value, so its frame is four octets shorter and
# cannot be parsed against the declared digit count. omniORB's is the form
# reproduced.

proc fixedBasic() =
  for endian in [eLE, eBE]:
    var w = initWriter(endian, eXcdr1)
    w.putFixed("123456.789", 9, 3)
    w.putFixed("-000000.001", 9, 3)
    w.putU8(0xAB'u8)
    wire(w, "123456789c000000001dab", "fixed-basic " & $endian)
    var r = initReader(w.bytes(), endian, eXcdr1)
    doAssert r.getFixed(9, 3) == "123456.789"
    doAssert r.getFixed(9, 3) == "-000000.001"
    doAssert r.getU8() == 0xAB'u8

# ============================ the range edges ============================
#
# The last sweep found four defects and every one of them was at an end of a
# range, not in the middle: an Ada 83 divide that truncated toward zero for every
# negative argument, a Zig frame writer that ran past its output buffer, and nine
# of fifteen bindings answering `put_u16(70000)` with `7011`. So each operation
# is asked here what it does at 0, at −1, at the minimum, at the maximum, and one
# past the maximum.

proc integerEdges() =
  # `putU16` used to take an `int` and mask, so 70000 wrote `7011`. It takes a
  # `uint16` now: Nim refuses the literal at compile time and a runtime `int`
  # at the conversion, so the truncation has no spelling left. What is
  # measurable here is that the extremes go out whole.
  var w = initWriter(eLE, eXcdr2)
  w.putU8(0'u8)
  w.putU8(255'u8)
  w.putU16(0'u16)
  w.putU16(65535'u16)
  w.putU32(0'u32)
  w.putU32(4294967295'u32)
  w.putU64(0'u64)
  w.putU64(18446744073709551615'u64)
  # 00 ff | 0000 ffff | 0000 (the u32's padding) | 00000000 ffffffff
  # | 0000000000000000 ffffffffffffffff
  wire(w, "00ff0000ffff000000000000ffffffff" &
          "0000000000000000ffffffffffffffff", "the integer extremes")

  var r = initReader(w.bytes(), eLE, eXcdr2)
  doAssert r.getU8() == 0'u8
  doAssert r.getU8() == 255'u8
  doAssert r.getU16() == 0'u16
  doAssert r.getU16() == 65535'u16
  doAssert r.getU32() == 0'u32
  doAssert r.getU32() == 4294967295'u32
  doAssert r.getU64() == 0'u64
  doAssert r.getU64() == 18446744073709551615'u64

  # The signed extremes, as the two's complement a caller converts them to.
  var s = initWriter(eLE, eXcdr2)
  s.putU8(cast[uint8](-1'i8))
  s.putU8(cast[uint8](low(int8)))
  s.putU16(cast[uint16](-1'i16))
  s.putU16(cast[uint16](low(int16)))
  s.putU32(cast[uint32](-1'i32))
  s.putU32(cast[uint32](low(int32)))
  s.putU64(cast[uint64](-1'i64))
  s.putU64(cast[uint64](low(int64)))
  wire(s, "ff80ffff00800000ffffffff00000080" &
          "ffffffffffffffff0000000000000080", "the signed extremes")

proc u64AlignmentEdge() =
  # The Go column's third commit: `getU64` passed four where `putU64` passed
  # eight, so 0xFFFFFFFFFFFFFFFF read back as 0xFFFFFFFF00000000 with no error
  # anywhere. One octet first, so the value is not already aligned.
  var w = initWriter(eLE, eXcdr1)
  w.putU8(0xAB'u8)
  w.putU64(0xFFFF_FFFF_FFFF_FFFF'u64)
  wire(w, "ab00000000000000ffffffffffffffff", "putU64 under XCDR1")
  var r = initReader(w.bytes(), eLE, eXcdr1)
  doAssert r.getU8() == 0xAB'u8
  doAssert r.getU64() == 0xFFFF_FFFF_FFFF_FFFF'u64

proc floatEdges() =
  for v in [0.0'f64, -0.0'f64, 1.5'f64, -1.5'f64,
            2.2250738585072014e-308'f64, 1.7976931348623157e308'f64]:
    var w = initWriter(eBE, eXcdr1)
    w.putF64(v)
    var r = initReader(w.bytes(), eBE, eXcdr1)
    doAssert cast[uint64](r.getF64()) == cast[uint64](v)
    inc checks

proc characterEdges() =
  var w = initWriter(eLE, eXcdr2)
  w.putChar(0'u32)     # the minimum
  w.putChar(0xFF'u32)  # the maximum
  wire(w, "00ff", "putChar at both ends")
  # U+0161 truncates to 0x61, which is `a`: a frame of the right length holding
  # a different, plausible character.
  expectRefusal "putChar U+0161":
    var x = initWriter(eLE, eXcdr2)
    x.putChar(0x161'u32)
  expectRefusal "putChar U+0100":
    var x = initWriter(eLE, eXcdr2)
    x.putChar(0x100'u32)

  var ww = initWriter(eLE, eXcdr2)
  ww.putWchar(0'u32)
  ww.putWchar(0xFFFF'u32)
  wire(ww, "0000ffff", "putWchar at both ends")
  expectRefusal "putWchar U+10000":
    var x = initWriter(eLE, eXcdr2)
    x.putWchar(0x10000'u32)
  expectRefusal "putWchar U+1D11E":
    var x = initWriter(eLE, eXcdr2)
    x.putWchar(0x1D11E'u32)

  var empty = initWriter(eLE, eXcdr2)
  empty.putWstring(@[])
  wire(empty, "00000000", "putWstring of no units")

  var one = initWriter(eLE, eXcdr2)
  one.putWstring(@[0xFFFF'u16])
  wire(one, "02000000ffff", "putWstring at the unit maximum")

proc idEdges() =
  # 28 bits, so 0x0FFFFFFF is in and 0x10000000 is not.
  # 0xAFFFFFFF = must-understand | LC2 | 0x0FFFFFFF.
  var w = initWriter(eLE, eXcdr2)
  let m = w.emHeaderBegin(0x0FFF_FFFF'u32, true, lc2)
  w.putU32(0'u32)
  w.emHeaderEnd(m, lc2)
  wire(w, "ffffffaf00000000", "the widest EMHEADER member id")
  expectRefusal "an EMHEADER id of 2^28":
    var x = initWriter(eLE, eXcdr2)
    discard x.emHeaderBegin(0x1000_0000'u32, false, lc2)

  # 0x3F00 is the last short id; 0x3F01 is PID_EXTENDED.
  var p = initWriter(eLE, eXcdr1)
  let short = p.pidBegin(0x3F00'u16, false)
  p.pidEnd(short)
  inc checks
  expectRefusal "a short parameter id of 0x3F01":
    var x = initWriter(eLE, eXcdr1)
    discard x.pidBegin(0x3F01'u16, false)
  expectRefusal "an eMemberHeader id of 2^28":
    var x = initWriter(eLE, eXcdr1)
    discard x.pidExtendedBegin(0x1000_0000'u32, false)

proc mustUnderstandBits() =
  var w = initWriter(eLE, eXcdr2)
  let m = w.emHeaderBegin(1'u32, true, lc2)
  w.putU32(0'u32)
  w.emHeaderEnd(m, lc2)
  wire(w, "010000a000000000", "the EMHEADER must-understand bit") # 0xA0000001

  var p = initWriter(eLE, eXcdr1)
  let one = p.pidBegin(1'u16, true)
  p.putU32(0'u32)
  p.pidEnd(one)
  wire(p, "0140040000000000", "the PID must-understand bit") # 0x4001

proc lengthCodeMismatch() =
  expectRefusal "LC2 over a two-octet body":
    var w = initWriter(eLE, eXcdr2)
    let m = w.emHeaderBegin(0'u32, false, lc2)
    w.putU16(1'u16) # two octets where LC2 says four
    w.emHeaderEnd(m, lc2)
  expectRefusal "LC5 over a two-octet body":
    var w = initWriter(eLE, eXcdr2)
    let m = w.emHeaderBegin(0'u32, false, lc5)
    w.putU16(1'u16) # LC5 takes its length from four octets that are not there
    w.emHeaderEnd(m, lc5)

proc bitsetEdges() =
  var w = initWriter(eLE, eXcdr2)
  w.putBitset(0xFF'u64, 1)
  w.putBitset(0xFFFF'u64, 2)
  w.putBitset(0xFFFF_FFFF'u64, 4)
  w.putBitset(0xFFFF_FFFF_FFFF_FFFF'u64, 8)
  # ff | 00 (the u16 anchor's padding) | ffff | ffffffff | ffffffffffffffff
  wire(w, "ff00ffffffffffffffffffffffffffff", "the four bitset anchors")
  for bad in [0, 3, 16]:
    expectRefusal "a bitset anchor of " & $bad:
      var x = initWriter(eLE, eXcdr2)
      x.putBitset(0'u64, bad)

proc fixedEdges() =
  # The narrowest type: one digit, no fraction. (1+1) is even, so no leading
  # zero nibble, and the frame is one octet.
  doAssert fixedOctets("0", 1, 0) == hexBytes("0c")
  doAssert fixedOctets("9", 1, 0) == hexBytes("9c")
  doAssert fixedOctets("-9", 1, 0) == hexBytes("9d")
  # A negative zero is one zero, and its sign nibble is 0xC. A writer that took
  # the sign from the text would put 0xD there and two encoders of the same
  # value would disagree.
  doAssert fixedOctets("-0", 1, 0) == hexBytes("0c")
  doAssert fixedOctets("-0.000", 9, 3) == hexBytes("000000000c")
  # An odd digit count pairs the digits with the sign by prepending a zero
  # nibble: fixed<2,0> is (2+1) odd, so three nibbles become two octets.
  doAssert fixedOctets("12", 2, 0) == hexBytes("012c")
  # The widest an IDL `fixed` may be: 31 digits and a sign is 32 nibbles, so
  # sixteen octets and no leading pad.
  doAssert fixedOctets("1234567890123456789012345678901", 31, 0) ==
    hexBytes("1234567890123456789012345678901c")
  # scale == digits: every digit is fraction.
  doAssert fixedOctets("0.12", 2, 2) == hexBytes("012c")
  checks += 8

  # One past each end.
  expectRefusal "fixed<0,0>": discard fixedOctets("1", 0, 0)
  expectRefusal "fixed<32,0>": discard fixedOctets("1", 32, 0)
  expectRefusal "a scale past the digits": discard fixedOctets("1", 2, 3)
  expectRefusal "an integer part too wide": discard fixedOctets("100", 2, 0)
  expectRefusal "a fraction too wide": discard fixedOctets("0.123", 3, 2)
  expectRefusal "an exponent in a fixed": discard fixedOctets("1e3", 3, 0)
  expectRefusal "a sign and no digits": discard fixedOctets("-", 3, 0)

  for (text, digits, scale, back) in [("123456.789", 9, 3, "123456.789"),
                                      ("-000000.001", 9, 3, "-000000.001"),
                                      ("0", 1, 0, "0"),
                                      ("-9", 1, 0, "-9"),
                                      # scale == digits, so the point leads.
                                      ("0.12", 2, 2, ".12")]:
    var w = initWriter(eLE, eXcdr1)
    w.putFixed(text, digits, scale)
    var r = initReader(w.bytes(), eLE, eXcdr1)
    doAssert r.getFixed(digits, scale) == back, text
    inc checks

proc boolAndEmptyDheader() =
  var w = initWriter(eBE, eXcdr1)
  w.putBool(false)
  w.putBool(true)
  wire(w, "0001", "putBool")
  var r = initReader(w.bytes(), eBE, eXcdr1)
  doAssert not r.getBool()
  doAssert r.getBool()

  var d = initWriter(eLE, eXcdr2)
  let body = d.dheaderBegin()
  d.dheaderEnd(body)
  wire(d, "00000000", "a DHEADER over no octets")
  var dr = initReader(d.bytes(), eLE, eXcdr2)
  doAssert dr.dheaderRead() == 0'u32

proc surfaceTests*() =
  tailPad()
  extAppendable()
  extMutableXcdr2()
  extMutableXcdr1Short()
  extMutableXcdr1Extended()
  pid3f00()
  lc567()
  wide()
  bitsets()
  fixedBasic()
  integerEdges()
  u64AlignmentEdge()
  floatEdges()
  characterEdges()
  idEdges()
  mustUnderstandBits()
  lengthCodeMismatch()
  bitsetEdges()
  fixedEdges()
  boolAndEmptyDheader()
  echo "surface: ", checks, " checks OK"
