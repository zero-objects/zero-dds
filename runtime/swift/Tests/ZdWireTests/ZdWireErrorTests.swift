// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// A malformed or truncated stream throws a `ZdWireError` and leaves the
// process running — it does not call `exit`. Before this file, a
// refusal anywhere in `ZdReader`/`ZdWriter` wrote a message to standard
// error and terminated the process, which made the runtime untestable
// from inside the same process a refusal happened in: the assertion
// that would have reported the failure never ran. Every test below
// performs at least one more operation after the refusal it checks, so
// a regression back to `exit` would show up as the whole suite dying
// rather than as one red assertion.

import XCTest

@testable import ZdWire

final class ZdWireErrorTests: XCTestCase {
    /// The clause's own framing — sentinel-must-understand,
    /// `MAX_TYPECODE...` is not a factor here, just PL_CDR1's defaults —
    /// which is what every test below reads and writes under.
    private func framing() -> ZdPlCdr1Framing {
        return ZdPlCdr1Framing("sentinel-must-understand", 0x3F00, false, false, true, true)
    }

    // -- the reader: a truncated stream -------------------------------------

    /// Two bytes where a `long` needs four: `readU32` throws `.truncated`
    /// rather than reading past the buffer or calling `exit`.
    func testTruncatedStreamThrowsRatherThanExiting() {
        let reader = ZdReader([0x01, 0x02], .little, 4, framing())
        XCTAssertThrowsError(try reader.readU32()) { error in
            guard case let .some(.truncated(at, needed, available)) = error as? ZdWireError else {
                XCTFail("expected .truncated, got \(error)")
                return
            }
            XCTAssertEqual(at, 0)
            XCTAssertEqual(needed, 4)
            XCTAssertEqual(available, 2)
        }
        // The process is still here: a second, well-formed read on a
        // fresh reader succeeds right after the refusal above.
        let good = ZdReader([0x2A, 0x00, 0x00, 0x00], .little, 4, framing())
        XCTAssertEqual(try good.readU32(), 42)
    }

    // -- the reader: an impossible length ------------------------------------

    /// A `string`'s length field claims 0x7FFFFFFF octets and the
    /// stream has none behind it: `readString` throws
    /// `.impossibleLength`, distinct from `.truncated` because the
    /// length itself cannot be true rather than merely running short.
    func testImpossibleLengthThrowsRatherThanExiting() {
        let reader = ZdReader([0xFF, 0xFF, 0xFF, 0x7F], .little, 4, framing())
        XCTAssertThrowsError(try reader.readString()) { error in
            guard case let .some(.impossibleLength(at, claimed, available)) =
                error as? ZdWireError
            else {
                XCTFail("expected .impossibleLength, got \(error)")
                return
            }
            XCTAssertEqual(at, 0)
            XCTAssertEqual(claimed, 0x7FFF_FFFF)
            XCTAssertEqual(available, 0)
        }
        // Still running: the same reader answers its position and
        // status normally rather than the process having gone away.
        XCTAssertEqual(reader.readerPosition(), 4)
    }

    // -- the reader: not UTF-8 and out of range ------------------------------

    /// A string whose declared bytes are not valid UTF-8.
    func testNotUtf8ThrowsRatherThanExiting() {
        let bytes: [UInt8] = [0x03, 0x00, 0x00, 0x00, 0xFF, 0xFE, 0x00]
        let reader = ZdReader(bytes, .little, 4, framing())
        XCTAssertThrowsError(try reader.readString()) { error in
            guard case let .some(.notUtf8(at)) = error as? ZdWireError else {
                XCTFail("expected .notUtf8, got \(error)")
                return
            }
            // The offset of the length field, as `crates/wire/src/reader.rs`
            // reports it: the string starts there.
            XCTAssertEqual(at, 0)
        }
    }

