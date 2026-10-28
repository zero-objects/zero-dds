# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 ZeroDDS Contributors
#
# Tests for the DDS-XRCE 1.0 client in `lib/xrce_client.ex`:
#
#   1. Byte-identity of the five Layer 1 message encoders (`ZeroDDS.XrceWire`)
#      against the Rust-generated fixtures in `../testdata/xrce-client/*.bin`.
#   2. The `ZeroDDS.XrceClient` state machine's lifecycle rules, translated
#      from `crates/xrce-client/src/lib.rs`'s own `#[cfg(test)] mod tests`.
#
# Run with `elixir -pa beam xrce_client_test.exs` (after `elixirc -o beam
# lib/zerodds.ex lib/xrce_client.ex`), matching the other elixir test scripts
# in this directory.

alias ZeroDDS.{XrceWire, XrceObjectId, XrceObjectKind, XrceClient, MemTransport}

fixture = fn name -> File.read!("../testdata/xrce-client/#{name}.bin") end

key1234 = <<0x01, 0x02, 0x03, 0x04>>

# --- 1. Layer 1 wire-encoder byte-identity ----------------------------------
#
# session_id = 0x01 throughout (<=0x7F, so every header except CREATE_CLIENT's carries the
# ClientKey). CREATE_CLIENT travels on stream=NONE, seq 0. The four
# post-handshake operations are all reliable, so they share stream=0x80
# (builtin-reliable) and a real, strictly increasing sequence number (0..3)
# — not the best-effort stream with hand-picked numbers a previous fixture
# set used.

# CREATE_CLIENT: a real CLIENT_Representation (version={1,0},
# vendor_id={0,0}, this client_key, session_id=0x01, no properties,
# requested_mtu=508).
repr = XrceWire.encode_client_representation(<<0x01, 0x00>>, <<0x00, 0x00>>, key1234, 0x01)
got = XrceWire.encode_create_client_message(0x01, key1234, repr)
want = fixture.("xrce_client_create_client")
if got != want, do: raise("xrce_client_create_client.bin: not byte-identical")

# CLIENT_Representation, option representation_timestamps (7.8.2.1 prose vs Annex A / B.1).
ts = [timestamp: {1_518_905_996, 500_000_000}]
want_absent = <<0x58, 0x52, 0x43, 0x45, 1, 0, 0, 0, 1, 2, 3, 4, 1, 0, 0xFC, 1>>
want_app = <<0x58, 0x52, 0x43, 0x45, 1, 0, 0, 0, 8, 0, 0, 0, 0x8C, 0xAA, 0x88, 0x5A, 0x00, 0x65, 0xCD, 0x1D, 1, 2, 3, 4, 1, 0, 0xFC, 1>>
want_final = <<0x58, 0x52, 0x43, 0x45, 1, 0, 0, 0, 0x8C, 0xAA, 0x88, 0x5A, 0x00, 0x65, 0xCD, 0x1D, 1, 2, 3, 4, 1, 0, 0xFC, 1>>
if XrceWire.encode_client_representation(<<1, 0>>, <<0, 0>>, key1234, 1) != want_absent, do: raise("representation default must stay absent")
if XrceWire.encode_client_representation(<<1, 0>>, <<0, 0>>, key1234, 1, 508, representation_timestamps: :absent) != want_absent, do: raise("representation absent")
if XrceWire.encode_client_representation(<<1, 0>>, <<0, 0>>, key1234, 1, 508, [representation_timestamps: :present] ++ ts) != want_app, do: raise("representation present appendable")
if XrceWire.encode_client_representation(<<1, 0>>, <<0, 0>>, key1234, 1, 508, [representation_timestamps: :present, unannotated_types: :final] ++ ts) != want_final, do: raise("representation present final")
cl = XrceClient.new(key1234, MemTransport.new(), representation_timestamps: :present)
if byte_size(XrceClient.client_representation(cl, <<1, 0>>, <<0, 0>>)) != 28, do: raise("client representation present: 28 octets")
if byte_size(XrceClient.client_representation(XrceClient.new(key1234, MemTransport.new()), <<1, 0>>, <<0, 0>>)) != 16, do: raise("client representation default: 16 octets")

# DDS-XRCE 1.0 8.3.2.1: requested session id >= 128 -> header 0x80, no client key.
repr_hi = XrceWire.encode_client_representation(<<0x01, 0x00>>, <<0x00, 0x00>>, key1234, 0x81)
got_hi = XrceWire.encode_create_client_message(0x81, key1234, repr_hi)
<<0x80, _::binary>> = got_hi
if byte_size(got_hi) != byte_size(want) - 4, do: raise("CREATE_CLIENT with session 0x81: header must carry no client key")

# CREATE: request_id=[0x00,0x01], object_id=pack(raw=0x001,kind=datawriter),
# the real two-level ObjectVariant::ByReference("SensorWriter") under
# parent=pack(raw=0x001,kind=publisher), reuse=false, replace=false.
oid = XrceObjectId.pack(0x001, XrceObjectKind.datawriter())
publisher_oid = XrceObjectId.pack(0x001, XrceObjectKind.publisher())
got = XrceWire.encode_create_message(0x01, 0x80, 0, key1234, <<0x00, 0x01>>, oid, "SensorWriter", publisher_oid)
want = fixture.("xrce_client_create")
if got != want, do: raise("xrce_client_create.bin: not byte-identical")

