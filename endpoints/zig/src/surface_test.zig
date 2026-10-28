// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// The Zig endpoint SDK's XCDR surface, measured against octets four other
// vendors wrote.
//
// ## Why the expected octets are not ours
//
// A writer compared against a reference file this repository generated is two
// readings of one opinion. Every wire expectation in this file is a row of
// `crates/ast-layout/tests/fixtures/vendor_bytes.rows.tsv` — produced by
// `tools/byte-gate/gate.sh` from Eclipse Cyclone DDS, RTI Connext, eProsima
// Fast DDS and OpenDDS running their own code generators over
// `tools/byte-gate/cases/<case>/type.idl` and `fill.inc` — or a
// `tests/interop-results/<case>/<vendor>/` file written by omniORB or Fast DDS.
// The vendor and the case are named at every expectation, and where the vendors
// disagree the divergence is named too rather than averaged away.
//
// ## What each case pins
//
//   tail_pad          no padding after the last member
//   ext_appendable    DHEADER (§7.4.3.5) and MAXALIGN 8 vs 4 (§7.4.1)
//   ext_mutable       EMHEADER LC1/LC2/LC3, the short and the extended
//                     PL_CDR1 parameter header, the sentinel
//   pid_3f00          the short parameter header at its highest id
//   lc_5_6_7          EMHEADER LC4, LC5, LC6 and LC7 in one sample
//   wchar_member      the two-octet code unit and the stream's byte order
//   wstring_member    the length word in octets, no terminator, no BOM
//   wstring_nonbmp    a surrogate pair as two code units
//   bitset-basic      the one-octet anchor and the bit packing
//   bitset-multiword  the eight-octet anchor
//   fixed-basic       packed BCD, both signs
//
// and the last block asks every new operation what it does at 0, at −1, at the
// minimum, at the maximum and one past it.

const std = @import("std");
const testing = std.testing;
const zd = @import("zerodds.zig");

/// A hex string as octets. `comptime` on the text so the array length is known;
/// the decode itself runs wherever it is called.
fn hex(comptime s: []const u8) [s.len / 2]u8 {
    var out: [s.len / 2]u8 = undefined;
    for (0..out.len) |i| {
        out[i] = (nibble(s[2 * i]) << 4) | nibble(s[2 * i + 1]);
    }
    return out;
}

fn nibble(c: u8) u8 {
    return switch (c) {
        '0'...'9' => c - '0',
        'a'...'f' => c - 'a' + 10,
        'A'...'F' => c - 'A' + 10,
        else => unreachable,
    };
}

fn expectWire(w: *zd.Writer, comptime want: []const u8) !void {
    try testing.expectEqualSlices(u8, &hex(want), w.bytes());
}

// -- tail_pad: @final struct Odd { long a; short b; }; a = 1, b = 2 ----------
//
// Cyclone, RTI and Fast DDS all write six octets under both encodings and both
// byte orders. The case exists because a writer that rounded the stream up to
// its own alignment would write eight, and every one of the three says it does
// not.

test "tail_pad: no padding after the last member (cyclone, rti, fastdds)" {
    const orders = [_]struct { e: zd.Endian, want: []const u8 }{
        .{ .e = .little, .want = "010000000200" },
        .{ .e = .big, .want = "000000010002" },
    };
    inline for (orders) |order| {
        inline for ([_]zd.Encoding{ .xcdr1, .xcdr2 }) |encoding| {
            var w = zd.Writer.init(testing.allocator, order.e, encoding);
            defer w.deinit();
            try w.putU32(1);
            try w.putU16(2);
            try expectWire(&w, order.want);

            var r = zd.Reader.init(w.bytes(), order.e, encoding);
            try testing.expectEqual(@as(u32, 1), r.getU32());
            try testing.expectEqual(@as(u16, 2), r.getU16());
        }
    }
}

// -- ext_appendable: @appendable struct Ext { long a; double b; short c; } ---
//
// a = 7, b = 1.5, c = −3. The two encodings differ in both of the things this
// commit added: under XCDR1 there is no DHEADER and the `double` aligns to eight
// (offset 4..8 is padding); under XCDR2 a DHEADER of 14 precedes the body and
// the `double` aligns to four. Cyclone, RTI and Fast DDS agree on all four rows.

test "ext_appendable xcdr1: MAXALIGN 8 and no delimiter (cyclone, rti, fastdds)" {
    const rows = [_]struct { e: zd.Endian, want: []const u8 }{
        .{ .e = .little, .want = "0700000000000000000000000000f83ffdff" },
        .{ .e = .big, .want = "00000007000000003ff8000000000000fffd" },
    };
    inline for (rows) |row| {
        var w = zd.Writer.init(testing.allocator, row.e, .xcdr1);
        defer w.deinit();
        try w.putU32(7);
        try w.putF64(1.5);
        try w.putU16(0xFFFD);
        try expectWire(&w, row.want);

        var r = zd.Reader.init(w.bytes(), row.e, .xcdr1);
        try testing.expectEqual(@as(u32, 7), r.getU32());
        try testing.expectEqual(@as(f64, 1.5), r.getF64());
        try testing.expectEqual(@as(u16, 0xFFFD), r.getU16());
    }
}