    /// A `boolean` octet that is neither 0 nor 1.
    func testOutOfRangeThrowsRatherThanExiting() {
        let reader = ZdReader([0x02], .little, 4, framing())
        XCTAssertThrowsError(try reader.readBool()) { error in
            guard case let .some(.outOfRange(at, expected)) = error as? ZdWireError else {
                XCTFail("expected .outOfRange, got \(error)")
                return
            }
            XCTAssertEqual(at, 0)
            XCTAssertEqual(expected, "a boolean of 0 or 1")
        }
    }

    // -- the writer refuses too ----------------------------------------------

    /// The refusal is the sample's on the way out as well as the way
    /// in: a value the application handed the writer that its own
    /// declared bound cannot carry throws instead of writing a
    /// truncated string nobody asked for.
    func testWriterOutOfRangeThrowsRatherThanExiting() {
        let writer = ZdWriter(.little, 4, framing(), false)
        XCTAssertThrowsError(try writer.writeStringBounded("hello", 3)) { error in
            guard case .some(.outOfRange) = error as? ZdWireError else {
                XCTFail("expected .outOfRange, got \(error)")
                return
            }
        }
        // The writer is still usable: a value inside the bound writes
        // normally right after the refused one above.
        XCTAssertNoThrow(try writer.writeStringBounded("hi", 3))
    }

    // -- repeated refusals never bring the process down ----------------------

    /// The point of this file: many refusals in a row, each caught, with
    /// ordinary work between them. If any of `ZdReader`/`ZdWriter` still
    /// called `exit` on one of these paths, this test would not finish
    /// — it would take the whole process with it, and no failure would
    /// be reported at all.
    func testTheProcessSurvivesEveryRefusalHere() {
        var caught = 0
        for _ in 0..<5 {
            let reader = ZdReader([0x00], .little, 4, framing())
            XCTAssertThrowsError(try reader.readU64())
            caught += 1
        }
        XCTAssertEqual(caught, 5)
        // One last, ordinary round trip: writer then reader, no refusal
        // anywhere in it, running in the same process as every refusal
        // above.
        let writer = ZdWriter(.little, 4, framing(), false)
        writer.writeU32(7)
        let bytes = writer.finish()
        let reader = ZdReader(bytes, .little, 4, framing())
        XCTAssertEqual(try reader.readU32(), 7)
    }
}

/// The three axes `Dialect::Hdds` selects. Expected octets from the
/// recorded HDDS columns:
/// `tests/interop-results/wstring-member/hdds/plain-cdr2.le.bin`
/// (`04 00 00 00 61 62 63 00` for `"abc"`),
/// `tests/interop-results/array-1d-struct-final/hdds/plain-cdr2.le.bin`
/// (`01 00 00 00 02 00 03 00 00 00 04 00 77`) and
/// `tests/interop-results/sequence-struct-appendable/hdds/plain-cdr2.le.bin`
/// (the first `Cell` of `one`, `06 00 00 00 79 29 ed ff fe ff`).
final class HddsAxesTests: XCTestCase {
    /// The clause's own framing, as `ZdWireErrorTests` uses it.
    private func framing() -> ZdPlCdr1Framing {
        return ZdPlCdr1Framing("sentinel-must-understand", 0x3F00, false, false, true, true)
    }

    func testAWstringIsItsOwnUtf8Octets() throws {
        let writer = ZdWriter(.little, 4, framing(), false)
        try writer.writeWstringUtf8Octets("abc", "octets/terminated/2")
        XCTAssertEqual(writer.finish(), [4, 0, 0, 0, 0x61, 0x62, 0x63, 0])
    }

    func testAMultibyteUtf8WstringCountsItsOctets() throws {
        let writer = ZdWriter(.little, 4, framing(), false)
        try writer.writeWstringUtf8Octets("äöü ሴ Z", "octets/terminated/2")
        XCTAssertEqual(
            writer.finish(),
            [0x0d, 0, 0, 0, 0xc3, 0xa4, 0xc3, 0xb6, 0xc3, 0xbc, 0x20, 0xe1, 0x88, 0xb4, 0x20, 0x5a, 0])
    }

