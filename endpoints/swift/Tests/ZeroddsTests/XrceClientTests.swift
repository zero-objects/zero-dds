// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// Tests for the DDS-XRCE 1.0 client in `Sources/Zerodds/XrceClient.swift`:
//
//   1. Byte-identity of the five Layer 1 message encoders against the Rust-
//      generated fixtures in `../../testdata/xrce-client/*.bin`.
//   2. The `XrceClient` state machine's lifecycle rules, translated from
//      `crates/xrce-client/src/lib.rs`'s own `#[cfg(test)] mod tests`.

import Foundation
import XCTest

@testable import Zerodds

final class XrceClientTests: XCTestCase {
    // `swift test` runs with the package root (`endpoints/swift`) as the
    // working directory, so the fixtures one level above `endpoints/swift`
    // are `../testdata/xrce-client`.
    func fixture(_ name: String) throws -> [UInt8] {
        let url = URL(fileURLWithPath: "../testdata/xrce-client/\(name).bin")
        return [UInt8](try Data(contentsOf: url))
    }

    let key1234: ClientKeyBytes = [0x01, 0x02, 0x03, 0x04]

    // session_id = 0x01 throughout (<=0x7F, so every header except CREATE_CLIENT's carries the
    // ClientKey). CREATE_CLIENT travels on streamId=NONE, seq 0. The four
    // post-handshake operations are all on streamId=0x80 (builtin-
    // reliable) with a real, strictly increasing sequence number (0..3) —
    // not the best-effort stream with hand-picked numbers a previous
    // fixture set used.
    func testFixturesAreByteIdentical() throws {
        // 1. CREATE_CLIENT: a real CLIENT_Representation (version={1,0},
        //    vendor_id={0,0}, this client_key, session_id=0x01, no
        //    properties, requested_mtu=508).
        let repr = encodeClientRepresentation(
            version: (0x01, 0x00), vendorId: (0x00, 0x00), clientKey: key1234, sessionId: 0x01)
        let gotCreateClient = encodeCreateClientMessage(sessionId: 0x01, clientKey: key1234, representation: repr)
        try XCTAssertEqual(gotCreateClient, fixture("xrce_client_create_client"),
                            "xrce_client_create_client.bin: not byte-identical")
        // DDS-XRCE 1.0 §8.3.2.1: requested session id >= 128 -> header 0x80, no client key.
        let reprHi = encodeClientRepresentation(
            version: (0x01, 0x00), vendorId: (0x00, 0x00), clientKey: key1234, sessionId: 0x81)
        let gotHi = encodeCreateClientMessage(sessionId: 0x81, clientKey: key1234, representation: reprHi)
        XCTAssertEqual(gotHi[0], 0x80)
        XCTAssertEqual(gotHi.count, gotCreateClient.count - 4)

        // Representation timestamps (7.8.2.1 prose vs Annex A / B.1).
        let wantAbsent: [UInt8] = [0x58, 0x52, 0x43, 0x45, 1, 0, 0, 0, 1, 2, 3, 4, 1, 0, 0xFC, 1]
        let wantApp: [UInt8] = [0x58, 0x52, 0x43, 0x45, 1, 0, 0, 0, 8, 0, 0, 0,
                                0x8C, 0xAA, 0x88, 0x5A, 0x00, 0x65, 0xCD, 0x1D, 1, 2, 3, 4, 1, 0, 0xFC, 1]
        let wantFinal: [UInt8] = [0x58, 0x52, 0x43, 0x45, 1, 0, 0, 0,
                                  0x8C, 0xAA, 0x88, 0x5A, 0x00, 0x65, 0xCD, 0x1D, 1, 2, 3, 4, 1, 0, 0xFC, 1]
        let stamp: (seconds: Int32, nanoseconds: UInt32) = (1518905996, 500000000)
        XCTAssertEqual(repr, wantAbsent, "default representation must stay absent")
        XCTAssertEqual(encodeClientRepresentation(
            version: (1, 0), vendorId: (0, 0), clientKey: key1234, sessionId: 0x01,
            representationTimestamps: .absent), wantAbsent)
        XCTAssertEqual(encodeClientRepresentation(
            version: (1, 0), vendorId: (0, 0), clientKey: key1234, sessionId: 0x01,
            representationTimestamps: .present, timestamp: stamp), wantApp)
        XCTAssertEqual(encodeClientRepresentation(
            version: (1, 0), vendorId: (0, 0), clientKey: key1234, sessionId: 0x01,
            representationTimestamps: .present, timestamp: stamp, unannotatedTypes: .final), wantFinal)
        let present = XrceClient(clientKey: key1234, transport: MemTransport(), representationTimestamps: .present)
        XCTAssertEqual(present.clientRepresentation(version: (1, 0), vendorId: (0, 0)).count, 28)
        XCTAssertEqual(XrceClient(clientKey: key1234, transport: MemTransport())
            .clientRepresentation(version: (1, 0), vendorId: (0, 0)).count, 16)

        // 2. CREATE: request_id=[0x00,0x01], object_id=pack(raw=0x001,kind=datawriter),
        //    the real two-level ObjectVariant::ByReference("SensorWriter") under
        //    parent=pack(raw=0x001,kind=publisher), reuse=false, replace=false.
        let createOid = ObjectId(rawId: 0x001, kind: .datawriter)
        let publisherOid = ObjectId(rawId: 0x001, kind: .publisher)
        let gotCreate = encodeCreateMessage(
            sessionId: 0x01, streamId: 0x80, sequenceNr: 0, clientKey: key1234,
            requestId: (0x00, 0x01), objectId: createOid, name: "SensorWriter",
            extra: ObjectVariantExtra(parentObjectId: publisherOid))
        try XCTAssertEqual(gotCreate, fixture("xrce_client_create"),
                            "xrce_client_create.bin: not byte-identical")

        // 3. DELETE: request_id=[0x00,0x02], object_id=pack(raw=0x001,kind=datawriter)
        //    — deletes the writer CREATEd above.
        let deleteOid = ObjectId(rawId: 0x001, kind: .datawriter)
        let gotDelete = encodeDeleteMessage(
            sessionId: 0x01, streamId: 0x80, sequenceNr: 1, clientKey: key1234,
            requestId: (0x00, 0x02), objectId: deleteOid)
        try XCTAssertEqual(gotDelete, fixture("xrce_client_delete"),
                            "xrce_client_delete.bin: not byte-identical")

        // 4. READ_DATA: request_id=[0x00,0x03], object_id=pack(raw=0x002,kind=datareader),
        //    preferred_stream_id=0x01, data_format=Data, no filter, no delivery control.
        let readOid = ObjectId(rawId: 0x002, kind: .datareader)
        let gotRead = encodeReadDataMessage(
            sessionId: 0x01, streamId: 0x80, sequenceNr: 2, clientKey: key1234,
            requestId: (0x00, 0x03), objectId: readOid, preferredStreamId: 0x01)
        try XCTAssertEqual(gotRead, fixture("xrce_client_read_data"),
                            "xrce_client_read_data.bin: not byte-identical")

        // 5. WRITE_DATA: request_id=[0x00,0x04], object_id=pack(raw=0x001,kind=datawriter)
        //    — the same writer — serialized_data=[0x19,0x00].
        let writeOid = ObjectId(rawId: 0x001, kind: .datawriter)
        let gotWrite = encodeWriteDataMessage(
            sessionId: 0x01, streamId: 0x80, sequenceNr: 3, clientKey: key1234,
            requestId: (0x00, 0x04), objectId: writeOid, serializedData: [0x19, 0x00])
        try XCTAssertEqual(gotWrite, fixture("xrce_client_write_data"),
                            "xrce_client_write_data.bin: not byte-identical")

        // 6. A second CREATE, appended to the same session:
        //    request_id=[0x00,0x05], object_id=pack(raw=0x010,kind=topic),
        //    the real two-level ObjectVariant::ByReference("HelloWorldTopic")
        //    under parent=pack(raw=0x001,kind=participant), sequenceNr=4.
        //    OBJK_TOPIC's own representation carries its participant's
        //    ObjectId (commit 31c62df2a63, "xrce:
        //    OBJK_TOPIC_Representation carries its participant's
        //    ObjectId"), the same trailing field OBJK_PUBLISHER's
        //    representation carries above — this fixture is generated the
        //    same way (endpoints/golden-gen), not a hand-picked byte
        //    string.
        let topicOid = ObjectId(rawId: 0x010, kind: .topic)
        let participantOid = ObjectId(rawId: 0x001, kind: .participant)
        let gotCreateTopic = encodeCreateMessage(
            sessionId: 0x01, streamId: 0x80, sequenceNr: 4, clientKey: key1234,
            requestId: (0x00, 0x05), objectId: topicOid, name: "HelloWorldTopic",
            extra: ObjectVariantExtra(parentObjectId: participantOid))
        try XCTAssertEqual(gotCreateTopic, fixture("xrce_client_create_topic"),
                            "xrce_client_create_topic.bin: not byte-identical")
    }

