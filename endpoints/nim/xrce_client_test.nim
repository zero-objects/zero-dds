# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 ZeroDDS Contributors
#
# Tests for the DDS-XRCE 1.0 client in `xrce_client.nim`:
#
#   1. Byte-identity of the five Layer 1 message encoders against the Rust-
#      generated fixtures in `../testdata/xrce-client/*.bin`.
#   2. The `XrceClient` state machine's lifecycle rules, translated from
#      `crates/xrce-client/src/lib.rs`'s own `#[cfg(test)] mod tests`.
#
# Called from `test.nim` (`proc xrceClientTests()`), the one file GitLab CI
# runs — a test file nothing imports goes unrun.

import std/options
import ./zerodds
import ./xrce_client

# Raw-bytes file read: `readFile` returns the file's bytes verbatim in a
# Nim string (no text/encoding transform), so converting char-by-char to
# `seq[byte]` is a binary-safe read, not a text one. Same pattern `test.nim`
# already uses for the XCDR goldens.
proc toBytes(s: string): seq[byte] =
  result = newSeq[byte](s.len)
  for i in 0 ..< s.len:
    result[i] = byte(s[i])

proc fixture(name: string): seq[byte] =
  toBytes(readFile("../testdata/xrce-client/" & name & ".bin"))

proc asciiBytes(s: string): seq[byte] =
  result = newSeq[byte](s.len)
  for i in 0 ..< s.len:
    result[i] = byte(s[i])