test "ext_appendable xcdr2: DHEADER 14 and MAXALIGN 4 (cyclone, rti, fastdds)" {
    const rows = [_]struct { e: zd.Endian, want: []const u8 }{
        .{ .e = .little, .want = "0e00000007000000000000000000f83ffdff" },
        .{ .e = .big, .want = "0000000e000000073ff8000000000000fffd" },
    };
    inline for (rows) |row| {
        var w = zd.Writer.init(testing.allocator, row.e, .xcdr2);
        defer w.deinit();
        const body = try w.dheaderBegin();
        try w.putU32(7);
        try w.putF64(1.5);
        try w.putU16(0xFFFD);
        w.dheaderEnd(body);
        try expectWire(&w, row.want);

        var r = zd.Reader.init(w.bytes(), row.e, .xcdr2);
        try testing.expectEqual(@as(u32, 14), r.dheaderRead());
        try testing.expectEqual(@as(u32, 7), r.getU32());
        try testing.expectEqual(@as(f64, 1.5), r.getF64());
        try testing.expectEqual(@as(u16, 0xFFFD), r.getU16());
    }
}

// -- ext_mutable, the same three members under PL_CDR2 -----------------------
//
// Cyclone, RTI and Fast DDS write the identical thirty octets. The three length
// codes are the three member widths: LC2 for the `long`, LC3 for the `double`,
// LC1 for the `short`, and none of the three reserves a NEXTINT.

test "ext_mutable xcdr2: EMHEADER LC1, LC2, LC3 (cyclone, rti, fastdds)" {
    const rows = [_]struct { e: zd.Endian, want: []const u8 }{
        .{ .e = .little, .want = "1a000000000000200700000001000030000000000000f83f02000010fdff" },
        .{ .e = .big, .want = "0000001a2000000000000007300000013ff800000000000010000002fffd" },
    };
    inline for (rows) |row| {
        var w = zd.Writer.init(testing.allocator, row.e, .xcdr2);
        defer w.deinit();
        const body = try w.dheaderBegin();
        var m = try w.emHeaderBegin(0, false, .lc2);
        try w.putU32(7);
        try w.emHeaderEnd(m, .lc2);
        m = try w.emHeaderBegin(1, false, .lc3);
        try w.putF64(1.5);
        try w.emHeaderEnd(m, .lc3);
        m = try w.emHeaderBegin(2, false, .lc1);
        try w.putU16(0xFFFD);
        try w.emHeaderEnd(m, .lc1);
        w.dheaderEnd(body);
        try expectWire(&w, row.want);

        // And back: the extent each code announces is the member's own width,
        // with nothing consumed beyond the header.
        var r = zd.Reader.init(w.bytes(), row.e, .xcdr2);
        try testing.expectEqual(@as(u32, 26), r.dheaderRead());
        var member = r.emHeaderRead();
        try testing.expectEqual(@as(u32, 0), member.id);
        try testing.expectEqual(zd.LengthCode.lc2, member.lc);
        try testing.expectEqual(@as(u32, 4), member.extent);
        try testing.expectEqual(@as(u32, 7), r.getU32());
        member = r.emHeaderRead();
        try testing.expectEqual(@as(u32, 1), member.id);
        try testing.expectEqual(@as(u32, 8), member.extent);
        try testing.expectEqual(@as(f64, 1.5), r.getF64());
        member = r.emHeaderRead();
        try testing.expectEqual(@as(u32, 2), member.id);
        try testing.expectEqual(@as(u32, 2), member.extent);
        try testing.expectEqual(@as(u16, 0xFFFD), r.getU16());
    }
}

// The same sample under PL_CDR1, in the short parameter header RTI and Fast DDS
// write. RTI's thirty-two octets are reproduced exactly; Fast DDS differs in one
// place only — it closes the list with the bare 0x3F02 where RTI and Cyclone set
// the must-understand bit and write 0x7F02 — so its row is the same octets with
// `023f` for `027f`, and this SDK writes the majority form.
//
// The `double` is the reason the alignment origin exists: its header sits at
// offset 8..12 and the value follows at 12 with no padding, because §7.4.1.2
// aligns a parameter's value against its own block. Measured from the stream the
// value would land at 16 and the header would declare twelve; all three vendors
// declare eight.
//
// The `short` is the reason the declared length counts the trailing padding: RTI
// declares four for a two-octet value. Fast DDS declares two — a second named
// divergence, and the reason `lc_5_6_7` under XCDR1 is not pinned here at all.

