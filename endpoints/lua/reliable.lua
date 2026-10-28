-- SPDX-License-Identifier: Apache-2.0
-- Copyright 2026 ZeroDDS Contributors
--
-- Reliable XRCE stream (spec §8.4.10/§8.4.11) for the Lua endpoint SDK.
-- Self-contained: does NOT `require("zerodds")` -- that module's wire
-- helpers (Writer/Reader) are chunk-locals, not reachable from here -- so
-- this module carries its own minimal frame codec, byte-identical to
-- `crates/xrce` and the other endpoint SDKs (mirrors `endpoints/nim/reliable.nim`
-- / `endpoints/d/reliable.d`).
--
-- Frame layout (little-endian, 8-byte header + body):
--   [0]=session [1]=stream [2..4)=seq u16 LE [4]=submessage id [5]=flags
--   [6..8)=body-len u16 LE [8..]=body
--   WRITE_DATA id=0x07 flags=0x03 body=sample; header seq = sample seq (RFC-1982)
--   ACKNACK   id=0x0A flags=0x01 body(9)= DHEADER u32 LE (=5), first_unacked i16 LE,
--             nack[0], nack[1], stream. ACKNACK_Payload has no @extensibility, so it is
--             APPENDABLE (DDS-XTypes 1.3 section 7.3.1.2.1.8) and carries a DHEADER in
--             XCDR2 (unannotatedTypes = "appendable", the default). With
--             unannotatedTypes = "final" the body is the 5 octets of the FINAL form of
--             the reference implementation (eProsima), length field 5, 13-octet frame.
--             parseAcknack accepts both forms in either setting.
--   HEARTBEAT id=0x0B flags=0x01 body(5)= first i16 LE, last i16 LE, stream
--
-- The state machine (Sender/Receiver) mirrors `crates/xrce/src/reliable.rs`
-- 1:1: same window caps, same RFC-1982 wraparound arithmetic, same
-- ack/retransmit rules.

local M = {}

M.SESSION_NOKEY = 0x80
M.STREAM_NONE = 0x00
M.RELIABLE_STREAM_ID = 0x80
M.HEARTBEAT_PERIOD_MS = 500
M.SENDER_WINDOW = 16
M.RECEIVER_BUFFER = 64
M.MAX_PAYLOAD = 65535

local SM_WRITE_DATA = 0x07
local SM_ACKNACK = 0x0A
local SM_HEARTBEAT = 0x0B
-- E flag (little-endian body) + DataFormat FORMAT_DATA (0b000) in bits 1..3 --
-- DDS-XRCE 1.0 8.3.5.8.
local FLAG_WRITE = 0x01
-- Wire size of the BaseObjectRequest that precedes the sample (7.7.8).
M.BASE_SIZE = 4
local FLAG_E_LE = 0x01

-- --- RFC-1982 16-bit serial-number comparison (half-window = 32768) ---

local function seqLt(a, b)
  local d = (b - a) & 0xffff
  return d ~= 0 and d < 0x8000
end
M.seqLt = seqLt

local function seqGt(a, b) return seqLt(b, a) end
M.seqGt = seqGt

-- --- frame codec ---

