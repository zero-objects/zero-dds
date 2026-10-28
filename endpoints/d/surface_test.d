// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// The D endpoint SDK's XCDR surface, measured against octets four other vendors
// wrote.
//
// ## Why the expected octets are not ours
//
// A writer compared against a reference file this repository generated is two
// readings of one opinion. Every wire expectation in this file is a row of
// `crates/ast-layout/tests/fixtures/vendor_bytes.rows.tsv` — produced by
// `tools/byte-gate/gate.sh` from Eclipse Cyclone DDS, RTI Connext, eProsima Fast
// DDS and OpenDDS running their own code generators over
// `tools/byte-gate/cases/<case>/type.idl` and `fill.inc` — or a
// `tests/interop-results/<case>/<vendor>/` file written by omniORB or Fast DDS.
// The vendor and the case are named at every expectation, and where the vendors
// disagree the divergence is named too rather than averaged away.
//
// The closing block asks every new operation what it does at 0, at -1, at the
// minimum, at the maximum and one past it.

module surface_test;

import zerodds;
import std.stdio : writeln;
import std.conv : to;

private int checks = 0;

private ubyte[] hexBytes(string s) {
    ubyte[] outp;
    for (size_t i = 0; i + 1 < s.length; i += 2)
        outp ~= cast(ubyte)((nibble(s[i]) << 4) | nibble(s[i + 1]));
    return outp;
}

private ubyte nibble(char c) {
    if (c >= '0' && c <= '9') return cast(ubyte)(c - '0');
    if (c >= 'a' && c <= 'f') return cast(ubyte)(c - 'a' + 10);
    if (c >= 'A' && c <= 'F') return cast(ubyte)(c - 'A' + 10);
    assert(false, "not a hex digit");
}

private string toHexText(const(ubyte)[] b) {
    immutable string digits = "0123456789abcdef";
    string outp;
    foreach (x; b) {
        outp ~= digits[x >> 4];
        outp ~= digits[x & 0x0F];
    }
    return outp;
}

private void wire(Writer w, string want, string label) {
    string got = toHexText(w.bytes());
    assert(got == want, label ~ ": wrote " ~ got ~ ", the vendor wrote " ~ want);
    checks++;
}

/// A refusal is measured by running the write and requiring the exception, not
/// by inspecting the buffer afterwards: a binding that wrote the wrong octets
/// and then threw would pass an inspection of the exception alone.
private void expectRefusal(void delegate() run, string label) {
    bool refused = false;
    try
        run();
    catch (WireException)
        refused = true;
    assert(refused, label ~ ": the refusal did not happen");
    checks++;
}

// -- tail_pad: @final struct Odd { long a; short b; }; a = 1, b = 2 -----------
//
// Cyclone, RTI and Fast DDS all write six octets under both encodings and both
// byte orders. The case exists because a writer that rounded the stream up to
// its own alignment would write eight, and every one of the three says it does
// not.

private void tailPad() {
    struct Row { Endian e; string want; }
    foreach (row; [Row(Endian.LE, "010000000200"), Row(Endian.BE, "000000010002")]) {
        foreach (enc; [Encoding.XCDR1, Encoding.XCDR2]) {
            auto w = Writer(row.e, enc);
            w.putU32(1);
            w.putU16(2);
            wire(w, row.want, "tail_pad " ~ to!string(row.e) ~ " " ~ to!string(enc));
            auto r = Reader(w.bytes(), row.e, enc);
            assert(r.getU32() == 1);
            assert(r.getU16() == 2);
        }
    }
}

// -- ext_appendable: @appendable struct Ext { long a; double b; short c; } ----
//
// a = 7, b = 1.5, c = -3. The two encodings differ in both of the things this
// commit added: under XCDR1 there is no DHEADER and the `double` aligns to eight
// (offset 4..8 is padding); under XCDR2 a DHEADER of 14 precedes the body and
// the `double` aligns to four. Cyclone, RTI and Fast DDS agree on all four rows.