test "ext_mutable xcdr1 short parameter header (rti)" {
    const rows = [_]struct { e: zd.Endian, want: []const u8 }{
        .{ .e = .little, .want = "000004000700000001000800000000000000f83f02000400fdff0000027f0000" },
        .{ .e = .big, .want = "0000000400000007000100083ff800000000000000020004fffd00007f020000" },
    };
    inline for (rows) |row| {
        var w = zd.Writer.init(testing.allocator, row.e, .xcdr1);
        defer w.deinit();
        var p = try w.pidBegin(0, false);
        try w.putU32(7);
        try w.pidEnd(p);
        p = try w.pidBegin(1, false);
        try w.putF64(1.5);
        try w.pidEnd(p);
        p = try w.pidBegin(2, false);
        try w.putU16(0xFFFD);
        try w.pidEnd(p);
        try w.pidListEnd();
        try expectWire(&w, row.want);

        var r = zd.Reader.init(w.bytes(), row.e, .xcdr1);
        var param = r.pidRead();
        try testing.expectEqual(@as(u32, 0), param.id);
        try testing.expectEqual(@as(u32, 4), param.length);
        try testing.expectEqual(@as(u32, 7), r.getU32());
        r.pidSkip(4, param.length);
        param = r.pidRead();
        try testing.expectEqual(@as(u32, 1), param.id);
        try testing.expectEqual(@as(u32, 8), param.length);
        try testing.expectEqual(@as(f64, 1.5), r.getF64());
        r.pidSkip(12, param.length);
        param = r.pidRead();
        try testing.expectEqual(@as(u32, 2), param.id);
        try testing.expectEqual(@as(u32, 4), param.length);
        try testing.expectEqual(@as(u16, 0xFFFD), r.getU16());
        r.pidSkip(24, param.length);
        try testing.expect(r.pidRead().list_end);
    }
}

// The same sample again, in the twelve-octet extended parameter header Cyclone
// writes for every mutable member under PL_CDR1. Fifty-six octets, and the
// must-understand bit is on the PID_EXTENDED marker (0x7F01) while the member's
// own eMemberHeader carries none — which is what makes reading the flag off the
// marker wrong.

test "ext_mutable xcdr1 extended parameter header (cyclone)" {
    const rows = [_]struct { e: zd.Endian, want: []const u8 }{
        .{
            .e = .little,
            .want = "017f0800000000000400000007000000017f08000100000008000000" ++
                "000000000000f83f017f08000200000004000000fdff0000027f0000",
        },
        .{
            .e = .big,
            .want = "7f0100080000000000000004000000077f010008000000010000000" ++
                "83ff80000000000007f0100080000000200000004fffd00007f020000",
        },
    };
    inline for (rows) |row| {
        var w = zd.Writer.init(testing.allocator, row.e, .xcdr1);
        defer w.deinit();
        var p = try w.pidExtendedBegin(0, false);
        try w.putU32(7);
        try w.pidEnd(p);
        p = try w.pidExtendedBegin(1, false);
        try w.putF64(1.5);
        try w.pidEnd(p);
        p = try w.pidExtendedBegin(2, false);
        try w.putU16(0xFFFD);
        try w.pidEnd(p);
        try w.pidListEnd();
        try expectWire(&w, row.want);

        var r = zd.Reader.init(w.bytes(), row.e, .xcdr1);
        const first = r.pidRead();
        try testing.expectEqual(@as(u32, 0), first.id);
        // The marker's flag is not the member's.
        try testing.expect(!first.must_understand);
        try testing.expectEqual(@as(u32, 4), first.length);
        try testing.expectEqual(@as(u32, 7), r.getU32());
    }
}

// -- pid_3f00: @mutable struct PidEdge { @id(16127) long a; @id(16128) long b; }
//
// 16128 is 0x3F00, the highest id a short parameter header can carry: 0x3F01 is
// PID_EXTENDED and 0x3F02 is PID_LIST_END. RTI writes both ids in the short
// form, which is the boundary this case exists to hold.

test "pid_3f00 xcdr1: the short parameter header at its highest id (rti)" {
    const rows = [_]struct { e: zd.Endian, want: []const u8 }{
        .{ .e = .little, .want = "ff3e040007000000003f040009000000027f0000" },
        .{ .e = .big, .want = "3eff0004000000073f000004000000097f020000" },
    };
    inline for (rows) |row| {
        var w = zd.Writer.init(testing.allocator, row.e, .xcdr1);
        defer w.deinit();
        var p = try w.pidBegin(0x3EFF, false);
        try w.putU32(7);
        try w.pidEnd(p);
        p = try w.pidBegin(0x3F00, false);
        try w.putU32(9);
        try w.pidEnd(p);
        try w.pidListEnd();
        try expectWire(&w, row.want);
    }
}

test "pid_3f00 xcdr2: the same ids in an EMHEADER (cyclone, rti, fastdds)" {
    const rows = [_]struct { e: zd.Endian, want: []const u8 }{
        .{ .e = .little, .want = "10000000ff3e002007000000003f002009000000" },
        .{ .e = .big, .want = "0000001020003eff0000000720003f0000000009" },
    };
    inline for (rows) |row| {
        var w = zd.Writer.init(testing.allocator, row.e, .xcdr2);
        defer w.deinit();
        const body = try w.dheaderBegin();
        var m = try w.emHeaderBegin(0x3EFF, false, .lc2);
        try w.putU32(7);
        try w.emHeaderEnd(m, .lc2);
        m = try w.emHeaderBegin(0x3F00, false, .lc2);
        try w.putU32(9);
        try w.emHeaderEnd(m, .lc2);
        w.dheaderEnd(body);
        try expectWire(&w, row.want);
    }
}

