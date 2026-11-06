// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// The five operations the generated code reaches only in a dialect or with a
// member no corpus case carries: `writePidExtended`, `writeUnionOverlay`,
// `readerClearDiscard`, `readerSave` and `readerRestore`
// (internal/release/OPEN-ITEMS.md). Every runtime checks the same four
// vectors, each against an oracle outside this code.

import Foundation
import XCTest

@testable import ZdWire

final class ZdWireOperationsTests: XCTestCase {
    private func framing() -> ZdPlCdr1Framing {
        return ZdPlCdr1Framing("sentinel-must-understand", 0x3F00, false, false, true, true)
    }

    private func hex(_ bytes: [UInt8]) -> String {
        return bytes.map { String(format: "%02x", $0) }.joined()
    }

    /// The first parameter of `annotation-autoid-sequential` as Cyclone DDS
    /// recorded it, XCDR1 little endian (tests/interop-corpus/expected-bytes.tsv).
    func testWritePidExtendedWritesCyclonesRecordedParameter() throws {
        let writer = ZdWriter(.little, 8, framing(), false)
        let header = try writer.writePidExtended(0, .member, false)
        writer.writeI32(-1234567)
        try writer.finishPid(header)
        XCTAssertEqual(hex(writer.finish()), "017f0800" + "00000000" + "04000000" + "7929edff")
    }

    /// A `short` read through a four-octet and a one-octet `default:` branch:
    /// Cyclone DDS 11.0.1 writes the union's storage, the member's octets with
    /// zeros above (`combined-everything`, crates/wire-codec/src/encode.rs
    /// `write_selected_branch`).
    func testWriteUnionOverlayWritesTheStorageAtTheBranchsWidth() throws {
        let writer = ZdWriter(.little, 4, framing(), false)
        let pattern = zdBitPatternInt(-2, 2)
        try writer.writeUnionOverlay(pattern, 4)
        try writer.writeUnionOverlay(pattern, 1)
        XCTAssertEqual(hex(writer.finish()), "feff0000" + "fe")
    }

    /// XTypes 1.3 §7.2.2.4.4.4.9: a member's explicit @default comes before
    /// Table 9's value, so the call site asks for DISCARD and takes it back.
    /// Any other refusal is not a narrowing and stays.
    func testReaderClearDiscardTakesBackADiscardAndNothingElse() {
        let discarded = ZdReader([1, 2, 3, 4], .little, 4, framing())
        discarded.readerRefuse(.discarded)
        XCTAssertTrue(discarded.readerClearDiscard())
        XCTAssertEqual(discarded.readerStatus(), .ok)
        let refused = ZdReader([1, 2, 3, 4], .little, 4, framing())
        refused.readerRefuse(.outOfRange)
        XCTAssertFalse(refused.readerClearDiscard())
        XCTAssertEqual(refused.readerStatus(), .outOfRange)
    }

    /// XTypes 1.3 §7.4.3.5.3 rule (15): a delimited map<long, long> as its
    /// DHEADER and the pairs, without a count. The reader counts the pairs,
    /// goes back and reads them again; a refusal made while counting goes back
    /// with the cursor.
    func testReaderSaveAndRestoreCountADelimitedMapAndReadItAgain() throws {
        let reader = ZdReader(
            [0x10, 0, 0, 0, 1, 0, 0, 0, 0x0a, 0, 0, 0, 2, 0, 0, 0, 0x14, 0, 0, 0],
            .little, 4, framing())
        let extent = try reader.readDheader()
        let end = reader.readerPosition() + Int(extent)
        let mark = reader.readerSave()
        var pairs = 0
        while reader.readerPosition() < end && reader.readerStatus() == .ok {
            _ = try reader.readI32()
            _ = try reader.readI32()
            pairs += 1
        }
        reader.readerRefuse(.outOfRange)
        reader.readerRestore(mark)
        XCTAssertEqual(pairs, 2)
        XCTAssertEqual(try reader.readI32(), 1)
        XCTAssertEqual(try reader.readI32(), 10)
        XCTAssertEqual(try reader.readI32(), 2)
        XCTAssertEqual(try reader.readI32(), 20)
        XCTAssertEqual(reader.readerPosition(), 20)
        XCTAssertEqual(reader.readerStatus(), .ok)
    }
}