private void extAppendable() {
    struct Row { Endian e; string want; }
    foreach (row; [Row(Endian.LE, "0700000000000000000000000000f83ffdff"),
                   Row(Endian.BE, "00000007000000003ff8000000000000fffd")]) {
        auto w = Writer(row.e, Encoding.XCDR1);
        w.putU32(7);
        w.putF64(1.5);
        w.putU16(0xFFFD);
        wire(w, row.want, "ext_appendable xcdr1 " ~ to!string(row.e));
        auto r = Reader(w.bytes(), row.e, Encoding.XCDR1);
        assert(r.getU32() == 7);
        assert(r.getF64() == 1.5);
        assert(r.getU16() == 0xFFFD);
    }
    foreach (row; [Row(Endian.LE, "0e00000007000000000000000000f83ffdff"),
                   Row(Endian.BE, "0000000e000000073ff8000000000000fffd")]) {
        auto w = Writer(row.e, Encoding.XCDR2);
        int body_ = w.dheaderBegin();
        w.putU32(7);
        w.putF64(1.5);
        w.putU16(0xFFFD);
        w.dheaderEnd(body_);
        wire(w, row.want, "ext_appendable xcdr2 " ~ to!string(row.e));
        auto r = Reader(w.bytes(), row.e, Encoding.XCDR2);
        assert(r.dheaderRead() == 14);
        assert(r.getU32() == 7);
        assert(r.getF64() == 1.5);
        assert(r.getU16() == 0xFFFD);
    }
}

// -- ext_mutable under PL_CDR2 ------------------------------------------------
//
// Cyclone, RTI and Fast DDS write the identical thirty octets. The three length
// codes are the three member widths: LC2 for the `long`, LC3 for the `double`,
// LC1 for the `short`, and none of the three reserves a NEXTINT.

private void extMutableXcdr2() {
    struct Row { Endian e; string want; }
    foreach (row; [
        Row(Endian.LE, "1a000000000000200700000001000030000000000000f83f02000010fdff"),
        Row(Endian.BE, "0000001a2000000000000007300000013ff800000000000010000002fffd")]) {
        auto w = Writer(row.e, Encoding.XCDR2);
        int body_ = w.dheaderBegin();
        int m = w.emHeaderBegin(0, false, LengthCode.LC2);
        w.putU32(7);
        w.emHeaderEnd(m, LengthCode.LC2);
        m = w.emHeaderBegin(1, false, LengthCode.LC3);
        w.putF64(1.5);
        w.emHeaderEnd(m, LengthCode.LC3);
        m = w.emHeaderBegin(2, false, LengthCode.LC1);
        w.putU16(0xFFFD);
        w.emHeaderEnd(m, LengthCode.LC1);
        w.dheaderEnd(body_);
        wire(w, row.want, "ext_mutable xcdr2 " ~ to!string(row.e));

        // And back: the extent each code announces is the member's own width,
        // with nothing consumed beyond the header.
        auto r = Reader(w.bytes(), row.e, Encoding.XCDR2);
        assert(r.dheaderRead() == 26);
        auto member = r.emHeaderRead();
        assert(member.id == 0 && member.lc == LengthCode.LC2 && member.extent == 4);
        assert(r.getU32() == 7);
        member = r.emHeaderRead();
        assert(member.id == 1 && member.extent == 8);
        assert(r.getF64() == 1.5);
        member = r.emHeaderRead();
        assert(member.id == 2 && member.extent == 2);
        assert(r.getU16() == 0xFFFD);
    }
}

// The same sample under PL_CDR1, in the short parameter header RTI writes. Fast
// DDS differs in two places: it closes the list with the bare 0x3F02 where RTI
// and Cyclone set the must-understand bit and write 0x7F02, and it declares two
// octets for the `short` where RTI declares four. This SDK writes the majority
// form on both.
//
// The `double` is the reason the alignment origin exists: its header sits at
// offset 8..12 and the value follows at 12 with no padding, because section
// 7.4.1.2 aligns a parameter's value against its own block. Measured from the
// stream the value would land at 16 and the header would declare twelve; all
// three vendors declare eight.

private void extMutableXcdr1Short() {
    struct Row { Endian e; string want; }
    foreach (row; [
        Row(Endian.LE, "000004000700000001000800000000000000f83f02000400fdff0000027f0000"),
        Row(Endian.BE, "0000000400000007000100083ff800000000000000020004fffd00007f020000")]) {
        auto w = Writer(row.e, Encoding.XCDR1);
        auto p = w.pidBegin(0, false);
        w.putU32(7);
        w.pidEnd(p);
        p = w.pidBegin(1, false);
        w.putF64(1.5);
        w.pidEnd(p);
        p = w.pidBegin(2, false);
        w.putU16(0xFFFD);
        w.pidEnd(p);
        w.pidListEnd();
        wire(w, row.want, "ext_mutable xcdr1 short pid " ~ to!string(row.e));

        auto r = Reader(w.bytes(), row.e, Encoding.XCDR1);
        auto param = r.pidRead();
        assert(param.id == 0 && param.length == 4);
        assert(r.getU32() == 7);
        r.pidSkip(4, param.length);
        param = r.pidRead();
        assert(param.id == 1 && param.length == 8);
        assert(r.getF64() == 1.5);
        r.pidSkip(12, param.length);
        param = r.pidRead();
        assert(param.id == 2 && param.length == 4);
        assert(r.getU16() == 0xFFFD);
        r.pidSkip(24, param.length);
        assert(r.pidRead().listEnd);
    }
}

