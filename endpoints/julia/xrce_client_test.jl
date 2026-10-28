# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 ZeroDDS Contributors
#
# Tests for the DDS-XRCE 1.0 client in `xrce_client.jl`:
#
#   1. Byte-identity of the five Layer 1 message encoders against the Rust-
#      generated fixtures in `../testdata/xrce-client/*.bin`.
#   2. The `XrceClient.Client` state machine's lifecycle rules, translated
#      from `crates/xrce-client/src/lib.rs`'s own `#[cfg(test)] mod tests`.
#
# Run with `julia xrce_client_test.jl` — the file nothing calls goes unrun,
# so this is invoked directly by `tools/endpoints-gate`'s `run_julia`, the
# same way `test.jl`/`surface_test.jl` are.

include("zerodds.jl")
using .ZeroDDS
include("xrce_client.jl")
using .XrceClient

fixture(name) = read(joinpath("..", "testdata", "xrce-client", "$name.bin"))

const KEY1234 = UInt8[0x01, 0x02, 0x03, 0x04]

function fixture_tests()
    # session_id = 0x01 throughout (<=0x7F, so every header after CREATE_CLIENT
    # carries the ClientKey). CREATE_CLIENT travels on stream=NONE, seq 0, with
    # header session 0x00 (SESSION_NONE_WITH_KEY, DDS-XRCE 1.0 Sec 8.3.2.1)
    # while its payload asks for session 0x01. The four
    # post-handshake operations are all reliable, so they share stream=0x80
    # (builtin-reliable) and a real, strictly increasing sequence number
    # (0..3) — not the best-effort stream with hand-picked numbers a
    # previous fixture set used.

    # 1. CREATE_CLIENT: a real CLIENT_Representation (version={1,0},
    #    vendor_id={0,0}, this client_key, session_id=0x01, no properties,
    #    requested_mtu=508).
    repr = XrceClient.encode_client_representation(UInt8[0x01, 0x00], UInt8[0x00, 0x00], KEY1234, UInt8(0x01))
    got = XrceClient.encode_create_client_message(UInt8(0x01), KEY1234, repr)
    want = fixture("xrce_client_create_client")
    @assert got == want "xrce_client_create_client.bin: not byte-identical"
    @assert got[1] == 0x00 "CREATE_CLIENT header session must be 0x00"

    # DDS-XRCE 1.0 Sec 7.8.2.1 prose vs Annex A / B.1: representation_timestamps.
    head = UInt8[0x58, 0x52, 0x43, 0x45, 0x01, 0x00, 0x00, 0x00]
    time_t = UInt8[0x8c, 0xaa, 0x88, 0x5a, 0x00, 0x65, 0xcd, 0x1d]
    tail = UInt8[0x01, 0x02, 0x03, 0x04, 0x01, 0x00, 0xfc, 0x01]
    ver, ven = UInt8[0x01, 0x00], UInt8[0x00, 0x00]
    @assert XrceClient.encode_client_representation(ver, ven, KEY1234, UInt8(0x01)) == vcat(head, tail) "default: 16 octets"
    @assert XrceClient.encode_client_representation(ver, ven, KEY1234, UInt8(0x01);
        representation_timestamps = :absent, seconds = 1518905996, nanoseconds = 500000000) == vcat(head, tail) "absent"
    @assert XrceClient.encode_client_representation(ver, ven, KEY1234, UInt8(0x01);
        representation_timestamps = :present, seconds = 1518905996, nanoseconds = 500000000) ==
        vcat(head, UInt8[0x08, 0x00, 0x00, 0x00], time_t, tail) "present, appendable (28 octets)"
    @assert XrceClient.encode_client_representation(ver, ven, KEY1234, UInt8(0x01);
        representation_timestamps = :present, seconds = 1518905996, nanoseconds = 500000000,
        unannotated_types = :final) == vcat(head, time_t, tail) "present, final (24 octets)"
    @assert length(XrceClient.client_representation(
        XrceClient.Client(KEY1234, nothing; representation_timestamps = :present))) == 28 "client option present"

    # Sec 8.3.2.1: a requested session_id of 128..255 goes in a header with
    # SESSION_NONE_WITHOUT_KEY (0x80), which carries no ClientKey.
    repr81 = XrceClient.encode_client_representation(UInt8[0x01, 0x00], UInt8[0x00, 0x00], KEY1234, UInt8(0x81))
    got81 = XrceClient.encode_create_client_message(UInt8(0x81), KEY1234, repr81)
    @assert got81[1] == 0x80 "CREATE_CLIENT for 0x81: header session must be 0x80"
    @assert length(got81) == 4 + 4 + length(repr81) "CREATE_CLIENT for 0x81: header must not carry the ClientKey"
    @assert got81[4 + 4 + 12 + 1] == 0x81 "CREATE_CLIENT for 0x81: payload keeps session_id 0x81"

    # 2. CREATE: request_id=[0x00,0x01], object_id=pack(raw=0x001,kind=DATAWRITER),
    #    the real two-level ObjectVariant::ByReference("SensorWriter") under
    #    parent=pack(raw=0x001,kind=PUBLISHER), reuse=false, replace=false.
    writer_oid = XrceClient.pack_object_id(UInt16(0x001), XrceClient.DATAWRITER)
    publisher_oid = XrceClient.pack_object_id(UInt16(0x001), XrceClient.PUBLISHER)
    got = XrceClient.encode_create_message(UInt8(0x01), UInt8(0x80), UInt16(0), KEY1234,
                                            (UInt8(0x00), UInt8(0x01)), writer_oid, XrceClient.ByReference("SensorWriter");
                                            parent_object_id = publisher_oid)
    want = fixture("xrce_client_create")
    @assert got == want "xrce_client_create.bin: not byte-identical"

    # 3. DELETE: request_id=[0x00,0x02], object_id=pack(raw=0x001,kind=DATAWRITER)
    #    — deletes the writer CREATEd above.
    got = XrceClient.encode_delete_message(UInt8(0x01), UInt8(0x80), UInt16(1), KEY1234,
                                            (UInt8(0x00), UInt8(0x02)), writer_oid)
    want = fixture("xrce_client_delete")
    @assert got == want "xrce_client_delete.bin: not byte-identical"

    # 4. READ_DATA: request_id=[0x00,0x03], object_id=pack(raw=0x002,kind=DATAREADER),
    #    preferred_stream_id=0x01, data_format=Data, no filter, no delivery control.
    reader_oid = XrceClient.pack_object_id(UInt16(0x002), XrceClient.DATAREADER)
    got = XrceClient.encode_read_data_message(UInt8(0x01), UInt8(0x80), UInt16(2), KEY1234,
                                               (UInt8(0x00), UInt8(0x03)), reader_oid, UInt8(0x01))
    want = fixture("xrce_client_read_data")
    @assert got == want "xrce_client_read_data.bin: not byte-identical"

    # 5. WRITE_DATA: request_id=[0x00,0x04], object_id=pack(raw=0x001,kind=DATAWRITER)
    #    — the same writer — serialized_data=[0x19,0x00].
    got = XrceClient.encode_write_data_message(UInt8(0x01), UInt8(0x80), UInt16(3), KEY1234,
                                                (UInt8(0x00), UInt8(0x04)), writer_oid, UInt8[0x19, 0x00])
    want = fixture("xrce_client_write_data")
    @assert got == want "xrce_client_write_data.bin: not byte-identical"

    # 6. A second CREATE, appended to the same session:
    #    request_id=[0x00,0x05], object_id=pack(raw=0x010,kind=TOPIC), the
    #    real two-level ObjectVariant::ByReference("HelloWorldTopic") under
    #    parent=pack(raw=0x001,kind=PARTICIPANT), sequence_nr=4. OBJK_TOPIC's
    #    own representation carries its participant's ObjectId (commit
    #    31c62df2a63, "xrce: OBJK_TOPIC_Representation carries its
    #    participant's ObjectId"), the same trailing field OBJK_PUBLISHER's
    #    representation carries above — this fixture is generated the same
    #    way (endpoints/golden-gen), not a hand-picked byte string.
    topic_oid = XrceClient.pack_object_id(UInt16(0x010), XrceClient.TOPIC)
    participant_oid = XrceClient.pack_object_id(UInt16(0x001), XrceClient.PARTICIPANT)
    got = XrceClient.encode_create_message(UInt8(0x01), UInt8(0x80), UInt16(4), KEY1234,
                                            (UInt8(0x00), UInt8(0x05)), topic_oid, XrceClient.ByReference("HelloWorldTopic");
                                            parent_object_id = participant_oid)
    want = fixture("xrce_client_create_topic")
    @assert got == want "xrce_client_create_topic.bin: not byte-identical"

    println("xrce-client fixtures: 6/6 byte-identical")