-- Reliable WRITE_DATA -- identical bytes to the best-effort `writeFrame`
-- with stream = RELIABLE_STREAM_ID: the submessage body is the
-- BaseObjectRequest (7.7.8: request_id then object_id, each two octets most
-- significant first) followed by the sample. `objectId` names the target
-- DataWriter; the request_id is the sample's own sequence number.
function M.writeDataFrame(seqNo, objectId, sample)
  return string.pack("<BBI2BBI2", M.SESSION_NOKEY, M.RELIABLE_STREAM_ID, seqNo,
    SM_WRITE_DATA, FLAG_WRITE, M.BASE_SIZE + #sample)
    .. string.pack(">I2I2", seqNo, objectId) .. sample
end

-- byte-identical to `golden_heartbeat_le.bin` for (0x80, 0x00, 1, {1,3,0x80}).
function M.heartbeatFrame(session, streamHdr, msgSeq, first, last, streamId)
  return string.pack("<BBI2BBI2", session, streamHdr, msgSeq, SM_HEARTBEAT, FLAG_E_LE, 5)
    .. string.pack("<I2I2B", first, last, streamId)
end

-- How the types of DDS-XRCE 1.0 Annex A without @extensibility are written
-- (DDS-XTypes 1.3 section 7.3.1.2.1.8). This module writes one such type,
-- ACKNACK_Payload; HEARTBEAT_Payload is FINAL in Annex A and does not change.
M.UNANNOTATED_APPENDABLE = "appendable" -- with a DHEADER (default)
M.UNANNOTATED_FINAL = "final" -- without a DHEADER (reference implementation)

local function checkUnannotatedTypes(mode)
  if mode == nil then return M.UNANNOTATED_APPENDABLE end
  if mode ~= M.UNANNOTATED_APPENDABLE and mode ~= M.UNANNOTATED_FINAL then
    error("unannotatedTypes must be 'appendable' or 'final', got " .. tostring(mode), 3)
  end
  return mode
end

-- `unannotatedTypes` (optional, "appendable" by default): "appendable" is the
-- 17-octet frame, byte-identical to `golden_acknack_le.bin` for
-- (0x80, 0x00, 1, {1,0,0,0x80}); "final" is the 13-octet frame (length field 5).
function M.acknackFrame(session, streamHdr, msgSeq, firstUnacked, nackLo, nackHi, streamId, unannotatedTypes)
  if checkUnannotatedTypes(unannotatedTypes) == M.UNANNOTATED_FINAL then
    return string.pack("<BBI2BBI2", session, streamHdr, msgSeq, SM_ACKNACK, FLAG_E_LE, 5)
      .. string.pack("<I2BBB", firstUnacked, nackLo, nackHi, streamId)
  end
  return string.pack("<BBI2BBI2", session, streamHdr, msgSeq, SM_ACKNACK, FLAG_E_LE, 9)
    .. string.pack("<I4I2BBB", 5, firstUnacked, nackLo, nackHi, streamId)
end

-- Returns seq, sample or nil if `frame` is not a WRITE_DATA frame. The sample
-- starts after the BaseObjectRequest; a body too short to hold one is refused
-- rather than handed back truncated.
function M.parseWriteData(frame)
  if #frame < 8 + M.BASE_SIZE or frame:byte(5) ~= SM_WRITE_DATA then return nil end
  local smLen = string.unpack("<I2", frame, 7)
  if smLen < M.BASE_SIZE or 8 + smLen > #frame then return nil end
  local seq = string.unpack("<I2", frame, 3)
  return seq, frame:sub(9 + M.BASE_SIZE, 8 + smLen)
end

-- Returns {first, last, streamId} or nil.
function M.parseHeartbeat(frame)
  if #frame < 13 or frame:byte(5) ~= SM_HEARTBEAT then return nil end
  local first, last, streamId = string.unpack("<I2I2B", frame, 9)
  return { first = first, last = last, streamId = streamId }
end

-- Returns {firstUnacked, nackLo, nackHi, streamId} or nil. Accepts body length
-- 9 (DHEADER + 5 octets) and the FINAL form with body length 5.
function M.parseAcknack(frame)
  if #frame < 13 or frame:byte(5) ~= SM_ACKNACK then return nil end
  local pos = 9
  if string.unpack("<I2", frame, 7) >= 9 then
    if #frame < 17 then return nil end
    pos = 13 -- skip the 4-octet DHEADER
  end
  local firstUnacked, nackLo, nackHi, streamId = string.unpack("<I2BBB", frame, pos)
  return { firstUnacked = firstUnacked, nackLo = nackLo, nackHi = nackHi, streamId = streamId }
end

-- =====================================================================
-- Sender
-- =====================================================================

local Sender = {}
Sender.__index = Sender
M.Sender = Sender

function Sender.new()
  return setmetatable({
    nextSeq = 0,
    inFlight = {}, -- seq (number) -> payload (string)
    lastHeartbeatMs = nil, -- nil = never sent
  }, Sender)
end

function Sender:inFlightCount()
  local n = 0
  for _ in pairs(self.inFlight) do n = n + 1 end
  return n
end

-- Buffers a new sample. Returns `seq` on success, or `nil, "too_large"` /
-- `nil, "window_full"` on error -- the caller must process ACKNACKs first
-- when the window is full (16 in-flight, matches the 16-bit nack bitmap).
function Sender:submit(payload)
  if #payload > M.MAX_PAYLOAD then return nil, "too_large" end
  if self:inFlightCount() >= M.SENDER_WINDOW then return nil, "window_full" end
  local seq = self.nextSeq
  self.inFlight[seq] = payload
  self.nextSeq = (self.nextSeq + 1) & 0xffff
  return seq
end

function Sender:getInFlight(seq)
  return self.inFlight[seq]
end

-- Iterates (seq, payload) pairs currently in-flight (e.g. for retransmit).
function Sender:inFlightPairs()
  return pairs(self.inFlight)
end

-- `some(HEARTBEAT)` when in-flight samples exist and the period elapsed
-- (fires on the first call, then every HEARTBEAT_PERIOD_MS). first = RFC-1982
-- window base (oldest unacked), last = RFC-1982 newest unacked -- computed via
-- seqLt/seqGt, NOT numeric min/max, which is wrong across a 16-bit wrap: window
-- 0xFFFE,0xFFFF,0x0000,0x0001 -> base 0xFFFE / end 0x0001, not 0x0000 / 0xFFFF.
-- Mirrors window_base / serial_max_in_flight in crates/xrce/src/reliable.rs.
function Sender:pendingHeartbeat(nowMs)
  local first, last
  for k in pairs(self.inFlight) do
    if first == nil then
      first = k; last = k
    else
      if seqLt(k, first) then first = k end
      if seqGt(k, last) then last = k end
    end
  end
  if first == nil then return nil end
  local due = self.lastHeartbeatMs == nil or (nowMs - self.lastHeartbeatMs) >= M.HEARTBEAT_PERIOD_MS
  if not due then return nil end
  self.lastHeartbeatMs = nowMs
  return { first = first, last = last, streamId = M.RELIABLE_STREAM_ID }
end

-- `base = firstUnacked`; everything strictly before it (RFC-1982) is
-- acknowledged and dropped; in [base, base+16) a set bit means missing
-- (keep), a clear bit means acked (drop).
function Sender:recvAcknack(firstUnacked, nackLo, nackHi)
  local base = firstUnacked
  local bitmap = nackLo | (nackHi << 8)
  local toRemove = {}
  for k in pairs(self.inFlight) do
    local diff = (base - k) & 0xffff
    if diff ~= 0 and diff < 0x8000 then
      toRemove[#toRemove + 1] = k
    end
  end
  for _, k in ipairs(toRemove) do self.inFlight[k] = nil end
  for i = 0, 15 do
    local seq = (base + i) & 0xffff
    local bit = (bitmap >> i) & 1
    if bit == 0 then self.inFlight[seq] = nil end
  end
end

function Sender:reset()
  self.nextSeq = 0
  self.inFlight = {}
  self.lastHeartbeatMs = nil
end

-- =====================================================================
-- Receiver
-- =====================================================================

local Receiver = {}
Receiver.__index = Receiver
M.Receiver = Receiver

-- `opts.unannotatedTypes`: "appendable" (default) or "final", the form of
-- the ACKNACK frames Receiver:acknackFrame writes.
function Receiver.new(opts)
  return setmetatable({
    expected = 0,
    received = {},
    unannotatedTypes = checkUnannotatedTypes(opts and opts.unannotatedTypes),
  }, Receiver)
end

-- The ACKNACK frame for pendingAcknack(hintLastSeen), in the form of this
-- receiver's unannotatedTypes. session/streamHdr/msgSeq default to
-- SESSION_NOKEY / STREAM_NONE / 1.
function Receiver:acknackFrame(hintLastSeen, session, streamHdr, msgSeq)
  local a = self:pendingAcknack(hintLastSeen)
  return M.acknackFrame(session or M.SESSION_NOKEY, streamHdr or M.STREAM_NONE, msgSeq or 1,
    a.firstUnacked, a.nackLo, a.nackHi, a.streamId, self.unannotatedTypes)
end

function Receiver:outOfOrderCount()
  local n = 0
  for _ in pairs(self.received) do n = n + 1 end
  return n
end

-- Buffers an incoming sample (or silently accepts a duplicate as a no-op).
-- Returns `false` only when the receiver buffer is full (DoS bound).
function Receiver:recvData(seq, payload)
  if seqLt(seq, self.expected) then return true end -- already delivered -> drop
  if self.received[seq] ~= nil then return true end -- already buffered
  if self:outOfOrderCount() >= M.RECEIVER_BUFFER then return false end
  self.received[seq] = payload
  return true
end

-- Delivers contiguously from `expected`, advancing it. Returns an array of
-- {seq=, payload=}.
function Receiver:drainInOrder()
  local out = {}
  while self.received[self.expected] ~= nil do
    out[#out + 1] = { seq = self.expected, payload = self.received[self.expected] }
    self.received[self.expected] = nil
    self.expected = (self.expected + 1) & 0xffff
  end
  return out
end

-- Bitmap of the missing slots in [expected, expected+16). `hintLastSeen`
-- (or nil): slots beyond it are treated as not-missing (a HEARTBEAT hint
-- narrows what the sender could possibly have sent so far).
function Receiver:pendingAcknack(hintLastSeen)
  local base = self.expected
  local bitmap = 0
  for i = 0, 15 do
    local seq = (base + i) & 0xffff
    if hintLastSeen ~= nil and seqGt(seq, hintLastSeen) then
      -- beyond the hint: not (yet) missing
    elseif self.received[seq] == nil then
      bitmap = bitmap | (1 << i)
    end
  end
  return {
    firstUnacked = base,
    nackLo = bitmap & 0xff,
    nackHi = (bitmap >> 8) & 0xff,
    streamId = M.RELIABLE_STREAM_ID,
  }
end

function Receiver:reset()
  self.expected = 0
  self.received = {}
end

-- =====================================================================
-- Async-decoupled writer (cooperative -- stock lua5.4 has no OS threads)
-- =====================================================================
--
-- The producer's hot path is `push`: a plain table insert, no socket I/O.
-- `drain` performs the real work (Sender:submit + the caller's `sendFn`,
-- typically a UDP send). Both are invoked from the SAME call stack / OS
-- thread by the caller's event loop (see `reliable_app.lua`) -- this is the
-- "single-process interleaved submit/drain" option: push is cheaper than an
-- inline sendto (no syscall on that call), but nothing drains the queue
-- concurrently while the producer keeps running. On stock Lua 5.4 (no
-- native threads) that would require a coroutine yield/resume dance across
-- the same thread anyway, which is still cooperative, not real overlap --
-- so this simpler, easier-to-reason-about-under-loss-recovery form was
-- chosen instead of dressing the same cooperative behavior up in
-- coroutine.wrap. Report this honestly: it is NOT a wait-free ring, and the
-- producer still ultimately drives the socket, just via a cheaper call on
-- its own path.

local AsyncWriter = {}
AsyncWriter.__index = AsyncWriter
M.AsyncWriter = AsyncWriter

-- `sender`: a Sender. `sendFn(frame)`: called with an encoded WRITE_DATA
-- frame whenever `drain()` actually submits + sends one.
-- `objectId` names the target DataWriter in every frame's BaseObjectRequest
-- (7.7.8).
function AsyncWriter.new(sender, sendFn, objectId)
  return setmetatable(
    { sender = sender, sendFn = sendFn, objectId = objectId, queue = {}, head = 1, tail = 0 },
    AsyncWriter)
end

function AsyncWriter:pending()
  return self.tail - self.head + 1
end

-- Producer side: enqueue only. Returns false if the queue is at its cap
-- (bounded like the sender window) -- the caller should `drain()` to make
-- room.
function AsyncWriter:push(payload)
  if self:pending() >= M.SENDER_WINDOW then return false end
  self.tail = self.tail + 1
  self.queue[self.tail] = payload
  return true
end

-- Drain step: submits + sends everything queued so far, up to the sender's
-- window. Returns the number of frames sent.
function AsyncWriter:drain()
  local n = 0
  while self.head <= self.tail do
    local payload = self.queue[self.head]
    local seq = self.sender:submit(payload)
    if not seq then break end -- window full; retry on a later drain() call
    self.queue[self.head] = nil
    self.head = self.head + 1
    self.sendFn(M.writeDataFrame(seq, self.objectId, payload))
    n = n + 1
  end
  return n
end

function AsyncWriter:isEmpty()
  return self.head > self.tail
end

return M