// The same sample again, in the twelve-octet extended parameter header Cyclone
// writes for every mutable member under PL_CDR1. Fifty-six octets, and the
// must-understand bit is on the PID_EXTENDED marker (0x7F01) while the member's
// own eMemberHeader carries none — which is what makes reading the flag off the
// marker wrong.

private void extMutableXcdr1Extended() {
    struct Row { Endian e; string want; }
    foreach (row; [
        Row(Endian.LE, "017f0800000000000400000007000000017f08000100000008000000"
                       ~ "000000000000f83f017f08000200000004000000fdff0000027f0000"),
        Row(Endian.BE, "7f0100080000000000000004000000077f01000800000001000000"
                       ~ "083ff80000000000007f0100080000000200000004fffd00007f020000")]) {
        auto w = Writer(row.e, Encoding.XCDR1);
        auto p = w.pidExtendedBegin(0, false);
        w.putU32(7);
        w.pidEnd(p);
        p = w.pidExtendedBegin(1, false);
        w.putF64(1.5);
        w.pidEnd(p);
        p = w.pidExtendedBegin(2, false);
        w.putU16(0xFFFD);
        w.pidEnd(p);
        w.pidListEnd();
        wire(w, row.want, "ext_mutable xcdr1 extended pid " ~ to!string(row.e));

        auto r = Reader(w.bytes(), row.e, Encoding.XCDR1);
        auto first = r.pidRead();
        assert(first.id == 0);
        assert(!first.mustUnderstand, "the marker's flag is not the member's");
        assert(first.length == 4);
        assert(r.getU32() == 7);
    }
}

// -- pid_3f00 -----------------------------------------------------------------
//
// @mutable struct PidEdge { @id(16127) long a; @id(16128) long b; }. 16128 is
// 0x3F00, the highest id a short parameter header can carry: 0x3F01 is
// PID_EXTENDED and 0x3F02 is PID_LIST_END. RTI writes both ids in the short
// form, which is the boundary this case exists to hold.

private void pid3f00() {
    struct Row { Endian e; string want; }
    foreach (row; [Row(Endian.LE, "ff3e040007000000003f040009000000027f0000"),
                   Row(Endian.BE, "3eff0004000000073f000004000000097f020000")]) {
        auto w = Writer(row.e, Encoding.XCDR1);
        auto p = w.pidBegin(0x3EFF, false);
        w.putU32(7);
        w.pidEnd(p);
        p = w.pidBegin(0x3F00, false);
        w.putU32(9);
        w.pidEnd(p);
        w.pidListEnd();
        wire(w, row.want, "pid_3f00 xcdr1 " ~ to!string(row.e));
    }
    foreach (row; [Row(Endian.LE, "10000000ff3e002007000000003f002009000000"),
                   Row(Endian.BE, "0000001020003eff0000000720003f0000000009")]) {
        auto w = Writer(row.e, Encoding.XCDR2);
        int body_ = w.dheaderBegin();
        int m = w.emHeaderBegin(0x3EFF, false, LengthCode.LC2);
        w.putU32(7);
        w.emHeaderEnd(m, LengthCode.LC2);
        m = w.emHeaderBegin(0x3F00, false, LengthCode.LC2);
        w.putU32(9);
        w.emHeaderEnd(m, LengthCode.LC2);
        w.dheaderEnd(body_);
        wire(w, row.want, "pid_3f00 xcdr2 " ~ to!string(row.e));
    }
}