// -- lc_5_6_7: the four NEXTINT-bearing length codes in one sample -----------
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

test "lc_5_6_7 xcdr2: LC4, LC5, LC6 and LC7 (cyclone)" {
    const rows = [_]struct { e: zd.Endian, want: []const u8 }{
        .{
            .e = .little,
            .want = "440000000100005004000000616263000200005003000000010203" ++
                "000300006001000000070000000400007001000000000000000000f83f" ++
                "05000040080000000200000004000500",
        },
        .{
            .e = .big,
            .want = "00000044500000010000000461626300500000020000000301020300" ++
                "60000003000000010000000770000004000000013ff8000000000000" ++
                "40000005000000080000000200040005",
        },
    };
    inline for (rows) |row| {
        var w = zd.Writer.init(testing.allocator, row.e, .xcdr2);
        defer w.deinit();
        const body = try w.dheaderBegin();
        var m = try w.emHeaderBegin(1, false, .lc5);
        try w.putString("abc");
        try w.emHeaderEnd(m, .lc5);
        m = try w.emHeaderBegin(2, false, .lc5);
        try w.putSeqU8(&[_]u8{ 1, 2, 3 });
        try w.emHeaderEnd(m, .lc5);
        m = try w.emHeaderBegin(3, false, .lc6);
        try w.putU32(1);
        try w.putU32(7);
        try w.emHeaderEnd(m, .lc6);
        m = try w.emHeaderBegin(4, false, .lc7);
        try w.putU32(1);
        try w.putF64(1.5);
        try w.emHeaderEnd(m, .lc7);
        m = try w.emHeaderBegin(5, false, .lc4);
        try w.putU32(2);
        try w.putU16(4);
        try w.putU16(5);
        try w.emHeaderEnd(m, .lc4);
        w.dheaderEnd(body);
        try expectWire(&w, row.want);

        // Reading Cyclone's own octets: each code's extent counts what the
        // spec says it counts, and LC5..LC7 leave the leading word in place.
        var r = zd.Reader.init(w.bytes(), row.e, .xcdr2);
        try testing.expectEqual(@as(u32, 68), r.dheaderRead());
        var member = r.emHeaderRead();
        try testing.expectEqual(zd.LengthCode.lc5, member.lc);
        try testing.expectEqual(@as(u32, 8), member.extent); // 4 + 4
        try testing.expectEqualStrings("abc", r.getString());
        member = r.emHeaderRead();
        try testing.expectEqual(@as(u32, 7), member.extent); // 4 + 3
        try testing.expectEqualSlices(u8, &[_]u8{ 1, 2, 3 }, r.getSeqU8());
        member = r.emHeaderRead();
        try testing.expectEqual(zd.LengthCode.lc6, member.lc);
        try testing.expectEqual(@as(u32, 8), member.extent); // 4 + 4×1
        try testing.expectEqual(@as(u32, 1), r.getU32());
        try testing.expectEqual(@as(u32, 7), r.getU32());
        member = r.emHeaderRead();
        try testing.expectEqual(zd.LengthCode.lc7, member.lc);
        try testing.expectEqual(@as(u32, 12), member.extent); // 4 + 8×1
        try testing.expectEqual(@as(u32, 1), r.getU32());
        try testing.expectEqual(@as(f64, 1.5), r.getF64());
        member = r.emHeaderRead();
        try testing.expectEqual(zd.LengthCode.lc4, member.lc);
        try testing.expectEqual(@as(u32, 8), member.extent);
    }
}

// -- wchar_member: @final struct WCh { wchar c; octet tail; } ----------------
//
// c = U+0061, tail = 0xAB. Cyclone and Fast DDS write two octets under both
// encodings and RTI does under XCDR2. RTI's XCDR1 `wchar_t` is four octets wide
// (`61000000ab`) — the one vendor answer this SDK does not reproduce, and it is
// RTI alone.

test "wchar_member: two octets in the stream's byte order (cyclone, fastdds)" {
    const rows = [_]struct { e: zd.Endian, want: []const u8 }{
        .{ .e = .little, .want = "6100ab" },
        .{ .e = .big, .want = "0061ab" },
    };
    inline for (rows) |row| {
        inline for ([_]zd.Encoding{ .xcdr1, .xcdr2 }) |encoding| {
            var w = zd.Writer.init(testing.allocator, row.e, encoding);
            defer w.deinit();
            try w.putWchar(0x61);
            try w.putU8(0xAB);
            try expectWire(&w, row.want);

            var r = zd.Reader.init(w.bytes(), row.e, encoding);
            try testing.expectEqual(@as(u32, 0x61), r.getWchar());
            try testing.expectEqual(@as(u8, 0xAB), r.getU8());
        }
    }
}

