// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// Tests for the DDS-XRCE 1.0 client in `xrce_client.d`:
//
//   1. Byte-identity of the five Layer 1 message encoders against the Rust-
//      generated fixtures in `../testdata/xrce-client/*.bin`.
//   2. The `XrceClient` state machine's lifecycle rules, translated from
//      `crates/xrce-client/src/lib.rs`'s own `#[cfg(test)] mod tests`.
//
// Called from `test.d`'s `main()` (`xrceClientTests()`) — `make test` is the
// one command CI runs, and a module nothing calls goes unrun.

module xrce_client_test;

import zerodds : memTransport;
import xrce_client;
import std.stdio : writeln;
import std.file : read;

private ubyte[] fixture(string name) {
    return cast(ubyte[]) read("../testdata/xrce-client/" ~ name ~ ".bin");
}

private immutable ClientKeyBytes key1234 = [0x01, 0x02, 0x03, 0x04];

private void fixtureTests() {
    // session_id = 0x01 throughout (<=0x7F, so every header carries the
    // ClientKey). CREATE_CLIENT travels on stream=NONE, seq 0, with header
    // session id 0x00 (SESSION_ID_NONE_WITH_CLIENT_KEY, DDS-XRCE 1.0
    // Sec 8.3.2.1; the payload carries session_id 0x01). The four
    // post-handshake operations are all reliable, so they share
    // stream=0x80 (builtin-reliable) and a real, strictly increasing
    // sequence number (0..3) — not the best-effort stream with
    // hand-picked numbers a previous fixture set used.

    // 1. CREATE_CLIENT: a real CLIENT_Representation (version={1,0},
    //    vendor_id={0,0}, this client_key, session_id=0x01, no properties,
    //    requested_mtu=508).
    {
        ubyte[2] version_ = [0x01, 0x00];
        ubyte[2] vendorId = [0x00, 0x00];
        auto repr = encodeClientRepresentation(version_, vendorId, key1234, 0x01);
        auto got = encodeCreateClientMessage(0x01, key1234, repr);
        auto want = fixture("xrce_client_create_client");
        assert(got == want, "xrce_client_create_client.bin: not byte-identical");
        assert(got[0] == 0x00, "CREATE_CLIENT header session id is 0x00 for session < 128");

        // Requested session >= 128: header session 0x80, no client key.
        auto repr2 = encodeClientRepresentation(version_, vendorId, key1234, 0x81);
        auto got2 = encodeCreateClientMessage(0x81, key1234, repr2);
        assert(got2[0] == 0x80 && got2.length == 24, "CREATE_CLIENT session >= 128: header 0x80, no client key");
    }

    // 1b. Setting RepresentationTimestamps (DDS-XRCE 1.0 7.8.2.1 prose vs
    //     Annex A / B.1): absent (default), present+appendable, present+final.
    {
        ubyte[2] version_ = [0x01, 0x00];
        ubyte[2] vendorId = [0x00, 0x00];
        immutable ubyte[] absent = [0x58, 0x52, 0x43, 0x45, 0x01, 0x00, 0x00, 0x00,
                                    0x01, 0x02, 0x03, 0x04, 0x01, 0x00, 0xfc, 0x01];
        immutable ubyte[] presentAppendable = [0x58, 0x52, 0x43, 0x45, 0x01, 0x00, 0x00, 0x00,
                                    0x08, 0x00, 0x00, 0x00,
                                    0x8c, 0xaa, 0x88, 0x5a, 0x00, 0x65, 0xcd, 0x1d,
                                    0x01, 0x02, 0x03, 0x04, 0x01, 0x00, 0xfc, 0x01];
        immutable ubyte[] presentFinal = [0x58, 0x52, 0x43, 0x45, 0x01, 0x00, 0x00, 0x00,
                                    0x8c, 0xaa, 0x88, 0x5a, 0x00, 0x65, 0xcd, 0x1d,
                                    0x01, 0x02, 0x03, 0x04, 0x01, 0x00, 0xfc, 0x01];
        assert(encodeClientRepresentation(version_, vendorId, key1234, 0x01) == absent,
               "default is absent: bytes unchanged");
        assert(encodeClientRepresentation(version_, vendorId, key1234, 0x01, DefaultMtu,
                   RepresentationTimestamps.absent, XrceUnannotatedTypes.appendable,
                   1518905996, 500000000) == absent, "absent ignores the timestamp");
        auto a = encodeClientRepresentation(version_, vendorId, key1234, 0x01, DefaultMtu,
                   RepresentationTimestamps.present, XrceUnannotatedTypes.appendable,
                   1518905996, 500000000);
        assert(a == presentAppendable && a.length == 28, "present, appendable: 28 octets");
        auto f = encodeClientRepresentation(version_, vendorId, key1234, 0x01, DefaultMtu,
                   RepresentationTimestamps.present, XrceUnannotatedTypes.final_,
                   1518905996, 500000000);
        assert(f == presentFinal && f.length == 24, "present, final: 24 octets");
        auto msg = encodeCreateClientMessage(0x01, key1234, a);
        assert(msg.length == 40 && msg[10] == 28 && msg[11] == 0, "submessage length field grows to 28");
    }

    auto writerOid = ObjectId.pack(0x001, ObjectKind.DATAWRITER);

    // 2. CREATE: request_id=[0x00,0x01], object_id=pack(raw=0x001,kind=DATAWRITER),
    //    the real two-level ObjectVariant::ByReference("SensorWriter") under
    //    parent=pack(raw=0x001,kind=PUBLISHER), reuse=false, replace=false.
    {
        auto publisherOid = ObjectId.pack(0x001, ObjectKind.PUBLISHER);
        auto got = encodeCreateMessage(0x01, 0x80, 0, key1234,
                                        [cast(ubyte) 0x00, cast(ubyte) 0x01], writerOid,
                                        "SensorWriter", &publisherOid);
        auto want = fixture("xrce_client_create");
        assert(got == want, "xrce_client_create.bin: not byte-identical");
    }

    // 3. DELETE: request_id=[0x00,0x02], object_id=pack(raw=0x001,kind=DATAWRITER)
    //    — deletes the writer CREATEd above.
    {
        auto got = encodeDeleteMessage(0x01, 0x80, 1, key1234,
                                        [cast(ubyte) 0x00, cast(ubyte) 0x02], writerOid);
        auto want = fixture("xrce_client_delete");
        assert(got == want, "xrce_client_delete.bin: not byte-identical");
    }

    // 4. READ_DATA: request_id=[0x00,0x03], object_id=pack(raw=0x002,kind=DATAREADER),
    //    preferred_stream_id=0x01, data_format=Data, no filter, no delivery control.
    {
        auto oid = ObjectId.pack(0x002, ObjectKind.DATAREADER);
        auto got = encodeReadDataMessage(0x01, 0x80, 2, key1234,
                                          [cast(ubyte) 0x00, cast(ubyte) 0x03], oid, 0x01);
        auto want = fixture("xrce_client_read_data");
        assert(got == want, "xrce_client_read_data.bin: not byte-identical");
    }

    // 5. WRITE_DATA: request_id=[0x00,0x04], object_id=pack(raw=0x001,kind=DATAWRITER)
    //    — the same writer — serialized_data=[0x19,0x00].
    {
        auto got = encodeWriteDataMessage(0x01, 0x80, 3, key1234,
                                           [cast(ubyte) 0x00, cast(ubyte) 0x04], writerOid,
                                           [cast(ubyte) 0x19, cast(ubyte) 0x00]);
        auto want = fixture("xrce_client_write_data");
        assert(got == want, "xrce_client_write_data.bin: not byte-identical");
    }

    // 6. A second CREATE, appended to the same session:
    //    request_id=[0x00,0x05], object_id=pack(raw=0x010,kind=TOPIC), the
    //    real two-level ObjectVariant::ByReference("HelloWorldTopic")
    //    under parent=pack(raw=0x001,kind=PARTICIPANT), sequence_nr=4.
    //    OBJK_TOPIC's own representation carries its participant's
    //    ObjectId (commit 31c62df2a63, "xrce:
    //    OBJK_TOPIC_Representation carries its participant's ObjectId"),
    //    the same trailing field OBJK_PUBLISHER's representation carries
    //    above — this fixture is generated the same way (endpoints/golden-
    //    gen), not a hand-picked byte string.
    {
        auto topicOid = ObjectId.pack(0x010, ObjectKind.TOPIC);
        auto participantOid = ObjectId.pack(0x001, ObjectKind.PARTICIPANT);
        auto got = encodeCreateMessage(0x01, 0x80, 4, key1234,
                                        [cast(ubyte) 0x00, cast(ubyte) 0x05], topicOid,
                                        "HelloWorldTopic", &participantOid);
        auto want = fixture("xrce_client_create_topic");
        assert(got == want, "xrce_client_create_topic.bin: not byte-identical");
    }

    writeln("xrce-client fixtures: 6/6 byte-identical");
}