// -- lc_5_6_7 -----------------------------------------------------------------
//
// @mutable struct M { @id(1) string s; @id(2) sequence<octet> b;
//                     @id(3) sequence<long> l; @id(4) sequence<double> d;
//                     @id(5) sequence<short> h; };
//
// Cyclone's seventy-two octets. LC5, LC6 and LC7 take the member's own leading
// word as the length — LC6 counting four-octet elements and LC7 eight — and
// reserve no NEXTINT; only `sequence<short>` gets LC4 and a NEXTINT of 8. A
// writer that emitted a NEXTINT for LC5..LC7 as well would put every following
// member four octets late, which is the defect this case holds.
//
// RTI did not compile the case (`fill.inc:6:54: 'struct DDS_OctetSeq' has no
// member named '_buffer'`) and Fast DDS writes LC4 where Cyclone writes LC6 for
// `sequence<long>`; Cyclone's rows are the ones reproduced.

private void lc567() {
    struct Row { Endian e; string want; }
    foreach (row; [
        Row(Endian.LE, "440000000100005004000000616263000200005003000000010203"
                       ~ "000300006001000000070000000400007001000000000000000000f83f"
                       ~ "05000040080000000200000004000500"),
        Row(Endian.BE, "00000044500000010000000461626300500000020000000301020300"
                       ~ "60000003000000010000000770000004000000013ff8000000000000"
                       ~ "40000005000000080000000200040005")]) {
        auto w = Writer(row.e, Encoding.XCDR2);
        int body_ = w.dheaderBegin();
        int m = w.emHeaderBegin(1, false, LengthCode.LC5);
        w.putString("abc");
        w.emHeaderEnd(m, LengthCode.LC5);
        m = w.emHeaderBegin(2, false, LengthCode.LC5);
        w.putSeqU8([cast(ubyte) 1, 2, 3]);
        w.emHeaderEnd(m, LengthCode.LC5);
        m = w.emHeaderBegin(3, false, LengthCode.LC6);
        w.putU32(1);
        w.putU32(7);
        w.emHeaderEnd(m, LengthCode.LC6);
        m = w.emHeaderBegin(4, false, LengthCode.LC7);
        w.putU32(1);
        w.putF64(1.5);
        w.emHeaderEnd(m, LengthCode.LC7);
        m = w.emHeaderBegin(5, false, LengthCode.LC4);
        w.putU32(2);
        w.putU16(4);
        w.putU16(5);
        w.emHeaderEnd(m, LengthCode.LC4);
        w.dheaderEnd(body_);
        wire(w, row.want, "lc_5_6_7 xcdr2 " ~ to!string(row.e));

        // Reading Cyclone's own octets: each code's extent counts what the spec
        // says it counts, and LC5..LC7 leave the leading word in place.
        auto r = Reader(w.bytes(), row.e, Encoding.XCDR2);
        assert(r.dheaderRead() == 68);
        auto member = r.emHeaderRead();
        assert(member.lc == LengthCode.LC5 && member.extent == 8); // 4 + 4
        assert(r.getString() == "abc");
        member = r.emHeaderRead();
        assert(member.extent == 7); // 4 + 3
        assert(r.getSeqU8() == [cast(ubyte) 1, 2, 3]);
        member = r.emHeaderRead();
        assert(member.lc == LengthCode.LC6 && member.extent == 8); // 4 + 4*1
        assert(r.getU32() == 1);
        assert(r.getU32() == 7);
        member = r.emHeaderRead();
        assert(member.lc == LengthCode.LC7 && member.extent == 12); // 4 + 8*1
        assert(r.getU32() == 1);
        assert(r.getF64() == 1.5);
        member = r.emHeaderRead();
        assert(member.lc == LengthCode.LC4 && member.extent == 8);
    }
}

// -- the wide character and the wide string -----------------------------------
//
// wchar_member: @final struct WCh { wchar c; octet tail; }, c = U+0061,
// tail = 0xAB. Cyclone and Fast DDS write two octets under both encodings and
// RTI does under XCDR2. RTI's XCDR1 `wchar_t` is four octets wide (`61000000ab`)
// — the one vendor answer this SDK does not reproduce, and it is RTI alone.
//
// wstring_member / wstring_nonbmp: @final struct WStr { wstring w; octet tail; }.
// The little-endian rows are Cyclone's under both encodings and RTI's under
// XCDR2: the length word counts four octets for two code units, there is no
// terminator and no byte-order mark. Fast DDS writes the unit *count* there
// (`02000000`) and truncates U+1D11E to one unit (`010000001ed1`); neither is
// reproduced. The big-endian row is RTI's under XCDR2 alone — Cyclone byte-swaps
// the length word but leaves the code units little-endian
// (`0000000461006200ab`), which no reading of section 7.4.1 supports.