// -- wstring_member / wstring_nonbmp -----------------------------------------
//
// @final struct WStr { wstring w; octet tail; }, w = "ab" then U+1D11E.
//
// The little-endian rows are Cyclone's under both encodings and RTI's under
// XCDR2: the length word counts four octets for two code units, there is no
// terminator and no byte-order mark. Fast DDS writes the unit *count* there
// (`02000000`) — a length that means something else — and truncates U+1D11E to
// one unit (`010000001ed1`); neither is reproduced.
//
// The big-endian row is RTI's under XCDR2 alone. Cyclone byte-swaps the length
// word but leaves the code units little-endian (`0000000461006200ab`), which no
// reading of §7.4.1 supports and which this SDK does not reproduce either.

test "wstring_member: the length counts octets, units follow the stream" {
    var w = zd.Writer.init(testing.allocator, .little, .xcdr2);
    defer w.deinit();
    try w.putWstring(&[_]u16{ 0x61, 0x62 });
    try w.putU8(0xAB);
    try expectWire(&w, "0400000061006200ab"); // cyclone xcdr1+xcdr2, rti xcdr2

    var be = zd.Writer.init(testing.allocator, .big, .xcdr2);
    defer be.deinit();
    try be.putWstring(&[_]u16{ 0x61, 0x62 });
    try be.putU8(0xAB);
    try expectWire(&be, "0000000400610062ab"); // rti xcdr2

    var units: [8]u16 = undefined;
    var r = zd.Reader.init(be.bytes(), .big, .xcdr2);
    try testing.expectEqualSlices(u16, &[_]u16{ 0x61, 0x62 }, try r.getWstring(&units));
    try testing.expectEqual(@as(u8, 0xAB), r.getU8());
}

test "wstring_nonbmp: a surrogate pair is two units and four octets" {
    var w = zd.Writer.init(testing.allocator, .little, .xcdr2);
    defer w.deinit();
    try w.putWstring(&[_]u16{ 0xD834, 0xDD1E });
    try w.putU8(0xAB);
    try expectWire(&w, "0400000034d81eddab"); // cyclone xcdr1+xcdr2, rti xcdr2

    var be = zd.Writer.init(testing.allocator, .big, .xcdr2);
    defer be.deinit();
    try be.putWstring(&[_]u16{ 0xD834, 0xDD1E });
    try be.putU8(0xAB);
    try expectWire(&be, "00000004d834dd1eab"); // rti xcdr2

    var units: [8]u16 = undefined;
    var r = zd.Reader.init(w.bytes(), .little, .xcdr2);
    try testing.expectEqualSlices(u16, &[_]u16{ 0xD834, 0xDD1E }, try r.getWstring(&units));
}

// -- bitset ------------------------------------------------------------------
//
// `bitset Packed { bitfield<3> low; bitfield<5> high; }` with low = 5 and
// high = 17 packs into one octet: 5 | (17 << 3) = 0x8D. Fast DDS wrote
// `8dab` for all four configurations of `tests/interop-corpus/bitset-basic`
// (Cyclone, RTI and OpenDDS have no `bitset` in their IDL front ends and left
// the cell empty).
//
// `bitset Wide { bitfield<20> first; bitfield<20> second; bitfield<24> third; }`
// with first = 1048575, second = 0, third = 12345678 packs into eight octets:
// 0xBC614E00000FFFFF. The anchor's width is the alignment, which is what makes
// it a separate call from `putU64`.

test "bitset-basic: the one-octet anchor and its packing (fastdds)" {
    var w = zd.Writer.init(testing.allocator, .little, .xcdr1);
    defer w.deinit();
    try w.putBitset(5 | (17 << 3), 1);
    try w.putU8(0xAB);
    try expectWire(&w, "8dab");

    var r = zd.Reader.init(w.bytes(), .little, .xcdr1);
    try testing.expectEqual(@as(u64, 0x8D), try r.getBitset(1));
}

test "bitset-multiword: the eight-octet anchor (fastdds)" {
    const rows = [_]struct { e: zd.Endian, want: []const u8 }{
        .{ .e = .little, .want = "ffff0f00004e61bcab" },
        .{ .e = .big, .want = "bc614e00000fffffab" },
    };
    inline for (rows) |row| {
        inline for ([_]zd.Encoding{ .xcdr1, .xcdr2 }) |encoding| {
            var w = zd.Writer.init(testing.allocator, row.e, encoding);
            defer w.deinit();
            try w.putBitset(0xBC614E00000FFFFF, 8);
            try w.putU8(0xAB);
            try expectWire(&w, row.want);

            var r = zd.Reader.init(w.bytes(), row.e, encoding);
            try testing.expectEqual(@as(u64, 0xBC614E00000FFFFF), try r.getBitset(8));
        }
    }
}

// -- fixed<9,3> --------------------------------------------------------------
//
// `tests/interop-corpus/fixed-basic`: amount = "123456.789", debt =
// "-000000.001", tail = 0xAB. omniORB wrote `123456789c 000000001d ab` in both
// byte orders — BCD has no byte order, which is the property the two identical
// files state. JacORB writes `123456789c 1dab`: it drops the leading zero octets
// of the negative value, so its frame is four octets shorter and cannot be
// parsed against the declared digit count. omniORB's is the form reproduced.