const key1234: ClientKeyBytes = [0x01'u8, 0x02, 0x03, 0x04]

proc fixtureTests() =
  # session_id = 0x01 throughout (<=0x7F, so every header except CREATE_CLIENT's carries the
  # ClientKey). CREATE_CLIENT travels on stream=NONE, seq 0. The four
  # post-handshake operations are all reliable, so they share stream=0x80
  # (builtin-reliable) and a real, strictly increasing sequence number
  # (0..3) — not the best-effort stream with hand-picked numbers a
  # previous fixture set used.

  # 1. CREATE_CLIENT: a real CLIENT_Representation (version={1,0},
  #    vendor_id={0,0}, this client_key, session_id=0x01, no properties,
  #    requested_mtu=508).
  block:
    let repr = encodeClientRepresentation([0x01'u8, 0x00'u8], [0x00'u8, 0x00'u8], key1234, 0x01'u8)
    let got = encodeCreateClientMessage(0x01'u8, key1234, repr)
    let want = fixture("xrce_client_create_client")
    doAssert got == want, "xrce_client_create_client.bin: not byte-identical"
    # representation timestamps (7.8.2.1 prose vs Annex A / B.1)
    let v = [0x01'u8, 0x00'u8]
    let vd = [0x00'u8, 0x00'u8]
    let wantAbsent = @[0x58'u8, 0x52, 0x43, 0x45, 1, 0, 0, 0, 1, 2, 3, 4, 1, 0, 0xFC, 1]
    let wantApp = @[0x58'u8, 0x52, 0x43, 0x45, 1, 0, 0, 0, 8, 0, 0, 0,
                    0x8C, 0xAA, 0x88, 0x5A, 0x00, 0x65, 0xCD, 0x1D, 1, 2, 3, 4, 1, 0, 0xFC, 1]
    let wantFinal = @[0x58'u8, 0x52, 0x43, 0x45, 1, 0, 0, 0,
                      0x8C, 0xAA, 0x88, 0x5A, 0x00, 0x65, 0xCD, 0x1D, 1, 2, 3, 4, 1, 0, 0xFC, 1]
    doAssert repr == wantAbsent, "default representation must stay absent"
    doAssert encodeClientRepresentation(v, vd, key1234, 0x01'u8, 508'u16,
      RepresentationTimestamps.Absent) == wantAbsent
    doAssert encodeClientRepresentation(v, vd, key1234, 0x01'u8, 508'u16,
      RepresentationTimestamps.Present, 1518905996'i32, 500000000'u32) == wantApp
    doAssert encodeClientRepresentation(v, vd, key1234, 0x01'u8, 508'u16,
      RepresentationTimestamps.Present, 1518905996'i32, 500000000'u32,
      UnannotatedTypes.Final) == wantFinal
    let cl = newXrceClient(key1234, memTransport(), RepresentationTimestamps.Present)
    doAssert cl.clientRepresentation(v, vd).len == 28
    doAssert newXrceClient(key1234, memTransport()).clientRepresentation(v, vd).len == 16
    # DDS-XRCE 1.0 Sec 8.3.2.1: requested session id >= 128 -> header 0x80,
    # no client key.
    let reprHi = encodeClientRepresentation([0x01'u8, 0x00'u8], [0x00'u8, 0x00'u8], key1234, 0x81'u8)
    let gotHi = encodeCreateClientMessage(0x81'u8, key1234, reprHi)
    doAssert gotHi[0] == 0x80'u8, "CREATE_CLIENT with session 0x81: header session id must be 0x80"
    doAssert gotHi.len == want.len - 4, "CREATE_CLIENT with session 0x81: header must carry no client key"

  # 2. CREATE: request_id=[0x00,0x01], object_id=pack(raw=0x001,kind=DATAWRITER),
  #    the real two-level ObjectVariant::ByReference("SensorWriter") under
  #    parent=pack(raw=0x001,kind=PUBLISHER), reuse=false, replace=false.
  let writerOid = packObjectId(0x001'u16, okDatawriter)
  block:
    let publisherOid = packObjectId(0x001'u16, okPublisher)
    let got = encodeCreateMessage(0x01'u8, 0x80'u8, 0'u16, key1234,
                                   [0x00'u8, 0x01'u8], writerOid, "SensorWriter", some(publisherOid))
    let want = fixture("xrce_client_create")
    doAssert got == want, "xrce_client_create.bin: not byte-identical"

  # 3. DELETE: request_id=[0x00,0x02], object_id=pack(raw=0x001,kind=DATAWRITER)
  #    — deletes the writer CREATEd above.
  block:
    let got = encodeDeleteMessage(0x01'u8, 0x80'u8, 1'u16, key1234,
                                   [0x00'u8, 0x02'u8], writerOid)
    let want = fixture("xrce_client_delete")
    doAssert got == want, "xrce_client_delete.bin: not byte-identical"

  # 4. READ_DATA: request_id=[0x00,0x03], object_id=pack(raw=0x002,kind=DATAREADER),
  #    preferred_stream_id=0x01, data_format=Data, no filter, no delivery control.
  block:
    let oid = packObjectId(0x002'u16, okDatareader)
    let got = encodeReadDataMessage(0x01'u8, 0x80'u8, 2'u16, key1234,
                                     [0x00'u8, 0x03'u8], oid, 0x01'u8)
    let want = fixture("xrce_client_read_data")
    doAssert got == want, "xrce_client_read_data.bin: not byte-identical"

  # 5. WRITE_DATA: request_id=[0x00,0x04], object_id=pack(raw=0x001,kind=DATAWRITER)
  #    — the same writer — serialized_data=[0x19,0x00].
  block:
    let got = encodeWriteDataMessage(0x01'u8, 0x80'u8, 3'u16, key1234,
                                      [0x00'u8, 0x04'u8], writerOid, @[0x19'u8, 0x00'u8])
    let want = fixture("xrce_client_write_data")
    doAssert got == want, "xrce_client_write_data.bin: not byte-identical"

  # 6. A second CREATE, appended to the same session: request_id=[0x00,0x05],
  #    object_id=pack(raw=0x010,kind=TOPIC), the real two-level
  #    ObjectVariant::ByReference("HelloWorldTopic") under
  #    parent=pack(raw=0x001,kind=PARTICIPANT), sequence_nr=4. OBJK_TOPIC's
  #    own representation carries its participant's ObjectId (commit
  #    31c62df2a63, "xrce: OBJK_TOPIC_Representation carries its
  #    participant's ObjectId"), the same trailing field OBJK_PUBLISHER's
  #    representation carries above — this fixture is generated the same
  #    way (endpoints/golden-gen), not a hand-picked byte string.
  block:
    let topicOid = packObjectId(0x010'u16, okTopic)
    let participantOid = packObjectId(0x001'u16, okParticipant)
    let got = encodeCreateMessage(0x01'u8, 0x80'u8, 4'u16, key1234,
                                   [0x00'u8, 0x05'u8], topicOid, "HelloWorldTopic",
                                   some(participantOid))
    let want = fixture("xrce_client_create_topic")
    doAssert got == want, "xrce_client_create_topic.bin: not byte-identical"

  echo "xrce-client fixtures: 6/6 byte-identical"

template expectInvalidState(label: string, body: untyped) =
  ## Mirrors `expect_err("...")` + `assert_eq!(err, ClientError::InvalidState)`
  ## in the Rust reference: the refusal is measured by requiring the
  ## exception to actually happen, with the right kind, not by inspecting
  ## state afterwards.
  var refused = false
  try:
    body
  except XrceClientError as e:
    refused = e.kind == cekInvalidState
  doAssert refused, label & ": expected an InvalidState refusal"

proc stateMachineTests() =
  let testKey: ClientKeyBytes = [0xAB'u8, 0xAB, 0xAB, 0xAB]

  block: # new_client_starts_disconnected
    let c = newXrceClient(testKey, memTransport())
    doAssert c.state() == csDisconnected
    doAssert c.clientKey() == testKey

  block: # connect_transitions_to_connecting
    let c = newXrceClient(testKey, memTransport())
    c.connect(@[0x00'u8])
    doAssert c.state() == csConnecting

  block: # mark_connected_transitions_to_connected
    let c = newXrceClient(testKey, memTransport())
    c.connect(@[0x00'u8])
    c.markConnected()
    doAssert c.state() == csConnected

  block: # double_connect_rejected
    let c = newXrceClient(testKey, memTransport())
    c.connect(@[0x00'u8])
    expectInvalidState("double_connect_rejected"):
      c.connect(@[0x00'u8])

  block: # create_without_connect_rejected
    let c = newXrceClient(testKey, memTransport())
    let oid = fromRawWord(0x0010'u16)
    expectInvalidState("create_without_connect_rejected"):
      discard c.createObject(oid, "topic-ref")

  block: # write_without_connect_rejected
    let c = newXrceClient(testKey, memTransport())
    let oid = fromRawWord(0x0010'u16)
    expectInvalidState("write_without_connect_rejected"):
      discard c.requestWrite(oid, @[byte('x')])

  block: # read_without_connect_rejected
    let c = newXrceClient(testKey, memTransport())
    let oid = fromRawWord(0x0010'u16)
    expectInvalidState("read_without_connect_rejected"):
      discard c.requestRead(oid)

  block: # full_lifecycle_creates_unique_request_ids
    let c = newXrceClient(testKey, memTransport())
    c.connect(@[0x00'u8])
    c.markConnected()
    let oid = fromRawWord(0x0010'u16)
    let r1 = c.createObject(oid, "a")
    let r2 = c.deleteObject(oid)
    let r3 = c.requestWrite(oid, @[byte('x')])
    let r4 = c.requestRead(oid)
    # Request ids are strictly monotonic.
    doAssert r1 == 1'u16
    doAssert r2 == 2'u16
    doAssert r3 == 3'u16
    doAssert r4 == 4'u16

  block: # disconnect_clears_state
    let c = newXrceClient(testKey, memTransport())
    c.connect(@[0x00'u8])
    c.markConnected()
    c.disconnect()
    doAssert c.state() == csDisconnected

  echo "xrce-client state machine: 9/9 OK"

proc xrceClientTests*() =
  fixtureTests()
  stateMachineTests()