private void wide() {
    struct Row { Endian e; string want; }
    foreach (row; [Row(Endian.LE, "6100ab"), Row(Endian.BE, "0061ab")]) {
        foreach (enc; [Encoding.XCDR1, Encoding.XCDR2]) {
            auto w = Writer(row.e, enc);
            w.putWchar(0x61);
            w.putU8(0xAB);
            wire(w, row.want, "wchar_member " ~ to!string(row.e) ~ " " ~ to!string(enc));
            auto r = Reader(w.bytes(), row.e, enc);
            assert(r.getWchar() == 0x61);
            assert(r.getU8() == 0xAB);
        }
    }

    auto le = Writer(Endian.LE, Encoding.XCDR2);
    le.putWstring([cast(ushort) 0x61, 0x62]);
    le.putU8(0xAB);
    wire(le, "0400000061006200ab", "wstring_member le"); // cyclone, rti xcdr2

    auto be = Writer(Endian.BE, Encoding.XCDR2);
    be.putWstring([cast(ushort) 0x61, 0x62]);
    be.putU8(0xAB);
    wire(be, "0000000400610062ab", "wstring_member be"); // rti xcdr2

    auto r = Reader(be.bytes(), Endian.BE, Encoding.XCDR2);
    assert(r.getWstring() == [cast(ushort) 0x61, 0x62]);
    assert(r.getU8() == 0xAB);

    auto nonbmp = Writer(Endian.LE, Encoding.XCDR2);
    nonbmp.putWstring([cast(ushort) 0xD834, 0xDD1E]);
    nonbmp.putU8(0xAB);
    wire(nonbmp, "0400000034d81eddab", "wstring_nonbmp le"); // cyclone, rti xcdr2

    auto nonbmpBe = Writer(Endian.BE, Encoding.XCDR2);
    nonbmpBe.putWstring([cast(ushort) 0xD834, 0xDD1E]);
    nonbmpBe.putU8(0xAB);
    wire(nonbmpBe, "00000004d834dd1eab", "wstring_nonbmp be"); // rti xcdr2

    auto rb = Reader(nonbmp.bytes(), Endian.LE, Encoding.XCDR2);
    assert(rb.getWstring() == [cast(ushort) 0xD834, 0xDD1E]);
}

// -- bitset -------------------------------------------------------------------
//
// `bitset Packed { bitfield<3> low; bitfield<5> high; }` with low = 5 and
// high = 17 packs into one octet: 5 | (17 << 3) = 0x8D. Fast DDS wrote `8dab` for
// all four configurations of `tests/interop-corpus/bitset-basic` (Cyclone, RTI
// and OpenDDS have no `bitset` in their IDL front ends and left the cell empty).
//
// `bitset Wide { bitfield<20> first; bitfield<20> second; bitfield<24> third; }`
// with first = 1048575, second = 0, third = 12345678 packs into eight octets:
// 0xBC614E00000FFFFF. The anchor's width is the alignment, which is what makes it
// a separate call from `putU64`.

private void bitsets() {
    auto one = Writer(Endian.LE, Encoding.XCDR1);
    one.putBitset(5 | (17 << 3), 1);
    one.putU8(0xAB);
    wire(one, "8dab", "bitset-basic");
    auto r1 = Reader(one.bytes(), Endian.LE, Encoding.XCDR1);
    assert(r1.getBitset(1) == 0x8D);

    struct Row { Endian e; string want; }
    foreach (row; [Row(Endian.LE, "ffff0f00004e61bcab"),
                   Row(Endian.BE, "bc614e00000fffffab")]) {
        foreach (enc; [Encoding.XCDR1, Encoding.XCDR2]) {
            auto w = Writer(row.e, enc);
            w.putBitset(0xBC614E00000FFFFFUL, 8);
            w.putU8(0xAB);
            wire(w, row.want, "bitset-multiword " ~ to!string(row.e) ~ " " ~ to!string(enc));
            auto r = Reader(w.bytes(), row.e, enc);
            assert(r.getBitset(8) == 0xBC614E00000FFFFFUL);
        }
    }
}

// -- fixed<9,3> ---------------------------------------------------------------
//
// `tests/interop-corpus/fixed-basic`: amount = "123456.789",
// debt = "-000000.001", tail = 0xAB. omniORB wrote `123456789c 000000001d ab` in
// both byte orders — BCD has no byte order, which is the property the two
// identical files state. JacORB writes `123456789c 1dab`: it drops the leading
// zero octets of the negative value, so its frame is four octets shorter and
// cannot be parsed against the declared digit count. omniORB's is the form
// reproduced.

