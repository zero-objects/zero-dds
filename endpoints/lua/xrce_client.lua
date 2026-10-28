-- SPDX-License-Identifier: Apache-2.0
-- Copyright 2026 ZeroDDS Contributors
--
-- DDS-XRCE 1.0 client: a from-scratch port of `crates/xrce-client` (Rust)
-- plus the wire codecs it delegates to in `crates/xrce`. This is additive
-- to `zerodds.lua`'s `Client`/`asyncReader`, which speak an older,
-- in-house, non-conformant frame on purpose (see the comment above
-- `encode_xrce_frame` in `endpoints/golden-gen/src/main.rs`) — this module
-- is the conformant sibling that actually reaches a real DDS-XRCE agent.
--
-- Two layers, the same split the port was specified against:
--   Layer 1 — pure wire encoders (no state, no I/O): MessageHeader,
--     SubmessageHeader, ObjectId, BaseObjectRequest, ObjectVariant, and
--     the five full submessage encoders (CREATE_CLIENT/CREATE/DELETE/
--     WRITE_DATA/READ_DATA).
--   Layer 2 — `XrceClient`, a state machine mirroring
--     `crates/xrce-client/src/lib.rs` 1:1 (states, InvalidState
--     refusals, monotonic request-id allocation) — but unlike the Rust
--     reference, whose `create_object`/`request_write`/etc. only track
--     state and leave encoding to an unspecified caller ("The caller
--     delegates submessage encoding; we only track the state"), this
--     port also builds and delivers the wire frame itself. Closing the
--     "no conformant way to reach a real XRCE agent" gap is the point of
--     this file, so the state machine has to actually speak the wire.

local M = {}

-- --- Layer 1: wire encoders ------------------------------------------------

-- ObjectKind nibble values (Spec §7.2.1, crates/xrce/src/object_kind.rs).
M.ObjectKind = {
  PARTICIPANT = 1, TOPIC = 2, PUBLISHER = 3, SUBSCRIBER = 4,
  DATAWRITER = 5, DATAREADER = 6, TYPE = 0x0A, QOSPROFILE = 0x0B,
  APPLICATION = 0x0C, AGENT = 0x0D, CLIENT = 0x0E, DOMAIN = 0x0F,
}

-- Reserved SessionId/StreamId values, Spec §8.3.2.
M.SESSION_NONE_WITH_KEY = 0x00
M.SESSION_NONE_WITHOUT_KEY = 0x80
M.STREAM_NONE = 0x00
M.STREAM_BUILTIN_BEST_EFFORT = 0x01
M.STREAM_BUILTIN_RELIABLE = 0x80

local SUBMSG_CREATE_CLIENT = 0
local SUBMSG_CREATE = 1
local SUBMSG_DELETE = 3
local SUBMSG_WRITE_DATA = 7
local SUBMSG_READ_DATA = 8
local FLAG_E_LITTLE_ENDIAN = 0x01
local CREATE_FLAG_REUSE = 0x02
local CREATE_FLAG_REPLACE = 0x04

-- ObjectId (Spec §7.2.1): a 16-bit word, `kind` in the low 4 bits and
-- `raw_id` in the high 12, written to the wire *big-endian* — the one
-- field in this whole client that is not little-endian (everything else
-- here, including `sequence_nr` and every length, is LE).
function M.packObjectId(rawId, kind)
  if math.type(rawId) ~= "integer" or rawId < 0 or rawId > 0x0FFF then
    error(("ObjectId raw_id %s does not fit 12 bits"):format(tostring(rawId)), 2)
  end
  if math.type(kind) ~= "integer" or kind < 0 or kind > 0x0F then
    error(("ObjectId kind %s does not fit 4 bits"):format(tostring(kind)), 2)
  end
  local word = (rawId << 4) | kind
  return string.pack(">I2", word)
end

-- An ObjectId already known as a raw 16-bit word (e.g. a well-known
-- singleton such as OBJECTID_CLIENT = 0xFFFE) — the big-endian pair
-- verbatim, with no kind/raw_id split.
function M.objectIdFromRaw(word)
  if math.type(word) ~= "integer" or word < 0 or word > 0xFFFF then
    error(("ObjectId raw word %s does not fit 16 bits"):format(tostring(word)), 2)
  end
  return string.pack(">I2", word)
end

-- MessageHeader (Spec §8.3.2): session_id, stream_id, sequence_nr (LE),
-- and client_key — present only when session_id <= 0x7F.
function M.encodeMessageHeader(sessionId, streamId, seqNr, clientKey)
  local hdr = string.pack("BB", sessionId, streamId) .. string.pack("<I2", seqNr)
  if sessionId <= 0x7F then
    if not clientKey or #clientKey ~= 4 then
      error("session_id <= 0x7F requires a 4-byte client_key", 2)
    end
    hdr = hdr .. clientKey
  end
  return hdr
end

-- SubmessageHeader (Spec §8.3.4): id, flags, and the body's byte length —
-- always little-endian regardless of the body's own byte order (the
-- E-flag governs the body only).
function M.encodeSubmessageHeader(id, flags, bodyLen)
  return string.pack("BB", id, flags) .. string.pack("<I2", bodyLen)
