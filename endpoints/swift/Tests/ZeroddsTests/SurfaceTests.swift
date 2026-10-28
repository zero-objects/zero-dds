// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// The wire surface, against octets this project did not write.
//
// Every expected string below is a row of
// `crates/ast-layout/tests/fixtures/vendor_bytes.rows.tsv`, which holds what
// Cyclone DDS, RTI Connext, Fast DDS and OpenDDS each produced from their own
// serialiser for the cases under `tools/byte-gate/cases/`. The vendor and the
// row are named at every check, so a change here that agreed with itself and
// not with a foreign implementation fails.
//
// Where the vendors disagree the divergence is written down rather than
// averaged away: a case with three answers is three findings, and this SDK
// follows a named one.
//
// Each operation is exercised at the ends of its range and not only in the
// middle, because that is where the last four defects in these bindings were.

import XCTest

@testable import Zerodds

private func hex(_ b: [UInt8]) -> String {
    b.map { String(format: "%02x", $0) }.joined()
}

private func unhex(_ s: String) -> [UInt8] {
    var out: [UInt8] = []
    var i = s.startIndex
    while i < s.endIndex {
        let j = s.index(i, offsetBy: 2)
        out.append(UInt8(s[i..<j], radix: 16)!)
        i = j
    }
    return out
}

final class SurfaceTests: XCTestCase {

    // --- the encoding decides MAXALIGN ------------------------------------

    func testExtFinalTheEncodingDecidesWhereTheDoubleLands() throws {
        // byte-gate `ext_final`: `@final struct Ext { long a; double b; short c; }`
        // with a = 7, b = 1.5, c = -3. Cyclone, RTI and Fast DDS agree on all
        // four rows. The `double` aligns to eight under XCDR1 and to four under
        // XCDR2, so the same sample is 18 octets in one encoding and 14 in the
        // other — which is the whole of what the encoding argument buys.
        for (endian, encoding, want) in [
            (Endianness.little, Encoding.xcdr1, "0700000000000000000000000000f83ffdff"),
            (Endianness.little, Encoding.xcdr2, "07000000000000000000f83ffdff"),
            (Endianness.big, Encoding.xcdr1, "00000007000000003ff8000000000000fffd"),
            (Endianness.big, Encoding.xcdr2, "000000073ff8000000000000fffd"),
        ] {
            var w = Writer(endian, encoding)
            w.putU32(7)
            w.putF64(1.5)
            w.putU16(0xFFFD)
            XCTAssertEqual(hex(w.bytes()), want, "cyclone+rti+fastdds")
        }
        XCTAssertEqual(Writer(.little, .xcdr1).maxAlign, 8)
        XCTAssertEqual(Writer(.little, .xcdr2).maxAlign, 4)
    }

    // --- DHEADER -----------------------------------------------------------

    func testExtAppendableTheDheaderCountsTheBodyAndNotItself() throws {
        // byte-gate `ext_appendable`: the same members under `@appendable`,
        // which XTypes 1.3 §7.3.1.2.1.8 makes the default extensibility. Under
        // XCDR2 a DHEADER precedes the body and counts it — 0x0e = 14 over an
        // 18-octet sample. Cyclone, RTI and Fast DDS agree on both rows.
        for (endian, want) in [
            (Endianness.little, "0e00000007000000000000000000f83ffdff"),
            (Endianness.big, "0000000e000000073ff8000000000000fffd"),
        ] {
            var w = Writer(endian, .xcdr2)
            let d = w.dheaderBegin()
            w.putU32(7)
            w.putF64(1.5)
            w.putU16(0xFFFD)
            w.dheaderEnd(d)
            XCTAssertEqual(hex(w.bytes()), want, "cyclone+rti+fastdds")
        }
    }

    // --- the member header -------------------------------------------------