private void fixedBasic() {
    foreach (endian; [Endian.LE, Endian.BE]) {
        auto w = Writer(endian, Encoding.XCDR1);
        w.putFixed("123456.789", 9, 3);
        w.putFixed("-000000.001", 9, 3);
        w.putU8(0xAB);
        wire(w, "123456789c000000001dab", "fixed-basic " ~ to!string(endian));
        auto r = Reader(w.bytes(), endian, Encoding.XCDR1);
        assert(r.getFixed(9, 3) == "123456.789");
        assert(r.getFixed(9, 3) == "-000000.001");
        assert(r.getU8() == 0xAB);
    }
}

// ============================ the range edges ============================
//
// The last sweep found four defects and every one of them was at an end of a
// range, not in the middle: an Ada 83 divide that truncated toward zero for
// every negative argument, a Zig frame writer that ran past its output buffer,
// and nine of fifteen bindings answering `put_u16(70000)` with `7011`. So each
// operation is asked here what it does at 0, at -1, at the minimum, at the
// maximum, and one past the maximum.

private void integerEdges() {
    // `putU16` used to take an `int` and mask, so 70000 wrote `7011`. It takes a
    // `ushort` now: D refuses the narrowing at compile time, so the truncation
    // has no spelling left. What is measurable here is that the extremes go out
    // whole.
    auto w = Writer(Endian.LE, Encoding.XCDR2);
    w.putU8(0);
    w.putU8(255);
    w.putU16(0);
    w.putU16(65535);
    w.putU32(0);
    w.putU32(4294967295);
    w.putU64(0);
    w.putU64(18446744073709551615UL);
    // 00 ff | 0000 ffff | 0000 (the u32's padding) | 00000000 ffffffff
    // | 0000000000000000 ffffffffffffffff
    wire(w, "00ff0000ffff000000000000ffffffff"
            ~ "0000000000000000ffffffffffffffff", "the integer extremes");

    auto r = Reader(w.bytes(), Endian.LE, Encoding.XCDR2);
    assert(r.getU8() == 0);
    assert(r.getU8() == 255);
    assert(r.getU16() == 0);
    assert(r.getU16() == 65535);
    assert(r.getU32() == 0);
    assert(r.getU32() == 4294967295);
    assert(r.getU64() == 0);
    assert(r.getU64() == 18446744073709551615UL);

    // The signed extremes, as the two's complement a caller converts them to.
    auto s = Writer(Endian.LE, Encoding.XCDR2);
    s.putU8(cast(ubyte) cast(byte) -1);
    s.putU8(cast(ubyte) byte.min);
    s.putU16(cast(ushort) cast(short) -1);
    s.putU16(cast(ushort) short.min);
    s.putU32(cast(uint) -1);
    s.putU32(cast(uint) int.min);
    s.putU64(cast(ulong) -1L);
    s.putU64(cast(ulong) long.min);
    wire(s, "ff80ffff00800000ffffffff00000080"
            ~ "ffffffffffffffff0000000000000080", "the signed extremes");
}

private void u64AlignmentEdge() {
    // The Go column's third commit: `getU64` passed four where `putU64` passed
    // eight, so 0xFFFFFFFFFFFFFFFF read back as 0xFFFFFFFF00000000 with no error
    // anywhere. One octet first, so the value is not already aligned.
    auto w = Writer(Endian.LE, Encoding.XCDR1);
    w.putU8(0xAB);
    w.putU64(0xFFFF_FFFF_FFFF_FFFFUL);
    wire(w, "ab00000000000000ffffffffffffffff", "putU64 under XCDR1");
    auto r = Reader(w.bytes(), Endian.LE, Encoding.XCDR1);
    assert(r.getU8() == 0xAB);
    assert(r.getU64() == 0xFFFF_FFFF_FFFF_FFFFUL);
}

private void floatEdges() {
    foreach (v; [0.0, -0.0, 1.5, -1.5, double.min_normal, double.max]) {
        auto w = Writer(Endian.BE, Encoding.XCDR1);
        w.putF64(v);
        auto r = Reader(w.bytes(), Endian.BE, Encoding.XCDR1);
        double back = r.getF64();
        assert(*cast(ulong*)&back == *cast(ulong*)&v);
        checks++;
    }
}