/// Mirrors `expect_err("...")` + `assert_eq!(err, ClientError::InvalidState)`
/// in the Rust reference: the refusal is measured by requiring the exception
/// to actually happen, with the right kind, not by inspecting state after.
private bool refusedInvalidState(void delegate() body_) {
    try {
        body_();
    } catch (XrceClientError e) {
        return e.kind == ClientErrorKind.INVALID_STATE;
    }
    return false;
}

private void stateMachineTests() {
    immutable ClientKeyBytes testKey = [0xAB, 0xAB, 0xAB, 0xAB];

    { // new_client_starts_disconnected
        auto c = new XrceClient(testKey, memTransport());
        assert(c.state() == ClientState.DISCONNECTED);
        assert(c.clientKey() == testKey);
    }

    { // connect_transitions_to_connecting
        auto c = new XrceClient(testKey, memTransport());
        c.connect([cast(ubyte) 0x00]);
        assert(c.state() == ClientState.CONNECTING);
    }

    { // mark_connected_transitions_to_connected
        auto c = new XrceClient(testKey, memTransport());
        c.connect([cast(ubyte) 0x00]);
        c.markConnected();
        assert(c.state() == ClientState.CONNECTED);
    }

    { // double_connect_rejected
        auto c = new XrceClient(testKey, memTransport());
        c.connect([cast(ubyte) 0x00]);
        assert(refusedInvalidState({ c.connect([cast(ubyte) 0x00]); }),
               "double_connect_rejected: expected an InvalidState refusal");
    }

    { // create_without_connect_rejected
        auto c = new XrceClient(testKey, memTransport());
        auto oid = ObjectId.fromRawWord(0x0010);
        assert(refusedInvalidState({ c.createObject(oid, "topic-ref"); }),
               "create_without_connect_rejected: expected an InvalidState refusal");
    }

    { // write_without_connect_rejected
        auto c = new XrceClient(testKey, memTransport());
        auto oid = ObjectId.fromRawWord(0x0010);
        assert(refusedInvalidState({ c.requestWrite(oid, [cast(ubyte) 'x']); }),
               "write_without_connect_rejected: expected an InvalidState refusal");
    }

    { // read_without_connect_rejected
        auto c = new XrceClient(testKey, memTransport());
        auto oid = ObjectId.fromRawWord(0x0010);
        assert(refusedInvalidState({ c.requestRead(oid); }),
               "read_without_connect_rejected: expected an InvalidState refusal");
    }

    { // full_lifecycle_creates_unique_request_ids
        auto c = new XrceClient(testKey, memTransport());
        c.connect([cast(ubyte) 0x00]);
        c.markConnected();
        auto oid = ObjectId.fromRawWord(0x0010);
        auto r1 = c.createObject(oid, "a");
        auto r2 = c.deleteObject(oid);
        auto r3 = c.requestWrite(oid, [cast(ubyte) 'x']);
        auto r4 = c.requestRead(oid);
        // Request ids are strictly monotonic.
        assert(r1 == 1);
        assert(r2 == 2);
        assert(r3 == 3);
        assert(r4 == 4);
    }

    { // disconnect_clears_state
        auto c = new XrceClient(testKey, memTransport());
        c.connect([cast(ubyte) 0x00]);
        c.markConnected();
        c.disconnect();
        assert(c.state() == ClientState.DISCONNECTED);
    }

    writeln("xrce-client state machine: 9/9 OK");
}

void xrceClientTests() {
    fixtureTests();
    stateMachineTests();
}