test "fixed-basic: packed BCD, both signs, no byte order (omniorb)" {
    inline for ([_]zd.Endian{ .little, .big }) |endian| {
        var w = zd.Writer.init(testing.allocator, endian, .xcdr1);
        defer w.deinit();
        try w.putFixed("123456.789", 9, 3);
        try w.putFixed("-000000.001", 9, 3);
        try w.putU8(0xAB);
        try expectWire(&w, "123456789c" ++ "000000001d" ++ "ab");

        var text: [16]u8 = undefined;
        var r = zd.Reader.init(w.bytes(), endian, .xcdr1);
        try testing.expectEqualStrings("123456.789", try r.getFixed(9, 3, &text));
        try testing.expectEqualStrings("-000000.001", try r.getFixed(9, 3, &text));
        try testing.expectEqual(@as(u8, 0xAB), r.getU8());
    }
}

// ============================ the range edges ============================
//
// The last sweep found four defects and every one of them was at an end of a
// range, not in the middle: an Ada 83 divide that truncated toward zero for
// every negative argument, a Zig frame writer that ran past its output buffer,
// and nine of fifteen bindings answering `put_u16(70000)` with `7011`. So each
// operation is asked here what it does at 0, at −1, at the minimum, at the
// maximum, and one past the maximum.

test "the integer writes are typed to their wire width" {
    // Zig refuses `putU16(70000)` at compile time: the parameter is `u16` and
    // 70000 is not one, so the truncation nine of the fifteen bindings
    // performed silently has no spelling here at all. What is measurable is
    // that the extremes go out whole.
    var w = zd.Writer.init(testing.allocator, .little, .xcdr2);
    defer w.deinit();
    try w.putU8(0);
    try w.putU8(255);
    try w.putU16(0);
    try w.putU16(65535);
    try w.putU32(0);
    try w.putU32(4294967295);
    try w.putU64(0);
    try w.putU64(18446744073709551615);
    // 00 ff | 0000 ffff | 0000 (the u32's padding) | 00000000 ffffffff
    // | 0000000000000000 ffffffffffffffff
    try expectWire(&w, "00ff0000ffff000000000000ffffffff" ++
        "0000000000000000ffffffffffffffff");

    var r = zd.Reader.init(w.bytes(), .little, .xcdr2);
    try testing.expectEqual(@as(u8, 0), r.getU8());
    try testing.expectEqual(@as(u8, 255), r.getU8());
    try testing.expectEqual(@as(u16, 0), r.getU16());
    try testing.expectEqual(@as(u16, 65535), r.getU16());
    try testing.expectEqual(@as(u32, 0), r.getU32());
    try testing.expectEqual(@as(u32, 4294967295), r.getU32());
    try testing.expectEqual(@as(u64, 0), r.getU64());
    try testing.expectEqual(@as(u64, 18446744073709551615), r.getU64());
}

test "the signed extremes reach the wire as their two's complement" {
    var w = zd.Writer.init(testing.allocator, .little, .xcdr2);
    defer w.deinit();
    try w.putU8(@as(u8, @bitCast(@as(i8, -1))));
    try w.putU8(@as(u8, @bitCast(@as(i8, -128))));
    try w.putU16(@as(u16, @bitCast(@as(i16, -1))));
    try w.putU16(@as(u16, @bitCast(@as(i16, -32768))));
    try w.putU32(@as(u32, @bitCast(@as(i32, -1))));
    try w.putU32(@as(u32, @bitCast(@as(i32, -2147483648))));
    try w.putU64(@as(u64, @bitCast(@as(i64, -1))));
    try w.putU64(@as(u64, @bitCast(@as(i64, -9223372036854775808))));
    try expectWire(&w, "ff80ffff00800000ffffffff00000080" ++
        "ffffffffffffffff0000000000000080");
}

test "putU64 under XCDR1 aligns to eight and the reader agrees" {
    // The Go column's third commit: `GetU64` passed four where `PutU64` passed
    // eight, so 0xFFFFFFFFFFFFFFFF read back as 0xFFFFFFFF00000000 with no
    // error anywhere. One octet first, so the value is not already aligned.
    var w = zd.Writer.init(testing.allocator, .little, .xcdr1);
    defer w.deinit();
    try w.putU8(0xAB);
    try w.putU64(0xFFFF_FFFF_FFFF_FFFF);
    try expectWire(&w, "ab00000000000000ffffffffffffffff");

    var r = zd.Reader.init(w.bytes(), .little, .xcdr1);
    try testing.expectEqual(@as(u8, 0xAB), r.getU8());
    try testing.expectEqual(@as(u64, 0xFFFF_FFFF_FFFF_FFFF), r.getU64());
}