private void characterEdges() {
    auto w = Writer(Endian.LE, Encoding.XCDR2);
    w.putChar(0);      // the minimum
    w.putChar(0xFF);   // the maximum
    wire(w, "00ff", "putChar at both ends");
    // U+0161 truncates to 0x61, which is `a`: a frame of the right length
    // holding a different, plausible character.
    expectRefusal({ auto x = Writer(Endian.LE, Encoding.XCDR2); x.putChar(0x161); },
                  "putChar U+0161");
    expectRefusal({ auto x = Writer(Endian.LE, Encoding.XCDR2); x.putChar(0x100); },
                  "putChar U+0100");

    auto ww = Writer(Endian.LE, Encoding.XCDR2);
    ww.putWchar(0);
    ww.putWchar(0xFFFF);
    wire(ww, "0000ffff", "putWchar at both ends");
    expectRefusal({ auto x = Writer(Endian.LE, Encoding.XCDR2); x.putWchar(0x10000); },
                  "putWchar U+10000");
    expectRefusal({ auto x = Writer(Endian.LE, Encoding.XCDR2); x.putWchar(0x1D11E); },
                  "putWchar U+1D11E");

    auto empty = Writer(Endian.LE, Encoding.XCDR2);
    empty.putWstring([]);
    wire(empty, "00000000", "putWstring of no units");

    auto one = Writer(Endian.LE, Encoding.XCDR2);
    one.putWstring([cast(ushort) 0xFFFF]);
    wire(one, "02000000ffff", "putWstring at the unit maximum");
}

private void idEdges() {
    // 28 bits, so 0x0FFFFFFF is in and 0x10000000 is not.
    // 0xAFFFFFFF = must-understand | LC2 | 0x0FFFFFFF.
    auto w = Writer(Endian.LE, Encoding.XCDR2);
    int m = w.emHeaderBegin(0x0FFF_FFFF, true, LengthCode.LC2);
    w.putU32(0);
    w.emHeaderEnd(m, LengthCode.LC2);
    wire(w, "ffffffaf00000000", "the widest EMHEADER member id");
    expectRefusal({
        auto x = Writer(Endian.LE, Encoding.XCDR2);
        x.emHeaderBegin(0x1000_0000, false, LengthCode.LC2);
    }, "an EMHEADER id of 2^28");

    // 0x3F00 is the last short id; 0x3F01 is PID_EXTENDED.
    auto p = Writer(Endian.LE, Encoding.XCDR1);
    auto short_ = p.pidBegin(0x3F00, false);
    p.pidEnd(short_);
    checks++;
    expectRefusal({
        auto x = Writer(Endian.LE, Encoding.XCDR1);
        x.pidBegin(0x3F01, false);
    }, "a short parameter id of 0x3F01");
    expectRefusal({
        auto x = Writer(Endian.LE, Encoding.XCDR1);
        x.pidExtendedBegin(0x1000_0000, false);
    }, "an eMemberHeader id of 2^28");
}

private void mustUnderstandBits() {
    auto w = Writer(Endian.LE, Encoding.XCDR2);
    int m = w.emHeaderBegin(1, true, LengthCode.LC2);
    w.putU32(0);
    w.emHeaderEnd(m, LengthCode.LC2);
    wire(w, "010000a000000000", "the EMHEADER must-understand bit"); // 0xA0000001

    auto p = Writer(Endian.LE, Encoding.XCDR1);
    auto one = p.pidBegin(1, true);
    p.putU32(0);
    p.pidEnd(one);
    wire(p, "0140040000000000", "the PID must-understand bit"); // 0x4001
}

private void lengthCodeMismatch() {
    expectRefusal({
        auto w = Writer(Endian.LE, Encoding.XCDR2);
        int m = w.emHeaderBegin(0, false, LengthCode.LC2);
        w.putU16(1); // two octets where LC2 says four
        w.emHeaderEnd(m, LengthCode.LC2);
    }, "LC2 over a two-octet body");
    expectRefusal({
        auto w = Writer(Endian.LE, Encoding.XCDR2);
        int m = w.emHeaderBegin(0, false, LengthCode.LC5);
        w.putU16(1); // LC5 takes its length from four octets that are not there
        w.emHeaderEnd(m, LengthCode.LC5);
    }, "LC5 over a two-octet body");
}