    func testExtMutableTheEmheaderInTheLengthCodesTheVendorsChose() throws {
        // byte-gate `ext_mutable` under XCDR2: all three vendors write the same
        // 30 octets. The member headers are LC2, LC3 and LC1 — the length is in
        // the code and no NEXTINT is written for any of them — inside a DHEADER
        // of 26.
        for (endian, want) in [
            (Endianness.little, "1a000000000000200700000001000030000000000000f83f02000010fdff"),
            (Endianness.big, "0000001a2000000000000007300000013ff800000000000010000002fffd"),
        ] {
            var w = Writer(endian, .xcdr2)
            let d = w.dheaderBegin()
            var m = try w.emheaderBegin(id: 0, mustUnderstand: false, lengthCode: .lc2)
            w.putU32(7)
            try w.emheaderEnd(m, lengthCode: .lc2)
            m = try w.emheaderBegin(id: 1, mustUnderstand: false, lengthCode: .lc3)
            w.putF64(1.5)
            try w.emheaderEnd(m, lengthCode: .lc3)
            m = try w.emheaderBegin(id: 2, mustUnderstand: false, lengthCode: .lc1)
            w.putU16(0xFFFD)
            try w.emheaderEnd(m, lengthCode: .lc1)
            w.dheaderEnd(d)
            XCTAssertEqual(hex(w.bytes()), want, "cyclone+rti+fastdds")
        }
    }

    func testMustUnderstandIsBitThirtyOneOfTheEmheader() throws {
        // `mut_mustunderstand` under XCDR2 reads `010000a0` for id 1 at LC2 with
        // the flag — Cyclone and RTI both write it. Fast DDS writes `01000020`
        // for the same member, i.e. it drops the flag; two of three carry it and
        // this SDK carries it.
        var w = Writer(.little, .xcdr2)
        let d = w.dheaderBegin()
        var m = try w.emheaderBegin(id: 1, mustUnderstand: true, lengthCode: .lc2)
        w.putU32(7)
        try w.emheaderEnd(m, lengthCode: .lc2)
        m = try w.emheaderBegin(id: 2, mustUnderstand: false, lengthCode: .lc2)
        w.putU32(0xFFFF_FFFE)
        try w.emheaderEnd(m, lengthCode: .lc2)
        w.dheaderEnd(d)
        XCTAssertEqual(
            hex(w.bytes()), "10000000010000a00700000002000020feffffff",
            "cyclone+rti; fastdds drops bit 31")
    }

    func testTheMemberHeaderRefusesWhatItCannotFrame() throws {
        // The top of the range: 0x0FFFFFFF fits and 0x10000000 does not. A
        // truncating header would put a different member's id on the wire.
        var ok = Writer(.little, .xcdr2)
        _ = try ok.emheaderBegin(id: 0x0FFF_FFFF, mustUnderstand: false, lengthCode: .lc2)
        var past = Writer(.little, .xcdr2)
        XCTAssertThrowsError(
            try past.emheaderBegin(id: 0x1000_0000, mustUnderstand: false, lengthCode: .lc2))
        var short = Writer(.little, .xcdr2)
        let m = try short.emheaderBegin(id: 0, mustUnderstand: false, lengthCode: .lc2)
        short.putU16(1)
        XCTAssertThrowsError(try short.emheaderEnd(m, lengthCode: .lc2))
    }

    // --- the PL_CDR1 parameter list ----------------------------------------

    func testExtMutableUnderXcdr1IsRtisParameterList() throws {
        // byte-gate `ext_mutable` under XCDR1, RTI's row. Three things it pins:
        // the `double`'s parameter declares 8 and not 12, because its value
        // aligns against its own block; the `short`'s declares 4 and not 2,
        // because the padding that brings the next header back to a four-octet
        // boundary is inside the length; and the sentinel carries the
        // must-understand flag, so it is 0x7F02.
        //
        // The vendors do not agree here and the disagreement is the measurement:
        // Cyclone writes every parameter in the twelve-octet extended form (56
        // octets for this sample), Fast DDS declares 2 for the `short` and
        // closes with the bare 0x3F02 (32 octets, differing from RTI in two
        // places).
        for (endian, want) in [
            (
                Endianness.little,
                "000004000700000001000800000000000000f83f02000400fdff0000027f0000"
            ),
            (Endianness.big, "0000000400000007000100083ff800000000000000020004fffd00007f020000"),
        ] {
            var w = Writer(endian, .xcdr1)
            var p = try w.pidBegin(0, mustUnderstand: false)
            w.putU32(7)
            try w.pidEnd(p)
            p = try w.pidBegin(1, mustUnderstand: false)
            w.putF64(1.5)
            try w.pidEnd(p)
            p = try w.pidBegin(2, mustUnderstand: false)
            w.putU16(0xFFFD)
            try w.pidEnd(p)
            w.pidListEnd()
            XCTAssertEqual(hex(w.bytes()), want, "rti; cyclone extended, fastdds len 2 + 3f02")
        }
    }