end

# Mirrors `expect_err("...")` + `assert_eq!(err, ClientError::InvalidState)` in
# the Rust reference: the refusal is measured by requiring the exception to
# actually happen, not by inspecting state afterwards.
function refused_invalid_state(f::Function)
    try
        f()
        return false
    catch e
        return e isa XrceClient.InvalidStateError
    end
end

function state_machine_tests()
    test_key = UInt8[0xAB, 0xAB, 0xAB, 0xAB]

    # new_client_starts_disconnected
    c = XrceClient.Client(test_key, ZeroDDS.mem_transport())
    @assert c.state == :disconnected
    @assert c.client_key == test_key

    # connect_transitions_to_connecting
    c = XrceClient.Client(test_key, ZeroDDS.mem_transport())
    XrceClient.connect!(c, UInt8[0x00])
    @assert c.state == :connecting

    # mark_connected_transitions_to_connected
    c = XrceClient.Client(test_key, ZeroDDS.mem_transport())
    XrceClient.connect!(c, UInt8[0x00])
    XrceClient.mark_connected!(c)
    @assert c.state == :connected

    # double_connect_rejected
    c = XrceClient.Client(test_key, ZeroDDS.mem_transport())
    XrceClient.connect!(c, UInt8[0x00])
    @assert refused_invalid_state(() -> XrceClient.connect!(c, UInt8[0x00])) "double_connect_rejected: expected an InvalidStateError"

    # create_without_connect_rejected
    c = XrceClient.Client(test_key, ZeroDDS.mem_transport())
    oid = XrceClient.object_id_from_raw(UInt16(0x0010))
    @assert refused_invalid_state(() -> XrceClient.create_object!(c, oid, XrceClient.ByReference("topic-ref"))) "create_without_connect_rejected: expected an InvalidStateError"

    # write_without_connect_rejected
    c = XrceClient.Client(test_key, ZeroDDS.mem_transport())
    @assert refused_invalid_state(() -> XrceClient.request_write!(c, oid, UInt8[UInt8('x')])) "write_without_connect_rejected: expected an InvalidStateError"

    # read_without_connect_rejected
    c = XrceClient.Client(test_key, ZeroDDS.mem_transport())
    @assert refused_invalid_state(() -> XrceClient.request_read!(c, oid)) "read_without_connect_rejected: expected an InvalidStateError"

    # full_lifecycle_creates_unique_request_ids
    c = XrceClient.Client(test_key, ZeroDDS.mem_transport())
    XrceClient.connect!(c, UInt8[0x00])
    XrceClient.mark_connected!(c)
    r1 = XrceClient.create_object!(c, oid, XrceClient.ByReference("a"))
    r2 = XrceClient.delete_object!(c, oid)
    r3 = XrceClient.request_write!(c, oid, UInt8[UInt8('x')])
    r4 = XrceClient.request_read!(c, oid)
    # Request ids are strictly monotonic.
    @assert r1 == 1 && r2 == 2 && r3 == 3 && r4 == 4

    # disconnect_clears_state
    XrceClient.disconnect!(c)
    @assert c.state == :disconnected

    println("xrce-client state machine: 9/9 OK")
end

fixture_tests()
state_machine_tests()
println("ALL OK")