private void bitsetEdges() {
    auto w = Writer(Endian.LE, Encoding.XCDR2);
    w.putBitset(0xFFUL, 1);
    w.putBitset(0xFFFFUL, 2);
    w.putBitset(0xFFFF_FFFFUL, 4);
    w.putBitset(0xFFFF_FFFF_FFFF_FFFFUL, 8);
    // ff | 00 (the u16 anchor's padding) | ffff | ffffffff | ffffffffffffffff
    wire(w, "ff00ffffffffffffffffffffffffffff", "the four bitset anchors");
    foreach (bad; [0, 3, 16])
        expectRefusal({
            auto x = Writer(Endian.LE, Encoding.XCDR2);
            x.putBitset(0, bad);
        }, "a bitset anchor of " ~ to!string(bad));
}

private void fixedEdges() {
    // The narrowest type: one digit, no fraction. (1+1) is even, so no leading
    // zero nibble, and the frame is one octet.
    assert(fixedOctets("0", 1, 0) == hexBytes("0c"));
    assert(fixedOctets("9", 1, 0) == hexBytes("9c"));
    assert(fixedOctets("-9", 1, 0) == hexBytes("9d"));
    // A negative zero is one zero, and its sign nibble is 0xC. A writer that
    // took the sign from the text would put 0xD there and two encoders of the
    // same value would disagree.
    assert(fixedOctets("-0", 1, 0) == hexBytes("0c"));
    assert(fixedOctets("-0.000", 9, 3) == hexBytes("000000000c"));
    // An odd digit count pairs the digits with the sign by prepending a zero
    // nibble: fixed<2,0> is (2+1) odd, so three nibbles become two octets.
    assert(fixedOctets("12", 2, 0) == hexBytes("012c"));
    // The widest an IDL `fixed` may be: 31 digits and a sign is 32 nibbles, so
    // sixteen octets and no leading pad.
    assert(fixedOctets("1234567890123456789012345678901", 31, 0)
           == hexBytes("1234567890123456789012345678901c"));
    // scale == digits: every digit is fraction.
    assert(fixedOctets("0.12", 2, 2) == hexBytes("012c"));
    checks += 8;

    // One past each end.
    expectRefusal({ fixedOctets("1", 0, 0); }, "fixed<0,0>");
    expectRefusal({ fixedOctets("1", 32, 0); }, "fixed<32,0>");
    expectRefusal({ fixedOctets("1", 2, 3); }, "a scale past the digits");
    expectRefusal({ fixedOctets("100", 2, 0); }, "an integer part too wide");
    expectRefusal({ fixedOctets("0.123", 3, 2); }, "a fraction too wide");
    expectRefusal({ fixedOctets("1e3", 3, 0); }, "an exponent in a fixed");
    expectRefusal({ fixedOctets("-", 3, 0); }, "a sign and no digits");

    struct Case { string text; int digits; int scale; string back; }
    foreach (c; [Case("123456.789", 9, 3, "123456.789"),
                 Case("-000000.001", 9, 3, "-000000.001"),
                 Case("0", 1, 0, "0"),
                 Case("-9", 1, 0, "-9"),
                 // scale == digits, so the point leads.
                 Case("0.12", 2, 2, ".12")]) {
        auto w = Writer(Endian.LE, Encoding.XCDR1);
        w.putFixed(c.text, c.digits, c.scale);
        auto r = Reader(w.bytes(), Endian.LE, Encoding.XCDR1);
        assert(r.getFixed(c.digits, c.scale) == c.back, c.text);
        checks++;
    }
}

private void boolAndEmptyDheader() {
    auto w = Writer(Endian.BE, Encoding.XCDR1);
    w.putBool(false);
    w.putBool(true);
    wire(w, "0001", "putBool");
    auto r = Reader(w.bytes(), Endian.BE, Encoding.XCDR1);
    assert(!r.getBool());
    assert(r.getBool());

    auto d = Writer(Endian.LE, Encoding.XCDR2);
    int body_ = d.dheaderBegin();
    d.dheaderEnd(body_);
    wire(d, "00000000", "a DHEADER over no octets");
    auto dr = Reader(d.bytes(), Endian.LE, Encoding.XCDR2);
    assert(dr.dheaderRead() == 0);
}

void surfaceTests() {
    tailPad();
    extAppendable();
    extMutableXcdr2();
    extMutableXcdr1Short();
    extMutableXcdr1Extended();
    pid3f00();
    lc567();
    wide();
    bitsets();
    fixedBasic();
    integerEdges();
    u64AlignmentEdge();
    floatEdges();
    characterEdges();
    idEdges();
    mustUnderstandBits();
    lengthCodeMismatch();
    bitsetEdges();
    fixedEdges();
    boolAndEmptyDheader();
    writeln("surface: ", checks, " checks OK");
}