    let testKey: ClientKeyBytes = [0xAB, 0xAB, 0xAB, 0xAB]

    func testNewClientStartsDisconnected() {
        let c = XrceClient(clientKey: testKey, transport: MemTransport())
        XCTAssertEqual(c.state, .disconnected)
        XCTAssertEqual(c.clientKey, testKey)
    }

    func testConnectTransitionsToConnecting() throws {
        let c = XrceClient(clientKey: testKey, transport: MemTransport())
        try c.connect(representation: [0x00])
        XCTAssertEqual(c.state, .connecting)
    }

    func testMarkConnectedTransitionsToConnected() throws {
        let c = XrceClient(clientKey: testKey, transport: MemTransport())
        try c.connect(representation: [0x00])
        try c.markConnected()
        XCTAssertEqual(c.state, .connected)
    }

    func testDoubleConnectRejected() throws {
        let c = XrceClient(clientKey: testKey, transport: MemTransport())
        try c.connect(representation: [0x00])
        XCTAssertThrowsError(try c.connect(representation: [0x00])) { error in
            XCTAssertEqual(error as? XrceClientError, .invalidState)
        }
    }

    func testCreateWithoutConnectRejected() {
        let c = XrceClient(clientKey: testKey, transport: MemTransport())
        let oid = ObjectId(fromRawWord: 0x0010)
        XCTAssertThrowsError(try c.createObject(objectId: oid, name: "topic-ref")) { error in
            XCTAssertEqual(error as? XrceClientError, .invalidState)
        }
    }