    func testTheParameterMustUnderstandFlagIsBitFourteen() throws {
        // RTI's `mut_mustunderstand` row reads `0140` for id 1 with the flag,
        // and Cyclone sets the same bit inside its extended header. Fast DDS
        // writes `0100` — no flag.
        var w = Writer(.little, .xcdr1)
        var p = try w.pidBegin(1, mustUnderstand: true)
        w.putU32(7)
        try w.pidEnd(p)
        p = try w.pidBegin(2, mustUnderstand: false)
        w.putU32(0xFFFF_FFFE)
        try w.pidEnd(p)
        w.pidListEnd()
        XCTAssertEqual(
            hex(w.bytes()), "014004000700000002000400feffffff027f0000",
            "rti; fastdds drops bit 14")
    }

    func testPid3f00IsTheLastIdTheShortHeaderCarries() throws {
        // byte-gate `pid_3f00`: `@id(16127)` and `@id(16128)`, i.e. 0x3EFF and
        // 0x3F00. RTI and Fast DDS both still use the short header for 0x3F00,
        // so the boundary is inclusive — and one past it is refused rather than
        // truncated.
        var w = Writer(.little, .xcdr1)
        var p = try w.pidBegin(0x3EFF, mustUnderstand: false)
        w.putU32(7)
        try w.pidEnd(p)
        p = try w.pidBegin(0x3F00, mustUnderstand: false)
        w.putU32(9)
        try w.pidEnd(p)
        w.pidListEnd()
        XCTAssertEqual(
            hex(w.bytes()), "ff3e040007000000003f040009000000027f0000",
            "rti+fastdds; cyclone extended")
        var past = Writer(.little, .xcdr1)
        XCTAssertThrowsError(try past.pidBegin(0x3F01, mustUnderstand: false))
    }

    // --- the tail is not padded --------------------------------------------

    func testTailPadASampleIsNotRoundedUpAtTheEnd() throws {
        // byte-gate `tail_pad`: `@final struct Odd { long a; short b; }` with 1
        // and 2 is six octets in all four rows and all three vendors. A writer
        // that rounded the sample up to a four-octet boundary would add two.
        for (endian, want) in [
            (Endianness.little, "010000000200"), (Endianness.big, "000000010002"),
        ] {
            for encoding in [Encoding.xcdr1, Encoding.xcdr2] {
                var w = Writer(endian, encoding)
                w.putU32(1)
                w.putU16(2)
                XCTAssertEqual(hex(w.bytes()), want, "cyclone+rti+fastdds")
            }
        }
    }

    // --- the wide character and string -------------------------------------

    func testWcharIsTwoOctetsInTheStreamsOrder() throws {
        // byte-gate `wchar_member`: `@final struct WCh { wchar c; octet tail; }`
        // with U+0061 and 0xAB. Cyclone and Fast DDS write a two-octet code unit
        // in both encodings; RTI writes four octets under XCDR1 and two under
        // XCDR2. This SDK writes two octets always — the majority form, and
        // RTI's own XCDR2 form.
        for (endian, want) in [(Endianness.little, "6100ab"), (Endianness.big, "0061ab")] {
            for encoding in [Encoding.xcdr1, Encoding.xcdr2] {
                var w = Writer(endian, encoding)
                try w.putWchar(0x0061)
                w.putU8(0xAB)
                XCTAssertEqual(
                    hex(w.bytes()), want, "cyclone+fastdds; rti 4 octets under XCDR1")
            }
        }
        // U+FFFF is the last one that fits and U+10000 the first that does not.
        var ok = Writer(.little, .xcdr2)
        try ok.putWchar(0xFFFF)
        var past = Writer(.little, .xcdr2)
        XCTAssertThrowsError(try past.putWchar(0x10000))
    }