test "putF64 at the ends of binary64" {
    const values = [_]f64{ 0.0, -0.0, 1.5, -1.5, std.math.floatMin(f64), std.math.floatMax(f64) };
    for (values) |v| {
        var w = zd.Writer.init(testing.allocator, .big, .xcdr1);
        defer w.deinit();
        try w.putF64(v);
        var r = zd.Reader.init(w.bytes(), .big, .xcdr1);
        const back = r.getF64();
        try testing.expectEqual(@as(u64, @bitCast(v)), @as(u64, @bitCast(back)));
    }
}

test "putChar refuses above U+00FF instead of truncating" {
    var w = zd.Writer.init(testing.allocator, .little, .xcdr2);
    defer w.deinit();
    try w.putChar(0); // the minimum
    try w.putChar(0xFF); // the maximum
    try expectWire(&w, "00ff");
    // U+0161 truncates to 0x61, which is `a`: a frame of the right length
    // holding a different, plausible character.
    try testing.expectError(error.CharAboveLatin1, w.putChar(0x161));
    try testing.expectError(error.CharAboveLatin1, w.putChar(0x100));
    // Nothing was appended by the refusal.
    try expectWire(&w, "00ff");
}

test "putWchar refuses above the basic plane" {
    var w = zd.Writer.init(testing.allocator, .little, .xcdr2);
    defer w.deinit();
    try w.putWchar(0);
    try w.putWchar(0xFFFF);
    try expectWire(&w, "0000ffff");
    try testing.expectError(error.WcharAboveBmp, w.putWchar(0x10000));
    try testing.expectError(error.WcharAboveBmp, w.putWchar(0x1D11E));
    try expectWire(&w, "0000ffff");
}

test "putWstring at zero units and at the surrogate boundary" {
    var w = zd.Writer.init(testing.allocator, .little, .xcdr2);
    defer w.deinit();
    try w.putWstring(&[_]u16{});
    try expectWire(&w, "00000000");

    var one = zd.Writer.init(testing.allocator, .little, .xcdr2);
    defer one.deinit();
    try one.putWstring(&[_]u16{0xFFFF});
    try expectWire(&one, "02000000ffff");
}

test "the member id boundaries of the EMHEADER and of the two parameter headers" {
    var w = zd.Writer.init(testing.allocator, .little, .xcdr2);
    defer w.deinit();
    // 28 bits, so 0x0FFFFFFF is in and 0x10000000 is not.
    const m = try w.emHeaderBegin(0x0FFF_FFFF, true, .lc2);
    try w.putU32(0);
    try w.emHeaderEnd(m, .lc2);
    // 0xAFFFFFFF = must-understand | LC2 | 0x0FFFFFFF
    try expectWire(&w, "ffffffaf00000000");
    try testing.expectError(error.MemberIdTooWide, w.emHeaderBegin(0x1000_0000, false, .lc2));

    var p = zd.Writer.init(testing.allocator, .little, .xcdr1);
    defer p.deinit();
    // 0x3F00 is the last short id; 0x3F01 is PID_EXTENDED.
    const short = try p.pidBegin(0x3F00, false);
    try p.pidEnd(short);
    try testing.expectError(error.ParameterIdTooWide, p.pidBegin(0x3F01, false));
    try testing.expectError(error.MemberIdTooWide, p.pidExtendedBegin(0x1000_0000, false));
}

test "the must-understand bit is bit 31 of the EMHEADER and bit 14 of a PID" {
    var w = zd.Writer.init(testing.allocator, .little, .xcdr2);
    defer w.deinit();
    const m = try w.emHeaderBegin(1, true, .lc2);
    try w.putU32(0);
    try w.emHeaderEnd(m, .lc2);
    try expectWire(&w, "010000a000000000"); // 0xA0000001 = MU | LC2 | 1

    var p = zd.Writer.init(testing.allocator, .little, .xcdr1);
    defer p.deinit();
    const one = try p.pidBegin(1, true);
    try p.putU32(0);
    try p.pidEnd(one);
    try expectWire(&p, "0140040000000000"); // 0x4001 = MU | 1
}

test "emHeaderEnd refuses a body its length code cannot describe" {
    var w = zd.Writer.init(testing.allocator, .little, .xcdr2);
    defer w.deinit();
    const m = try w.emHeaderBegin(0, false, .lc2);
    try w.putU16(1); // two octets where LC2 says four
    try testing.expectError(error.LengthCodeMismatch, w.emHeaderEnd(m, .lc2));

    var short = zd.Writer.init(testing.allocator, .little, .xcdr2);
    defer short.deinit();
    const n = try short.emHeaderBegin(0, false, .lc5);
    try short.putU16(1); // LC5 takes its length from four octets that are not there
    try testing.expectError(error.LengthCodeMismatch, short.emHeaderEnd(n, .lc5));
}