    func testABoundedUtf8WstringBoundsItsUtf16Units() throws {
        let writer = ZdWriter(.little, 4, framing(), false)
        try writer.writeWstringUtf8OctetsBounded("ሴäZ", 3, "octets/terminated/2")
        XCTAssertEqual(writer.finish(), [7, 0, 0, 0, 0xe1, 0x88, 0xb4, 0xc3, 0xa4, 0x5a, 0])
        XCTAssertThrowsError(
            try ZdWriter(.little, 4, framing(), false)
                .writeWstringUtf8OctetsBounded("ሴäZ", 2, "octets/terminated/2"))
    }

    func testAUtf8WstringReadsBackAsItsOctets() throws {
        let recorded: [UInt8] = [
            0x0d, 0, 0, 0, 0xc3, 0xa4, 0xc3, 0xb6, 0xc3, 0xbc, 0x20, 0xe1, 0x88, 0xb4, 0x20, 0x5a, 0,
        ]
        let reader = ZdReader(recorded, .little, 4, framing())
        XCTAssertEqual(try reader.readWstringUtf8Octets("octets/terminated/2"), "äöü ሴ Z")
    }

    func testAPackedElementCountsAlignmentFromItsOwnStart() {
        let writer = ZdWriter(.little, 4, framing(), false)
        var saved = writer.beginPackedElement()
        writer.writeI32(1)
        writer.writeI16(2)
        writer.finishPackedElement(saved)
        saved = writer.beginPackedElement()
        writer.writeI32(3)
        writer.writeI16(4)
        writer.finishPackedElement(saved)
        writer.writeU8(119)
        XCTAssertEqual(writer.finish(), [1, 0, 0, 0, 2, 0, 3, 0, 0, 0, 4, 0, 119])
    }

    func testAPackedAggregateCountsAlignmentFromItsOwnStart() {
        let writer = ZdWriter(.little, 4, framing(), false)
        var saved = writer.beginPackedElement()
        writer.writeI32(-1234567)
        writer.writeI16(-2)
        writer.finishPackedElement(saved)
        saved = writer.beginPackedElement()
        writer.writeI32(1)
        writer.writeI16(32767)
        writer.finishPackedElement(saved)
        XCTAssertEqual(
            writer.finish(), [0x79, 0x29, 0xed, 0xff, 0xfe, 0xff, 1, 0, 0, 0, 0xff, 0x7f])
    }

    func testAPackedElementReadsBackAtItsOwnOrigin() throws {
        let recorded: [UInt8] = [1, 0, 0, 0, 2, 0, 3, 0, 0, 0, 4, 0, 119]
        let reader = ZdReader(recorded, .little, 4, framing())
        var saved = reader.readerBeginPackedElement()
        XCTAssertEqual(try reader.readI32(), 1)
        XCTAssertEqual(try reader.readI16(), 2)
        reader.readerFinishPackedElement(saved)
        saved = reader.readerBeginPackedElement()
        XCTAssertEqual(try reader.readI32(), 3)
        XCTAssertEqual(try reader.readI16(), 4)
        reader.readerFinishPackedElement(saved)
        XCTAssertEqual(try reader.readU8(), 119)
    }

    func testAnElementLengthFramesTheElementItPrecedes() throws {
        // The first `Cell` of `sequence-struct-appendable`'s `one`: the
        // element's own length in DHEADER form, then the six octets of
        // the appendable `Cell`.
        let writer = ZdWriter(.little, 4, framing(), false)
        let header = writer.writeDheader()
        writer.writeI32(-1234567)
        writer.writeI16(-2)
        try writer.finishDheader(header)
        let got = writer.finish()
        XCTAssertEqual(got, [6, 0, 0, 0, 0x79, 0x29, 0xed, 0xff, 0xfe, 0xff])
        let reader = ZdReader(got, .little, 4, framing())
        let declared = Int(try reader.readU32())
        let end = reader.readerPosition() + declared
        XCTAssertEqual(try reader.readI32(), -1234567)
        XCTAssertEqual(try reader.readI16(), -2)
        XCTAssertEqual(reader.readerPosition(), end)
    }
}