    func testWstringLengthCountsOctets() throws {
        // byte-gate `wstring_member`: `wstring w; octet tail;` with the two
        // units U+0061 U+0062. Three vendors, three answers, and the length word
        // is the axis they split on:
        //
        //   cyclone  04000000 6100 6200 ab  — the length counts OCTETS
        //   fastdds  02000000 6100 6200 ab  — the length counts CODE UNITS
        //   rti      03000000 ...           — units incl. a terminator, four
        //            octets wide, under XCDR1; under XCDR2 RTI writes the octet
        //            count.
        //
        // This SDK writes the octet count with two-octet units and no
        // terminator.
        //
        // A fourth axis separates them in a big-endian stream, and only there:
        // Cyclone's row is `00000004 6100 6200 ab` — the length word is swapped
        // and the code units are not, so its wide units go out little-endian
        // whatever the stream says. RTI's row is `00000004 0061 0062 ab`, every
        // word in the stream's order. Cyclone swaps a standalone `wchar` (its
        // `wchar_member` row is `0061ab`) and does not swap the elements of a
        // `wstring`, which is a disagreement inside one vendor. This SDK writes
        // every unit in the stream's order — RTI's form, and Cyclone's own form
        // for a `wchar`.
        for (endian, want) in [
            (Endianness.little, "0400000061006200ab"),  // cyclone + rti agree
            (Endianness.big, "0000000400610062ab"),  // rti; cyclone leaves the units LE
        ] {
            var w = Writer(endian, .xcdr2)
            w.putWstring([0x0061, 0x0062])
            w.putU8(0xAB)
            XCTAssertEqual(hex(w.bytes()), want, "rti; fastdds counts units")
        }
        // The empty one is a length of zero and nothing after it.
        var e = Writer(.little, .xcdr2)
        e.putWstring([])
        XCTAssertEqual(hex(e.bytes()), "00000000")
    }

    func testWstringNonBmpIsASurrogatePairAndNotATruncation() throws {
        // byte-gate `wstring_nonbmp`: U+1D11E, which is one code point and two
        // UTF-16 code units. Cyclone writes the surrogate pair and a length of
        // four octets. Fast DDS writes `010000001ed1ab` — one unit 0xD11E, the
        // low sixteen bits of the code point: a silent truncation that produces
        // a frame of the right shape holding a different character.
        var w = Writer(.little, .xcdr2)
        w.putWstring([0xD834, 0xDD1E])
        w.putU8(0xAB)
        XCTAssertEqual(
            hex(w.bytes()), "0400000034d81eddab", "cyclone+rti; fastdds truncates to 0xD11E")
        var be = Writer(.big, .xcdr2)
        be.putWstring([0xD834, 0xDD1E])
        be.putU8(0xAB)
        XCTAssertEqual(hex(be.bytes()), "00000004d834dd1eab", "rti; cyclone leaves the units LE")
    }

    func testCharIsOneOctetAndRefusesWhatDoesNotFit() throws {
        // U+0161's low octet is 0x61, which is `a`: a truncating writer produces
        // a frame of the right length holding a plausible, wrong character.
        var ok = Writer(.little, .xcdr2)
        try ok.putChar(0xFF)
        var past = Writer(.little, .xcdr2)
        XCTAssertThrowsError(try past.putChar(0x0161))
    }

    // --- the 64-bit float ---------------------------------------------------

    func testPutF64WritesTheBitPattern() throws {
        // 1.5 is `ext_final`'s `b` and is checked above against three vendors;
        // the rest are the ends of the exponent range.
        for (value, want) in [
            (1.5, "000000000000f83f"),
            (0.0, "0000000000000000"),
            (3.141592653589793, "182d4454fb210940"),
            (Double.greatestFiniteMagnitude, "ffffffffffffef7f"),
            (Double.leastNonzeroMagnitude, "0100000000000000"),  // the smallest subnormal
        ] {
            var w = Writer(.little, .xcdr2)
            w.putF64(value)
            XCTAssertEqual(hex(w.bytes()), want, "putF64(\(value))")
        }
    }

    // --- the ends of every integer range ------------------------------------

    func testEveryWidthWritesItsEndsAndTheTypeRefusesThePastEnd() throws {
        // Swift's integer types carry the wire widths, so `putU16(70000)` is a
        // compile error rather than the `7011` nine of the fifteen bindings
        // wrote. What is checked here is that both ends of each width still
        // reach the wire unchanged.
        var w8 = Writer(.little, .xcdr2)
        w8.putU8(0)
        w8.putU8(UInt8.max)
        XCTAssertEqual(hex(w8.bytes()), "00ff")
        var w16 = Writer(.little, .xcdr2)
        w16.putU16(0)
        w16.putU16(UInt16.max)
        XCTAssertEqual(hex(w16.bytes()), "0000ffff")
        var w32 = Writer(.little, .xcdr2)
        w32.putU32(0)
        w32.putU32(UInt32.max)
        XCTAssertEqual(hex(w32.bytes()), "00000000ffffffff")
        var w64 = Writer(.little, .xcdr2)
        w64.putU64(0)
        w64.putU64(UInt64.max)
        XCTAssertEqual(hex(w64.bytes()), "0000000000000000ffffffffffffffff")
    }

    // --- the packed decimal --------------------------------------------------