    func testWriteWithoutConnectRejected() {
        let c = XrceClient(clientKey: testKey, transport: MemTransport())
        let oid = ObjectId(fromRawWord: 0x0010)
        XCTAssertThrowsError(try c.requestWrite(writerObjectId: oid, payload: [UInt8(ascii: "x")])) { error in
            XCTAssertEqual(error as? XrceClientError, .invalidState)
        }
    }

    func testReadWithoutConnectRejected() {
        let c = XrceClient(clientKey: testKey, transport: MemTransport())
        let oid = ObjectId(fromRawWord: 0x0010)
        XCTAssertThrowsError(try c.requestRead(readerObjectId: oid)) { error in
            XCTAssertEqual(error as? XrceClientError, .invalidState)
        }
    }

    func testFullLifecycleCreatesUniqueRequestIds() throws {
        let c = XrceClient(clientKey: testKey, transport: MemTransport())
        try c.connect(representation: [0x00])
        try c.markConnected()
        let oid = ObjectId(fromRawWord: 0x0010)
        let r1 = try c.createObject(objectId: oid, name: "a")
        let r2 = try c.deleteObject(objectId: oid)
        let r3 = try c.requestWrite(writerObjectId: oid, payload: [UInt8(ascii: "x")])
        let r4 = try c.requestRead(readerObjectId: oid)
        // Request ids are strictly monotonic.
        XCTAssertEqual(r1, 1)
        XCTAssertEqual(r2, 2)
        XCTAssertEqual(r3, 3)
        XCTAssertEqual(r4, 4)
    }

    func testDisconnectClearsState() throws {
        let c = XrceClient(clientKey: testKey, transport: MemTransport())
        try c.connect(representation: [0x00])
        try c.markConnected()
        c.disconnect()
        XCTAssertEqual(c.state, .disconnected)
    }
}