# DELETE: request_id=[0x00,0x02], object_id=pack(raw=0x001,kind=datawriter) —
# deletes the writer CREATEd above.
got = XrceWire.encode_delete_message(0x01, 0x80, 1, key1234, <<0x00, 0x02>>, oid)
want = fixture.("xrce_client_delete")
if got != want, do: raise("xrce_client_delete.bin: not byte-identical")

# READ_DATA: request_id=[0x00,0x03], object_id=pack(raw=0x002,kind=datareader),
# preferred_stream_id=0x01, data_format=Data, no filter, no delivery control.
read_oid = XrceObjectId.pack(0x002, XrceObjectKind.datareader())
got = XrceWire.encode_read_data_message(0x01, 0x80, 2, key1234, <<0x00, 0x03>>, read_oid, 0x01)
want = fixture.("xrce_client_read_data")
if got != want, do: raise("xrce_client_read_data.bin: not byte-identical")

# WRITE_DATA: request_id=[0x00,0x04], object_id=pack(raw=0x001,kind=datawriter)
# — the same writer — serialized_data=[0x19,0x00].
got = XrceWire.encode_write_data_message(0x01, 0x80, 3, key1234, <<0x00, 0x04>>, oid, <<0x19, 0x00>>)
want = fixture.("xrce_client_write_data")
if got != want, do: raise("xrce_client_write_data.bin: not byte-identical")

# A second CREATE, appended to the same session: request_id=[0x00,0x05],
# object_id=pack(raw=0x010,kind=topic), the real two-level
# ObjectVariant::ByReference("HelloWorldTopic") under
# parent=pack(raw=0x001,kind=participant), sequence_nr=4. OBJK_TOPIC's own
# representation carries its participant's ObjectId (commit 31c62df2a63,
# "xrce: OBJK_TOPIC_Representation carries its participant's ObjectId"),
# the same trailing field OBJK_PUBLISHER's representation carries above —
# this fixture is generated the same way (endpoints/golden-gen), not a
# hand-picked byte string.
topic_oid = XrceObjectId.pack(0x010, XrceObjectKind.topic())
participant_oid = XrceObjectId.pack(0x001, XrceObjectKind.participant())

got =
  XrceWire.encode_create_message(
    0x01,
    0x80,
    4,
    key1234,
    <<0x00, 0x05>>,
    topic_oid,
    "HelloWorldTopic",
    participant_oid
  )

want = fixture.("xrce_client_create_topic")
if got != want, do: raise("xrce_client_create_topic.bin: not byte-identical")

IO.puts("xrce-client fixtures: 6/6 byte-identical")

# --- 2. State-machine lifecycle rules ----------------------------------------

test_key = <<0xAB, 0xAB, 0xAB, 0xAB>>

# new_client_starts_disconnected
c = XrceClient.new(test_key, MemTransport.new())
if c.state != :disconnected, do: raise("new client must start :disconnected")
if c.client_key != test_key, do: raise("client_key must round-trip")

# connect_transitions_to_connecting
c = XrceClient.new(test_key, MemTransport.new())
{:ok, c} = XrceClient.connect(c, <<0x00>>)
if c.state != :connecting, do: raise("connect must transition to :connecting")

# mark_connected_transitions_to_connected
{:ok, c} = XrceClient.mark_connected(c)
if c.state != :connected, do: raise("mark_connected must transition to :connected")

# double_connect_rejected
case XrceClient.connect(c, <<0x00>>) do
  {:error, :invalid_state} -> :ok
  other -> raise("double_connect_rejected: expected {:error, :invalid_state}, got #{inspect(other)}")
end

# create_without_connect_rejected
fresh = XrceClient.new(test_key, MemTransport.new())
oid = XrceObjectId.from_raw_word(0x0010)

case XrceClient.create_object(fresh, oid, "topic-ref") do
  {:error, :invalid_state} -> :ok
  other -> raise("create_without_connect_rejected: expected {:error, :invalid_state}, got #{inspect(other)}")
end

# write_without_connect_rejected
case XrceClient.request_write(fresh, oid, "x") do
  {:error, :invalid_state} -> :ok
  other -> raise("write_without_connect_rejected: expected {:error, :invalid_state}, got #{inspect(other)}")
end

# read_without_connect_rejected
case XrceClient.request_read(fresh, oid) do
  {:error, :invalid_state} -> :ok
  other -> raise("read_without_connect_rejected: expected {:error, :invalid_state}, got #{inspect(other)}")
end

# full_lifecycle_creates_unique_request_ids
{:ok, req1, c} = XrceClient.create_object(c, oid, "a")
{:ok, req2, c} = XrceClient.delete_object(c, oid)
{:ok, req3, c} = XrceClient.request_write(c, oid, "x")
{:ok, req4, c} = XrceClient.request_read(c, oid)
# Request ids are strictly monotonic.
if {req1, req2, req3, req4} != {1, 2, 3, 4},
  do: raise("request ids must be strictly monotonic 1,2,3,4, got #{inspect({req1, req2, req3, req4})}")

# disconnect_clears_state
c = XrceClient.disconnect(c)
if c.state != :disconnected, do: raise("disconnect must return to :disconnected")

IO.puts("xrce-client state machine: 9/9 OK")

IO.puts("ALL OK")