    func testFixedBasicIsPackedBcdIndependentOfTheByteOrder() throws {
        // `tests/interop-corpus/fixed-basic` is `@final struct Sample { Money
        // amount; Money debt; octet tail; }` over `typedef fixed<9,3> Money`,
        // with "123456.789", "-000000.001" and 0xAB. P + 1 = 10 is even, so
        // there is no leading zero nibble and each value is five octets.
        //
        // `fixed<P,S>` is CORBA's and not DDS's, so the foreign octets come
        // from an ORB and not from the four DDS vendors: Cyclone, RTI, Fast DDS
        // and OpenDDS each refuse the type in their IDL front end, which is why
        // `fixed_basic` carries no `wrote` row in vendor_bytes.rows.tsv at all.
        //
        // omniORB wrote `123456789c000000001dab` in both byte orders
        // (tests/interop-results/fixed-basic/omniorb/plain-cdr1.{le,be}.bin),
        // octet for octet what this SDK writes — and the two orders being equal
        // is the measurement that no multi-octet integer is written here.
        //
        // JacORB wrote `123456789c1dab` for the same sample, big-endian only:
        // the first value agrees and the second is one octet rather than five,
        // so it packs the significant digits instead of the declared width. TAO
        // produced no bytes at all — `tao_idl` refused the IDL.
        var le = Writer(.little, .xcdr1)
        try le.putFixed("123456.789", digits: 9, scale: 3)
        try le.putFixed("-000000.001", digits: 9, scale: 3)
        le.putU8(0xAB)
        XCTAssertEqual(
            hex(le.bytes()), "123456789c000000001dab",
            "omniorb; jacorb packs the second value to one octet")
        var be = Writer(.big, .xcdr1)
        try be.putFixed("123456.789", digits: 9, scale: 3)
        try be.putFixed("-000000.001", digits: 9, scale: 3)
        be.putU8(0xAB)
        XCTAssertEqual(hex(be.bytes()), "123456789c000000001dab", "omniorb")
    }

    func testFixedAtTheEndsOfItsRange() throws {
        for (value, digits, scale, want) in [
            ("1", 1, 0, "1c"),
            ("-9", 1, 0, "9d"),
            ("12", 2, 0, "012c"),  // P + 1 odd -> a leading zero nibble
            ("0.123", 3, 3, "123c"),  // scale == digits: all fraction
            ("-0", 1, 0, "0c"),  // a negative zero is written positive
            ("-0.000", 3, 3, "000c"),
            ("7.5", 9, 3, "000007500c"),  // padding on both sides at once
            (
                "1234567890123456789012345678901", 31, 0,
                "1234567890123456789012345678901c"
            ),
            (
                "-1234567890123456789012345678901", 31, 0,
                "1234567890123456789012345678901d"
            ),
        ] {
            var w = Writer(.little, .xcdr2)
            try w.putFixed(value, digits: digits, scale: scale)
            XCTAssertEqual(hex(w.bytes()), want, "putFixed(\(value), \(digits), \(scale))")
            XCTAssertEqual(w.bytes().count, (digits + 2) / 2)
        }
        for (value, digits, scale) in [
            ("1234567", 6, 0),  // seven integer digits into six
            ("1.2345", 6, 3),  // four fraction digits into three
            ("1.0", 2, 3),  // a scale wider than the digit count
            ("twelve", 4, 0),  // not a decimal
            ("", 4, 0),  // no digits at all
        ] {
            var w = Writer(.little, .xcdr2)
            XCTAssertThrowsError(
                try w.putFixed(value, digits: digits, scale: scale),
                "putFixed(\(value), \(digits), \(scale)) was accepted")
        }
    }

    // --- the bitset anchor ---------------------------------------------------

    func testAnEightOctetAnchorTakesTheEncodingsAlignment() throws {
        var one = Writer(.little, .xcdr1)
        one.putU8(0xAB)
        try one.putBitset(1, anchor: 8)
        XCTAssertEqual(hex(one.bytes()), "ab00000000000000" + "0100000000000000")
        var two = Writer(.little, .xcdr2)
        two.putU8(0xAB)
        try two.putBitset(1, anchor: 8)
        XCTAssertEqual(hex(two.bytes()), "ab000000" + "0100000000000000")
        var bad = Writer(.little, .xcdr2)
        XCTAssertThrowsError(try bad.putBitset(0, anchor: 3))
    }

    // --- the reading direction ------------------------------------------------

