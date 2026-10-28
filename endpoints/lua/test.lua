-- SPDX-License-Identifier: Apache-2.0
-- Copyright 2026 ZeroDDS Contributors
--
-- Tests for the native Lua endpoint: byte-identity vs the Rust goldens, plus
-- sync + async loopback. Run with `lua5.4 test.lua` (GOLDEN_DIR=build).

package.path = "./?.lua;" .. package.path
local z = require("zerodds")
local xc = require("xrce_client")

local goldenDir = os.getenv("GOLDEN_DIR") or "build"

local function fixture(endian)
  local w = z.Writer.new(endian, z.XCDR2)
  w:putU32(0xA1B2C3D4)
  w:putU16(0x1234)
  w:putU8(0x5A)
  w:putF32(3.5)
  w:putU64(0x0102030405060708)
  w:putString("bay-12")
  w:putSeqU8("\xDE\xAD\xBE\xEF")
  return w:bytes()
end

local function sampleBody(id)
  local w = z.Writer.new(z.LE, z.XCDR2)
  w:putU32(id)
  w:putU16(0)
  w:putU8(0)
  return w:bytes()
end

local function readFile(path)
  local f = assert(io.open(path, "rb"))
  local data = f:read("*a")
  f:close()
  return data
end

-- byte identity
for _, pair in ipairs({ { z.LE, "golden_le.bin" }, { z.BE, "golden_be.bin" } }) do
  local got = fixture(pair[1])
  local golden = readFile(goldenDir .. "/" .. pair[2])
  assert(got == golden, pair[2] .. ": not byte-identical (got " .. #got .. " want " .. #golden .. ")")
  print(pair[2] .. ": " .. #golden .. " bytes byte-identical")
end

-- sync loopback
local t = z.memTransport()
local c = z.Client.new(t, z.objectId(0x001, z.OBJK_DATAWRITER))
for i = 0, 4 do c:write(sampleBody(0x3000 + i)) end
for i = 0, 4 do
  local body = c:poll()
  assert(body ~= nil, "sync: no sample")
  assert(z.Reader.new(body, z.LE, z.XCDR2):getU32() == 0x3000 + i, "sync: id mismatch")
end
print("sync loopback: 5 samples OK")

-- async loopback (coroutine producer)
local t2 = z.memTransport()
local w = z.Client.new(t2, z.objectId(0x001, z.OBJK_DATAWRITER))
for i = 0, 4 do w:write(sampleBody(0x1000 + i)) end
local reader = z.asyncReader(t2)
for i = 0, 4 do
  local body
  repeat body = reader() until body ~= nil
  assert(z.Reader.new(body, z.LE, z.XCDR2):getU32() == 0x1000 + i, "async: id mismatch")
end
print("async loopback: 5 samples OK")

-- negative frame vectors: length bounding + malformed reject
-- The DataWriter these vectors address (7.2.1: raw 0x001 + OBJK_DATAWRITER).
local WRITER_ID = z.objectId(0x001, z.OBJK_DATAWRITER)
local first = z.writeFrame(0x80, 0x01, 1, WRITER_ID, "\xAA\xBB\xCC")
local second = z.writeFrame(0x80, 0x01, 2, WRITER_ID, "\xDD\xEE")
assert(z.readFrame(first .. second) == "\xAA\xBB\xCC",
  "appended submessage must not leak into sample")
local overlong = first:sub(1, 6) .. "\xFF\xFF" .. first:sub(9)
assert(z.readFrame(overlong) == nil, "over-long length must reject")
assert(z.readFrame("\x80\x01\x00\x00\x07\x01\x04\x00") == nil, "truncated header must reject")
-- A refusal and no longer nil: `Client:write` handed the nil to `deliver` and
-- nothing at all reached the link with no error anywhere.
local ok = pcall(z.writeFrame, 0x80, 0x01, 1, WRITER_ID, string.rep("x", 0x10000))
assert(not ok, "sample > 0xFFFF must be refused")
-- The bound is the field's and not one off it.
assert(#z.writeFrame(0x80, 0x01, 1, WRITER_ID, string.rep("x", 0xFFFF - z.XRCE_BASE_SIZE))
  == 8 + 0xFFFF, "a maximal body must still frame")
print("negative frame vectors: OK")

-- xrce_client: Layer 1 byte identity against the five fixtures in
-- endpoints/testdata/xrce-client/, produced by endpoints/golden-gen
-- actually running zerodds_xrce_client::XrceClient (crates/xrce-client)
-- over zerodds_xrce (crates/xrce). Every fixture uses client_key =
-- [0x01,0x02,0x03,0x04], session_id = 0x01 (<=0x7F, so every header
-- after CREATE_CLIENT carries the client_key). CREATE_CLIENT travels on
-- STREAM_NONE, seq 0, with header session 0x00 (SESSION_NONE_WITH_KEY,
-- DDS-XRCE 1.0 §8.3.2.1) while its payload asks for session 0x01;
-- the four post-handshake operations are all reliable, so they share
-- STREAM_BUILTIN_RELIABLE (0x80) and a real, strictly increasing
-- sequence number (0..3) — not the best-effort stream with hand-picked
-- numbers a previous fixture set used.
local xrceKey = "\x01\x02\x03\x04"
local testdataDir = "../testdata/xrce-client"

local createClientRepr = xc.encodeClientRepresentation("\x01\x00", "\x00\x00", xrceKey, 0x01)
local got = xc.encodeCreateClient(0x01, xrceKey, createClientRepr)
local golden = readFile(testdataDir .. "/xrce_client_create_client.bin")
assert(got == golden, "xrce CREATE_CLIENT: not byte-identical")
assert(got:byte(1) == 0x00, "xrce CREATE_CLIENT: header session must be 0x00")

-- DDS-XRCE 1.0 §7.8.2.1 prose vs Annex A / B.1: representationTimestamps.
do
  local function bytes(...) return string.char(...) end
  local head = bytes(0x58, 0x52, 0x43, 0x45, 0x01, 0x00, 0x00, 0x00)
  local timeT = bytes(0x8c, 0xaa, 0x88, 0x5a, 0x00, 0x65, 0xcd, 0x1d)
  local tail = bytes(0x01, 0x02, 0x03, 0x04, 0x01, 0x00, 0xfc, 0x01)
  local absent = head .. tail
  local appendable = head .. bytes(0x08, 0x00, 0x00, 0x00) .. timeT .. tail
  local final = head .. timeT .. tail
  local t = { seconds = 1518905996, nanoseconds = 500000000 }
  assert(xc.encodeClientRepresentation("\x01\x00", "\x00\x00", xrceKey, 0x01) == absent,
    "representation default: 16 octets")
  assert(xc.encodeClientRepresentation("\x01\x00", "\x00\x00", xrceKey, 0x01, 508,
    { representationTimestamps = "absent", seconds = t.seconds, nanoseconds = t.nanoseconds }) == absent,
    "representationTimestamps absent")
  assert(xc.encodeClientRepresentation("\x01\x00", "\x00\x00", xrceKey, 0x01, 508,
    { representationTimestamps = "present", seconds = t.seconds, nanoseconds = t.nanoseconds }) == appendable,
    "representationTimestamps present, appendable (28 octets)")
  assert(xc.encodeClientRepresentation("\x01\x00", "\x00\x00", xrceKey, 0x01, 508,
    { representationTimestamps = "present", unannotatedTypes = "final",
      seconds = t.seconds, nanoseconds = t.nanoseconds }) == final,
    "representationTimestamps present, final (24 octets)")
  local pc = xc.XrceClient.new(xrceKey, { deliver = function() end }, { representationTimestamps = "present" })
  assert(#pc:clientRepresentation() == 28, "client option present: 28 octets")
end

-- §8.3.2.1: a requested session_id of 128..255 goes in a header with
-- SESSION_NONE_WITHOUT_KEY (0x80), which carries no client_key.
local repr81 = xc.encodeClientRepresentation("\x01\x00", "\x00\x00", xrceKey, 0x81)
local got81 = xc.encodeCreateClient(0x81, xrceKey, repr81)
assert(got81:byte(1) == 0x80, "xrce CREATE_CLIENT 0x81: header session must be 0x80")
assert(#got81 == 4 + 4 + #repr81, "xrce CREATE_CLIENT 0x81: header must not carry the client_key")
assert(got81:byte(4 + 4 + 12 + 1) == 0x81, "xrce CREATE_CLIENT 0x81: payload keeps session_id 0x81")

local writerId = xc.packObjectId(0x001, xc.ObjectKind.DATAWRITER)
local publisherId = xc.packObjectId(0x001, xc.ObjectKind.PUBLISHER)
got = xc.encodeCreate(0x01, xc.STREAM_BUILTIN_RELIABLE, 0, xrceKey, "\x00\x01",
  writerId, xc.encodeObjectVariantForKind(xc.ObjectKind.DATAWRITER, "SensorWriter", publisherId), false, false)
golden = readFile(testdataDir .. "/xrce_client_create.bin")
assert(got == golden, "xrce CREATE: not byte-identical")

got = xc.encodeDelete(0x01, xc.STREAM_BUILTIN_RELIABLE, 1, xrceKey, "\x00\x02", writerId)
golden = readFile(testdataDir .. "/xrce_client_delete.bin")
assert(got == golden, "xrce DELETE: not byte-identical")

got = xc.encodeReadData(0x01, xc.STREAM_BUILTIN_RELIABLE, 2, xrceKey, "\x00\x03",
  xc.packObjectId(0x002, xc.ObjectKind.DATAREADER), 0x01)
golden = readFile(testdataDir .. "/xrce_client_read_data.bin")
assert(got == golden, "xrce READ_DATA: not byte-identical")

got = xc.encodeWriteData(0x01, xc.STREAM_BUILTIN_RELIABLE, 3, xrceKey, "\x00\x04", writerId, "\x19\x00")
golden = readFile(testdataDir .. "/xrce_client_write_data.bin")
assert(got == golden, "xrce WRITE_DATA: not byte-identical")

-- A second CREATE, appended to the same session: request_id=[0x00,0x05],
-- object_id=pack(raw=0x010,kind=TOPIC), the real two-level
-- ObjectVariant::ByReference("HelloWorldTopic") under
-- parent=pack(raw=0x001,kind=PARTICIPANT), sequence_nr=4. OBJK_TOPIC's own
-- representation carries its participant's ObjectId (commit 31c62df2a63,
-- "xrce: OBJK_TOPIC_Representation carries its participant's ObjectId"),
-- the same trailing field OBJK_PUBLISHER's representation carries above —
-- this fixture is generated the same way (endpoints/golden-gen), not a
-- hand-picked byte string.
local topicId = xc.packObjectId(0x010, xc.ObjectKind.TOPIC)
local participantId = xc.packObjectId(0x001, xc.ObjectKind.PARTICIPANT)
got = xc.encodeCreate(0x01, xc.STREAM_BUILTIN_RELIABLE, 4, xrceKey, "\x00\x05",
  topicId, xc.encodeObjectVariantForKind(xc.ObjectKind.TOPIC, "HelloWorldTopic", participantId), false, false)
golden = readFile(testdataDir .. "/xrce_client_create_topic.bin")
assert(got == golden, "xrce CREATE(topic): not byte-identical")

print("xrce_client wire encoders: 6/6 fixtures byte-identical")

-- xrce_client: Layer 2 state machine, translated directly from
-- crates/xrce-client/src/lib.rs's own `#[cfg(test)] mod tests`.
local function xrceKeyFixture() return string.rep("\xAB", 4) end

do -- new_client_starts_disconnected
  local c = xc.XrceClient.new(xrceKeyFixture(), z.memTransport())
  assert(c:state() == xc.ClientState.DISCONNECTED, "new client must start Disconnected")
  assert(c:clientKey() == xrceKeyFixture(), "client_key must round-trip")
end

do -- connect_transitions_to_connecting
  local c = xc.XrceClient.new(xrceKeyFixture(), z.memTransport())
  c:connect("rep")
  assert(c:state() == xc.ClientState.CONNECTING, "connect must move to Connecting")
end

do -- mark_connected_transitions_to_connected
  local c = xc.XrceClient.new(xrceKeyFixture(), z.memTransport())
  c:connect("rep")
  c:markConnected()
  assert(c:state() == xc.ClientState.CONNECTED, "markConnected must move to Connected")
end

do -- double_connect_rejected
  local c = xc.XrceClient.new(xrceKeyFixture(), z.memTransport())
  c:connect("rep")
  local ok = pcall(function() c:connect("rep") end)
  assert(not ok, "a second connect() must be refused (InvalidState)")
end

do -- create_without_connect_rejected
  local c = xc.XrceClient.new(xrceKeyFixture(), z.memTransport())
  local oid = xc.objectIdFromRaw(0x0010)
  local ok = pcall(function()
    c:createObject(oid, xc.encodeObjectVariantByReference("topic-ref"))
  end)
  assert(not ok, "createObject before connect must be refused (InvalidState)")
end

do -- write_without_connect_rejected
  local c = xc.XrceClient.new(xrceKeyFixture(), z.memTransport())
  local oid = xc.objectIdFromRaw(0x0010)
  local ok = pcall(function() c:requestWrite(oid, "x") end)
  assert(not ok, "requestWrite before connect must be refused (InvalidState)")
end

do -- read_without_connect_rejected
  local c = xc.XrceClient.new(xrceKeyFixture(), z.memTransport())
  local oid = xc.objectIdFromRaw(0x0010)
  local ok = pcall(function() c:requestRead(oid) end)
  assert(not ok, "requestRead before connect must be refused (InvalidState)")
end

do -- full_lifecycle_creates_unique_request_ids
  local c = xc.XrceClient.new(xrceKeyFixture(), z.memTransport())
  c:connect("rep")
  c:markConnected()
  local oid = xc.objectIdFromRaw(0x0010)
  local r1 = c:createObject(oid, xc.encodeObjectVariantByReference("a"))
  local r2 = c:deleteObject(oid)
  local r3 = c:requestWrite(oid, "x")
  local r4 = c:requestRead(oid)
  assert(r1 == 1 and r2 == 2 and r3 == 3 and r4 == 4,
    "request ids must be strictly monotonic: 1,2,3,4")
end

do -- disconnect_clears_state
  local c = xc.XrceClient.new(xrceKeyFixture(), z.memTransport())
  c:connect("rep")
  c:markConnected()
  c:disconnect()
  assert(c:state() == xc.ClientState.DISCONNECTED, "disconnect must return to Disconnected")
end

print("xrce_client state machine: 9/9 behavioral tests OK")

print("ALL OK")