test "putBitset takes the four anchor widths and refuses the rest" {
    var w = zd.Writer.init(testing.allocator, .little, .xcdr2);
    defer w.deinit();
    try w.putBitset(0xFF, 1);
    try w.putBitset(0xFFFF, 2);
    try w.putBitset(0xFFFF_FFFF, 4);
    try w.putBitset(0xFFFF_FFFF_FFFF_FFFF, 8);
    try expectWire(&w, "ff00ffff" ++ "ffffffff" ++ "ffffffffffffffff");
    try testing.expectError(error.BitsetAnchorInvalid, w.putBitset(0, 0));
    try testing.expectError(error.BitsetAnchorInvalid, w.putBitset(0, 3));
    try testing.expectError(error.BitsetAnchorInvalid, w.putBitset(0, 16));
}

test "fixed at the ends of its range" {
    var out: [24]u8 = undefined;

    // The narrowest type: one digit, no fraction. (1+1) is even, so no leading
    // zero nibble, and the frame is one octet.
    try testing.expectEqualSlices(u8, &hex("0c"), try zd.fixedOctets("0", 1, 0, &out));
    try testing.expectEqualSlices(u8, &hex("9c"), try zd.fixedOctets("9", 1, 0, &out));
    try testing.expectEqualSlices(u8, &hex("9d"), try zd.fixedOctets("-9", 1, 0, &out));

    // A negative zero is one zero, and its sign nibble is 0xC. A writer that
    // took the sign from the text would put 0xD there and two encoders of the
    // same value would disagree.
    try testing.expectEqualSlices(u8, &hex("0c"), try zd.fixedOctets("-0", 1, 0, &out));
    try testing.expectEqualSlices(u8, &hex("000000000c"), try zd.fixedOctets("-0.000", 9, 3, &out));

    // An odd digit count pairs the digits with the sign by prepending a zero
    // nibble: fixed<2,0> is (2+1) odd, so three nibbles become two octets.
    try testing.expectEqualSlices(u8, &hex("012c"), try zd.fixedOctets("12", 2, 0, &out));

    // The widest an IDL `fixed` may be. 31 digits and a sign is 32 nibbles, so
    // sixteen octets and no leading pad.
    const wide = try zd.fixedOctets("1234567890123456789012345678901", 31, 0, &out);
    try testing.expectEqual(@as(usize, 16), wide.len);
    try testing.expectEqualSlices(u8, &hex("1234567890123456789012345678901c"), wide);

    // scale == digits: every digit is fraction.
    try testing.expectEqualSlices(u8, &hex("012c"), try zd.fixedOctets("0.12", 2, 2, &out));

    // One past each end.
    try testing.expectError(error.FixedDigitsOutOfRange, zd.fixedOctets("1", 0, 0, &out));
    try testing.expectError(error.FixedDigitsOutOfRange, zd.fixedOctets("1", 32, 0, &out));
    try testing.expectError(error.FixedScaleOutOfRange, zd.fixedOctets("1", 2, 3, &out));
    try testing.expectError(error.FixedIntegerPartTooWide, zd.fixedOctets("100", 2, 0, &out));
    try testing.expectError(error.FixedFractionTooWide, zd.fixedOctets("0.123", 3, 2, &out));
    try testing.expectError(error.FixedNotADecimal, zd.fixedOctets("1e3", 3, 0, &out));
    try testing.expectError(error.FixedNotADecimal, zd.fixedOctets("-", 3, 0, &out));
}

test "fixed round-trips through the reader at both signs" {
    const cases = [_]struct { text: []const u8, digits: usize, scale: usize, back: []const u8 }{
        .{ .text = "123456.789", .digits = 9, .scale = 3, .back = "123456.789" },
        .{ .text = "-000000.001", .digits = 9, .scale = 3, .back = "-000000.001" },
        .{ .text = "0", .digits = 1, .scale = 0, .back = "0" },
        .{ .text = "-9", .digits = 1, .scale = 0, .back = "-9" },
        // scale == digits, so there is no integer digit and the point leads.
        .{ .text = "0.12", .digits = 2, .scale = 2, .back = ".12" },
    };
    for (cases) |c| {
        var w = zd.Writer.init(testing.allocator, .little, .xcdr1);
        defer w.deinit();
        try w.putFixed(c.text, c.digits, c.scale);
        var text: [40]u8 = undefined;
        var r = zd.Reader.init(w.bytes(), .little, .xcdr1);
        try testing.expectEqualStrings(c.back, try r.getFixed(c.digits, c.scale, &text));
    }
}

test "putBool writes one octet and nothing else" {
    var w = zd.Writer.init(testing.allocator, .big, .xcdr1);
    defer w.deinit();
    try w.putBool(false);
    try w.putBool(true);
    try expectWire(&w, "0001");
    var r = zd.Reader.init(w.bytes(), .big, .xcdr1);
    try testing.expect(!r.getBool());
    try testing.expect(r.getBool());
}

test "a DHEADER of zero is a body of no octets, and one of the maximum is not truncated" {
    var w = zd.Writer.init(testing.allocator, .little, .xcdr2);
    defer w.deinit();
    const body = try w.dheaderBegin();
    w.dheaderEnd(body);
    try expectWire(&w, "00000000");

    var r = zd.Reader.init(w.bytes(), .little, .xcdr2);
    try testing.expectEqual(@as(u32, 0), r.dheaderRead());
}