    func testTheVendorsOwnOctetsReadBackIntoTheSample() throws {
        // `ext_appendable` under XCDR2 in both byte orders — the octets Cyclone,
        // RTI and Fast DDS all wrote, in through this SDK's reader.
        for (endian, wire) in [
            (Endianness.little, "0e00000007000000000000000000f83ffdff"),
            (Endianness.big, "0000000e000000073ff8000000000000fffd"),
        ] {
            let bytes = unhex(wire)
            var r = Reader(bytes, endian, .xcdr2)
            XCTAssertEqual(r.dheaderRead(), 14)
            XCTAssertEqual(r.getU32(), 7)
            XCTAssertEqual(r.getF64(), 1.5)
            XCTAssertEqual(r.getU16(), 0xFFFD)
            XCTAssertEqual(r.pos, bytes.count)
        }
    }

    func testTheMemberHeaderReadsBackFromTheVendorsOctets() throws {
        let bytes = unhex("1a000000000000200700000001000030000000000000f83f02000010fdff")
        var r = Reader(bytes, .little, .xcdr2)
        XCTAssertEqual(r.dheaderRead(), 26)
        for (id, lc, extent) in [
            (UInt32(0), LengthCode.lc2, UInt32(4)),
            (UInt32(1), LengthCode.lc3, UInt32(8)),
            (UInt32(2), LengthCode.lc1, UInt32(2)),
        ] {
            let m = r.emheaderRead()
            XCTAssertEqual(m.id, id)
            XCTAssertEqual(m.lengthCode, lc)
            XCTAssertEqual(m.extent, extent)
            r.pos += Int(m.extent)
        }
        XCTAssertEqual(r.pos, bytes.count)
    }

    func testTheParameterListReadsBackFromRtisOctets() throws {
        let bytes = unhex("000004000700000001000800000000000000f83f02000400fdff0000027f0000")
        var r = Reader(bytes, .little, .xcdr1)
        for (id, length) in [(UInt32(0), UInt32(4)), (UInt32(1), UInt32(8)), (UInt32(2), UInt32(4))]
        {
            let p = r.pidRead()
            XCTAssertFalse(p.listEnd)
            XCTAssertEqual(p.id, id)
            XCTAssertEqual(p.length, length)
            r.pidSkip(r.pos, p.length)
        }
        XCTAssertTrue(r.pidRead().listEnd)
        XCTAssertEqual(r.pos, bytes.count)
    }

    func testEveryWriteReadsBack() throws {
        for encoding in [Encoding.xcdr1, Encoding.xcdr2] {
            for endian in [Endianness.little, Endianness.big] {
                var w = Writer(endian, encoding)
                w.putU8(0xFF)
                w.putBool(true)
                try w.putChar(0xFF)
                w.putU16(0xFFFF)
                w.putU32(0xFFFF_FFFF)
                w.putU64(0xFFFF_FFFF_FFFF_FFFF)
                w.putF32(-3.5)
                w.putF64(3.141592653589793)
                try w.putWchar(0xFFFF)
                w.putString("grüße")
                w.putSeqU8([0xDE, 0xAD])
                w.putWstring([0xD83D, 0xDE00])
                try w.putBitset(0xBC61_4E00_000F_FFFF, anchor: 8)
                try w.putFixed("-000000.001", digits: 9, scale: 3)

                var r = Reader(w.bytes(), endian, encoding)
                XCTAssertEqual(r.getU8(), 0xFF)
                XCTAssertTrue(r.getBool())
                XCTAssertEqual(r.getChar(), 0xFF)
                XCTAssertEqual(r.getU16(), 0xFFFF)
                XCTAssertEqual(r.getU32(), 0xFFFF_FFFF)
                XCTAssertEqual(r.getU64(), 0xFFFF_FFFF_FFFF_FFFF)
                XCTAssertEqual(r.getF32(), -3.5)
                XCTAssertEqual(r.getF64(), 3.141592653589793)
                XCTAssertEqual(r.getWchar(), 0xFFFF)
                XCTAssertEqual(r.getString(), "grüße")
                XCTAssertEqual(r.getSeqU8(), [0xDE, 0xAD])
                XCTAssertEqual(r.getWstring(), [0xD83D, 0xDE00])
                XCTAssertEqual(try r.getBitset(anchor: 8), 0xBC61_4E00_000F_FFFF)
                XCTAssertEqual(r.getFixed(digits: 9, scale: 3), "-000000.001")
                XCTAssertEqual(r.pos, w.bytes().count)
            }
        }
    }
}
