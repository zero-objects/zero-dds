// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

import Foundation
import XCTest
@testable import Zerodds

final class ZeroddsTests: XCTestCase {
    func fixture(_ e: Endianness) -> [UInt8] {
        var w = Writer(e, .xcdr2)
        w.putU32(0xA1B2_C3D4)
        w.putU16(0x1234)
        w.putU8(0x5A)
        w.putF32(3.5)
        w.putU64(0x0102_0304_0506_0708)
        w.putString("bay-12")
        w.putSeqU8([0xDE, 0xAD, 0xBE, 0xEF])
        return w.bytes()
    }

    func sampleBody(_ id: UInt32) -> [UInt8] {
        var w = Writer(.little, .xcdr2)
        w.putU32(id)
        w.putU16(0)
        w.putU8(0)
        return w.bytes()
    }

    // testdata/golden_{le,be}.bin are the canonical output of the Rust
    // zerodds-endpoint-golden crate (the 15 other endpoints regenerate and
    // verify them fresh; the Swift CI image has no Rust, so they are committed).
    func goldenDir() -> String {
        ProcessInfo.processInfo.environment["GOLDEN_DIR"] ?? "testdata"
    }

    func testByteIdentity() throws {
        for (e, file) in [(Endianness.little, "golden_le.bin"), (.big, "golden_be.bin")] {
            let url = URL(fileURLWithPath: goldenDir()).appendingPathComponent(file)
            let golden = [UInt8](try Data(contentsOf: url))
            XCTAssertEqual(fixture(e), golden, "\(file) not byte-identical")
        }
    }

    func testSyncLoopback() {
        let t = MemTransport()
        let c = Client(t, objectId: xrceObjectId(0x001, objkDataWriter))
        for i in 0..<5 { c.write(sampleBody(0x3000 + UInt32(i))) }
        for i in 0..<5 {
            guard let body = c.poll() else { XCTFail("sync: no sample \(i)"); return }
            var r = Reader(body, .little, .xcdr2)
            XCTAssertEqual(r.getU32(), 0x3000 + UInt32(i))
        }
    }

    func testAsyncLoopback() async {
        let t = MemTransport()
        let w = Client(t, objectId: xrceObjectId(0x001, objkDataWriter))
        for i in 0..<5 { w.write(sampleBody(0x1000 + UInt32(i))) }
        let reader = AsyncReader(t)
        var got: UInt32 = 0
        for await body in reader.stream() {
            var r = Reader(body, .little, .xcdr2)
            XCTAssertEqual(r.getU32(), 0x1000 + got)
            got += 1
            if got == 5 { break }
        }
        XCTAssertEqual(got, 5)
    }

    /// The DataWriter these tests address (7.2.1: raw 0x001 + OBJK_DATAWRITER).
    private let writerId = xrceObjectId(0x001, objkDataWriter)

    /// A frame for a sample that is inside the field's range, unwrapped once
    /// so the read-side vectors below read as they did.
    private func frame(_ seq: UInt16, _ sample: [UInt8]) throws -> [UInt8] {
        try XCTUnwrap(
            xrceWriteFrame(session: xrceSessionNoKey, stream: xrceStreamBestEffort,
                           seq: seq, objectId: writerId, sample: sample))
    }

    func testReadFrameBoundsBodyIgnoringAppendedSubmessage() throws {
        let first = try frame(1, [0xAA, 0xBB, 0xCC])
        let second = try frame(2, [0xDD, 0xEE])
        let body = xrceReadFrame(first + second)
        XCTAssertEqual(body, [0xAA, 0xBB, 0xCC])
    }

    func testReadFrameRejectsOverlongLength() throws {
        var frame = try frame(1, [0xAA, 0xBB, 0xCC])
        frame[6] = 0xFF
        frame[7] = 0xFF
        XCTAssertNil(xrceReadFrame(frame))
    }

    func testReadFrameRejectsTruncatedHeader() {
        XCTAssertNil(xrceReadFrame([0x80, 0x01, 0x00, 0x00, 0x07, 0x01, 0x04, 0x00]))
    }

    /// A sample the 16-bit submessage_length cannot describe has no frame.
    ///
    /// This used to answer `[]` and `Client.write` handed that to `deliver`:
    /// an empty datagram went out and nothing said so. `nil` is the refusal
    /// the comment always claimed the caller was seeing. `Client.write`'s half
    /// is a `preconditionFailure`, which traps the process and therefore
    /// cannot be asserted from inside XCTest; the builder's half is here.
    func testWriteFrameRefusesASampleAboveTheLengthField() {
        XCTAssertNil(
            xrceWriteFrame(session: xrceSessionNoKey, stream: xrceStreamBestEffort,
                           seq: 1, objectId: writerId,
                           sample: [UInt8](repeating: 0, count: 0x10000)))
        // The bound is the field's and not one off it: a maximal body frames.
        let largest = xrceWriteFrame(
            session: xrceSessionNoKey, stream: xrceStreamBestEffort,
            seq: 1, objectId: writerId,
            sample: [UInt8](repeating: 0, count: 0xFFFF - xrceBaseSize))
        XCTAssertEqual(largest?.count, 8 + 0xFFFF)
    }
}