end

-- BaseObjectRequest (Spec §7.7.8): `request_id` passes through as the two
-- raw bytes the caller supplies — the spec gives it no numeric encoding
-- of its own, unlike `object_id`, which is explicitly big-endian —
-- followed by the (already-encoded, big-endian) object_id.
function M.encodeBaseObjectRequest(requestIdBytes, objectIdBytes)
  if #requestIdBytes ~= 2 then error("request_id is 2 bytes", 2) end
  if #objectIdBytes ~= 2 then error("object_id is 2 bytes", 2) end
  return requestIdBytes .. objectIdBytes
end

-- ObjectVariant::ByReference's *inner* payload only (Spec §7.7.2):
-- discriminator 0x01, three padding bytes to a 4-byte boundary (correct
-- only when this structure itself starts 4-aligned), then an XCDR2
-- string — a 4-byte LE length that includes the NUL terminator, the name
-- bytes, and the NUL terminator itself. Not the wire form a real agent
-- accepts on its own — see `M.encodeObjectVariantForKind`, whose 1-byte
-- outer ObjectKind shifts this padding to two bytes.
function M.encodeObjectVariantByReference(name)
  return string.char(0x01, 0, 0, 0) .. string.pack("<I4", #name + 1) .. name .. "\0"
end

-- The real wire ObjectVariant (Spec §7.7.3): a **two-level** discriminated
-- union — the outer ObjectKind byte (`kind`), then the inner
-- REPRESENTATION_BY_REFERENCE payload (disc 0x01, two padding bytes — the
-- outer kind byte already took the first of the four — then the same
-- XCDR2 string as `encodeObjectVariantByReference`), then that kind's own
-- trailing field: `parentIdBytes` (an already-encoded, big-endian
-- ObjectId, unaligned) for OBJK_TOPIC/OBJK_PUBLISHER/OBJK_SUBSCRIBER/
-- OBJK_DATAWRITER/OBJK_DATAREADER, `nil` for every other kind —
-- byte-oracled in `crates/xrce/src/object_repr.rs` against real eProsima
-- Micro-XRCE-DDS Client 3.0.1 CREATE traffic (OBJK_TOPIC's own
-- participant_id: commit 31c62df2a63). This — not
-- `encodeObjectVariantByReference` alone — is the only form a real agent
-- accepts.
function M.encodeObjectVariantForKind(kind, name, parentIdBytes)
  local inner = string.char(0x01, 0, 0) .. string.pack("<I4", #name + 1) .. name .. "\0"
  local variant = string.char(kind) .. inner
  if parentIdBytes then
    if #parentIdBytes ~= 2 then error("parent object_id is 2 bytes", 2) end
    variant = variant .. parentIdBytes
  end
  return variant
end

-- --- the five submessage encoders: header ++ submessage_header ++ body ----
--
-- Each of these sends exactly one submessage per message, so there is
-- never inter-submessage padding to add — that 4-byte-alignment rule
-- only applies *between* submessages in a multi-submessage message,
-- never after the last (or only) one.

-- eProsima's own hard datagram ceiling (`crates/xrce/src/handshake.rs`'s
-- module doc has the byte-oracle measurement backing it), used as this
-- port's own CREATE_CLIENT requested_mtu.
M.DEFAULT_MTU = 508

-- CLIENT_Representation (Annex A, Spec §7.8.2.1): xrce_cookie(4) ++
-- xrce_version(2) ++ xrce_vendor_id(2) ++ client_key(4) ++ session_id(1)
-- ++ a 0 properties-absent octet ++ requestedMtu(2, LE) — eProsima's own
-- non-normative MTU extension, always sent by this client. No
-- PropertySeq support here — this client sends none.
--
-- `opts` (optional): `representationTimestamps` "absent" (default, Annex A /
-- B.1 form) or "present" (the prose form of §7.8.2.1): after xrce_vendor_id
-- the `Time_t client_timestamp`, `opts.seconds` (i32) and `opts.nanoseconds`
-- (u32), little endian. Time_t is APPENDABLE (DDS-XTypes 1.3 §7.3.1.2.1.8)
-- with `opts.unannotatedTypes` "appendable" (default): DHEADER = 8 in front,
-- 12 octets; "final": no DHEADER, 8 octets. The agent has to be set the same
-- way (`representation_timestamps:` in its configuration).
function M.encodeClientRepresentation(version, vendorId, clientKey, sessionId, requestedMtu, opts)
  requestedMtu = requestedMtu or M.DEFAULT_MTU
  opts = opts or {}
  local ts = opts.representationTimestamps or "absent"
  local unann = opts.unannotatedTypes or "appendable"
  if ts ~= "absent" and ts ~= "present" then
    error("representationTimestamps must be 'absent' or 'present', got " .. tostring(ts), 2)
  end
  if unann ~= "appendable" and unann ~= "final" then
    error("unannotatedTypes must be 'appendable' or 'final', got " .. tostring(unann), 2)
  end
  local timeT = ""
  if ts == "present" then
    timeT = string.pack("<i4I4", opts.seconds or 0, opts.nanoseconds or 0)
    if unann == "appendable" then
      timeT = string.pack("<I4", 8) .. timeT -- DHEADER of Time_t
    end
  end
  return "XRCE" .. version .. vendorId .. timeT .. clientKey
    .. string.char(sessionId, 0) .. string.pack("<I2", requestedMtu)
end

-- CREATE_CLIENT (Spec §7.8.2, `XrceClient:connect`): `sessionId` is the
-- session_id the client asks for (it is also in the representation). No session
-- exists yet, so the MessageHeader carries SESSION_NONE_WITH_KEY (0x00, with
-- client_key) for a requested session_id of 0..127 and SESSION_NONE_WITHOUT_KEY
-- (0x80, no client_key) for 128..255 (DDS-XRCE 1.0 §8.3.2.1).
-- stream_id = NONE, sequence_nr = 0. The body is a real
-- CLIENT_Representation — build it with `M.encodeClientRepresentation` —
-- passed through verbatim here.
function M.encodeCreateClient(sessionId, clientKey, representation)
  local headerSession = sessionId <= 0x7F and M.SESSION_NONE_WITH_KEY or M.SESSION_NONE_WITHOUT_KEY
  local header = M.encodeMessageHeader(headerSession, M.STREAM_NONE, 0,
    headerSession == M.SESSION_NONE_WITH_KEY and clientKey or nil)
  local sub = M.encodeSubmessageHeader(SUBMSG_CREATE_CLIENT, FLAG_E_LITTLE_ENDIAN, #representation)
  return header .. sub .. representation
end

-- CREATE (Spec §7.8.3, `XrceClient:createObject`): BaseObjectRequest ++
-- the ObjectVariant encoding. Flags bit 1 = reuse, bit 2 = replace.
function M.encodeCreate(sessionId, streamId, seqNr, clientKey, requestIdBytes,
    objectIdBytes, variantBytes, reuse, replace)
  local flags = FLAG_E_LITTLE_ENDIAN
  if reuse then flags = flags | CREATE_FLAG_REUSE end
  if replace then flags = flags | CREATE_FLAG_REPLACE end
  local body = M.encodeBaseObjectRequest(requestIdBytes, objectIdBytes) .. variantBytes
  local header = M.encodeMessageHeader(sessionId, streamId, seqNr, clientKey)
  local sub = M.encodeSubmessageHeader(SUBMSG_CREATE, flags, #body)
  return header .. sub .. body
end

-- DELETE (Spec §7.8.3, `XrceClient:deleteObject`): BaseObjectRequest
-- only — no ObjectVariant.
function M.encodeDelete(sessionId, streamId, seqNr, clientKey, requestIdBytes, objectIdBytes)
  local body = M.encodeBaseObjectRequest(requestIdBytes, objectIdBytes)
  local header = M.encodeMessageHeader(sessionId, streamId, seqNr, clientKey)
  local sub = M.encodeSubmessageHeader(SUBMSG_DELETE, FLAG_E_LITTLE_ENDIAN, #body)
  return header .. sub .. body
end

-- WRITE_DATA (Spec §7.8.4, `XrceClient:requestWrite`): BaseObjectRequest
-- ++ serialized_data verbatim, with no length prefix of its own — the
-- sample is however many bytes are left in the submessage body, and
-- `submessage_length` is what bounds it (DDS-XRCE Annex B.3.1: 4 octets
-- of BaseObjectRequest + 2 octets of sample is submessageLength=6, not 10
-- — a `sequence<octet>` length would add four more).
function M.encodeWriteData(sessionId, streamId, seqNr, clientKey, requestIdBytes,
    objectIdBytes, serializedData)
  local body = M.encodeBaseObjectRequest(requestIdBytes, objectIdBytes) .. serializedData
  local header = M.encodeMessageHeader(sessionId, streamId, seqNr, clientKey)
  local sub = M.encodeSubmessageHeader(SUBMSG_WRITE_DATA, FLAG_E_LITTLE_ENDIAN, #body)
  return header .. sub .. body
end

-- READ_DATA (Spec §7.8.5, `XrceClient:requestRead`): BaseObjectRequest ++
-- ReadSpecification. This client never sets a content filter or delivery
-- control, so ReadSpecification is always these four trailing bytes:
-- preferred_stream_id, data_format = Data (0x00), no filter, no delivery
-- control.
function M.encodeReadData(sessionId, streamId, seqNr, clientKey, requestIdBytes,
    objectIdBytes, preferredStreamId)
  local readSpec = string.char(preferredStreamId, 0x00, 0x00, 0x00)
  local body = M.encodeBaseObjectRequest(requestIdBytes, objectIdBytes) .. readSpec
  local header = M.encodeMessageHeader(sessionId, streamId, seqNr, clientKey)
  local sub = M.encodeSubmessageHeader(SUBMSG_READ_DATA, FLAG_E_LITTLE_ENDIAN, #body)
  return header .. sub .. body
end

-- --- Layer 2: the state machine (mirrors crates/xrce-client 1:1) ----------

M.ClientState = {
  DISCONNECTED = "disconnected",
  CONNECTING = "connecting",
  CONNECTED = "connected",
}

local XrceClient = {}
XrceClient.__index = XrceClient
M.XrceClient = XrceClient

-- The Rust reference's `next_request_id.wrapping_add(1).max(1)`: wrap a
-- u16 and skip 0 — request_id 0 is never handed out once the counter has
-- started (it would otherwise land there once every 65536 calls).
local function nextRequestId(id)
  local nid = (id + 1) & 0xFFFF
  if nid == 0 then nid = 1 end
  return nid
end

-- `transport` is the SDK's existing `{ deliver = ..., receive = ... }`
-- shape from `zerodds.lua` (e.g. `z.memTransport()`) — no new transport
-- type is introduced here.
-- `opts` (optional): `representationTimestamps` ("absent" default | "present")
-- and `unannotatedTypes` ("appendable" default | "final") for
-- `XrceClient:clientRepresentation` / `XrceClient:connect()`.
function XrceClient.new(clientKey, transport, opts)
  if not clientKey or #clientKey ~= 4 then
    error("XrceClient.new takes a 4-byte client_key", 2)
  end
  return setmetatable({
    _clientKey = clientKey,
    _state = M.ClientState.DISCONNECTED,
    _transport = transport,
    _nextRequestId = 1,
    -- Defaults match the fixtures: session_id=0x01 (carries client_key; the
    -- CREATE_CLIENT header itself has session 0x00, §8.3.2.1),
    -- stream_id=0x80 (builtin-reliable — every post-handshake operation
    -- is reliable). next_seq starts at 0 — the CREATE_CLIENT handshake
    -- always sends sequence_nr=0 on stream_id=NONE and does not consume
    -- this counter; the first reliable-stream operation gets sequence_nr
    -- 0.
    _sessionId = 0x01,
    _streamId = M.STREAM_BUILTIN_RELIABLE,
    _nextSeq = 0,
    _representationTimestamps = (opts and opts.representationTimestamps) or "absent",
    _unannotatedTypes = (opts and opts.unannotatedTypes) or "appendable",
  }, XrceClient)
end

function XrceClient:state() return self._state end
function XrceClient:clientKey() return self._clientKey end
function XrceClient:representationTimestamps() return self._representationTimestamps end

-- The CLIENT_Representation of this client with its representationTimestamps
-- option. The Time_t is the wall clock at the time of the call.
function XrceClient:clientRepresentation(version, vendorId, requestedMtu)
  return M.encodeClientRepresentation(version or "\x01\x00", vendorId or "\x00\x00",
    self._clientKey, self._sessionId, requestedMtu, {
      representationTimestamps = self._representationTimestamps,
      seconds = os.time(),
      nanoseconds = 0,
      unannotatedTypes = self._unannotatedTypes,
    })
end

function XrceClient:requireConnected()
  if self._state ~= M.ClientState.CONNECTED then
    -- `error(msg, 2)` — not nil/Result — following this SDK's own
    -- convention (see `M.writeFrame` above) rather than Rust's
    -- `Result<_, ClientError::InvalidState>`.
    error("XrceClient: invalid state (InvalidState — not connected)", 2)
  end
end

-- Initiates the handshake (Spec §8.4.5): encodes and delivers
-- CREATE_CLIENT, then moves to Connecting. Without `representation` the
-- body is `self:clientRepresentation()`.
function XrceClient:connect(representation)
  representation = representation or self:clientRepresentation()
  if self._state ~= M.ClientState.DISCONNECTED then
    error("XrceClient:connect: invalid state (InvalidState — already connecting/connected)", 2)
  end
  local frame = M.encodeCreateClient(self._sessionId, self._clientKey, representation)
  self._transport.deliver(frame)
  self._state = M.ClientState.CONNECTING
end

-- Marks the handshake complete — the caller has externally received
-- STATUS_AGENT. Pure bookkeeping, no wire I/O: matches
-- `crates/xrce-client::mark_connected` exactly.
function XrceClient:markConnected()
  if self._state ~= M.ClientState.CONNECTING then
    error("XrceClient:markConnected: invalid state (InvalidState — not connecting)", 2)
  end
  self._state = M.ClientState.CONNECTED
end

-- Submits a CREATE operation (Spec §7.8.3). Returns the request_id.
function XrceClient:createObject(objectId, variantBytes, reuse, replace)
  self:requireConnected()
  local req = self._nextRequestId
  self._nextRequestId = nextRequestId(req)
  local frame = M.encodeCreate(self._sessionId, self._streamId, self._nextSeq, self._clientKey,
    string.pack(">I2", req), objectId, variantBytes, reuse or false, replace or false)
  self._transport.deliver(frame)
  self._nextSeq = (self._nextSeq + 1) & 0xFFFF
  return req
end

-- Submits a DELETE operation. Returns the request_id.
function XrceClient:deleteObject(objectId)
  self:requireConnected()
  local req = self._nextRequestId
  self._nextRequestId = nextRequestId(req)
  local frame = M.encodeDelete(self._sessionId, self._streamId, self._nextSeq, self._clientKey,
    string.pack(">I2", req), objectId)
  self._transport.deliver(frame)
  self._nextSeq = (self._nextSeq + 1) & 0xFFFF
  return req
end

-- Submits a WRITE_DATA operation. Returns the request_id.
function XrceClient:requestWrite(writerObjectId, payload)
  self:requireConnected()
  local req = self._nextRequestId
  self._nextRequestId = nextRequestId(req)
  local frame = M.encodeWriteData(self._sessionId, self._streamId, self._nextSeq, self._clientKey,
    string.pack(">I2", req), writerObjectId, payload)
  self._transport.deliver(frame)
  self._nextSeq = (self._nextSeq + 1) & 0xFFFF
  return req
end

-- Submits a READ_DATA (pull-based) operation. Returns the request_id.
function XrceClient:requestRead(readerObjectId)
  self:requireConnected()
  local req = self._nextRequestId
  self._nextRequestId = nextRequestId(req)
  local frame = M.encodeReadData(self._sessionId, self._streamId, self._nextSeq, self._clientKey,
    string.pack(">I2", req), readerObjectId, self._streamId)
  self._transport.deliver(frame)
  self._nextSeq = (self._nextSeq + 1) & 0xFFFF
  return req
end

-- Closes the session — goes back to Disconnected. That is all: matches
-- `crates/xrce-client::disconnect` exactly, which resets only `state`
-- (its `out_queue.clear()` has no counterpart here, since this port
-- delivers each frame immediately rather than queuing it).
-- `next_request_id`, `session_id`, and `next_seq` are deliberately left
-- alone — a literal re-implementation "helpfully" resetting more than
-- that would be wrong.
function XrceClient:disconnect()
  self._state = M.ClientState.DISCONNECTED
end

return M
