-- SPDX-License-Identifier: Apache-2.0
-- Copyright 2026 ZeroDDS Contributors
--
-- Native pure-Lua endpoint SDK (ADR 0013): a from-scratch XCDR wire-core built
-- on string.pack/string.unpack (Lua 5.3+), byte-identical to the Rust core and
-- the other SDKs — no C FFI needed. Sync (poll) and async (coroutines, the
-- idiomatic Lua concurrency model).

local M = {}

-- endian is the string.pack prefix: "<" little-endian, ">" big-endian.
M.LE = "<"
M.BE = ">"

-- --- Writer: string pieces + endian; alignment relative to buffer start, cap 4 ---

local Writer = {}
Writer.__index = Writer
M.Writer = Writer

function Writer.new(endian)
  return setmetatable({ parts = {}, len = 0, endian = endian }, Writer)
end

function Writer:raw(s)
  self.parts[#self.parts + 1] = s
  self.len = self.len + #s
end

function Writer:align(a)
  local cap = a < 4 and a or 4
  local pad = (cap - (self.len % cap)) % cap
  if pad > 0 then self:raw(string.rep("\0", pad)) end
end

function Writer:putU8(v) self:raw(string.pack("B", v & 0xff)) end
function Writer:putU16(v) self:align(2); self:raw(string.pack(self.endian .. "I2", v)) end
function Writer:putU32(v) self:align(4); self:raw(string.pack(self.endian .. "I4", v)) end
function Writer:putU64(v) self:align(4); self:raw(string.pack(self.endian .. "I8", v)) end
function Writer:putF32(v) self:align(4); self:raw(string.pack(self.endian .. "f", v)) end

function Writer:putString(s)
  self:putU32(#s + 1)
  self:raw(s)
  self:putU8(0)
end

function Writer:putSeqU8(b) -- b: a string of bytes
  self:putU32(#b)
  self:raw(b)
end

function Writer:bytes() return table.concat(self.parts) end

-- --- Reader: a cursor over a byte string (pos is 0-based) ---

local Reader = {}
Reader.__index = Reader
M.Reader = Reader

function Reader.new(data, endian)
  return setmetatable({ data = data, pos = 0, endian = endian }, Reader)
end

function Reader:align(a)
  local cap = a < 4 and a or 4
  local pad = (cap - (self.pos % cap)) % cap
  self.pos = self.pos + pad
end

function Reader:getU32()
  self:align(4)
  local v, nextpos = string.unpack(self.endian .. "I4", self.data, self.pos + 1)
  self.pos = nextpos - 1
  return v
end

-- --- primitives beyond getU32 — the byte-exact inverse of the Writer ---

function Reader:getU8()
  local v = self.data:byte(self.pos + 1)
  self.pos = self.pos + 1
  return v
end

function Reader:getU16()
  self:align(2)
  local v, nextpos = string.unpack(self.endian .. "I2", self.data, self.pos + 1)
  self.pos = nextpos - 1
  return v
end

function Reader:getU64()
  self:align(4)
  local v, nextpos = string.unpack(self.endian .. "I8", self.data, self.pos + 1)
  self.pos = nextpos - 1
  return v
end

function Reader:getF32()
  self:align(4)
  local v, nextpos = string.unpack(self.endian .. "f", self.data, self.pos + 1)
  self.pos = nextpos - 1
  return v
end

function Reader:getString()
  local n = self:getU32()
  local s = self.data:sub(self.pos + 1, self.pos + n - 1)
  self.pos = self.pos + n
  return s
end

function Reader:getSeqU8()
  local n = self:getU32()
  local b = self.data:sub(self.pos + 1, self.pos + n)
  self.pos = self.pos + n
  return b
end

-- --- XRCE WRITE_DATA framing ---

M.SESSION_NOKEY = 0x80
M.STREAM_BEST_EFFORT = 0x01

function M.writeFrame(session, stream, seqNo, sample)
  local hdr = string.pack("BB", session, stream)
    .. string.pack("<I2", seqNo)
    .. string.pack("BB", 0x07, 0x03)
    .. string.pack("<I2", #sample)
  return hdr .. sample
end

function M.readFrame(frame)
  if #frame >= 8 and frame:byte(5) == 0x07 then
    return frame:sub(9)
  end
  return nil
end

-- --- sync client ---

local Client = {}
Client.__index = Client
M.Client = Client

function Client.new(transport)
  return setmetatable(
    { transport = transport, session = M.SESSION_NOKEY, stream = M.STREAM_BEST_EFFORT, seq = 1 },
    Client)
end

function Client:write(sample)
  local frame = M.writeFrame(self.session, self.stream, self.seq, sample)
  self.transport.deliver(frame)
  self.seq = (self.seq + 1) & 0xffff
end

-- One non-blocking receive: returns the sample body, or nil.
function Client:poll()
  local frame = self.transport.receive()
  if frame == nil then return nil end
  return M.readFrame(frame)
end

-- --- async reader: a coroutine producer (idiomatic Lua) ---
--
-- Resume it to get the next decoded sample; it yields nil when the transport is
-- momentarily empty, so a cooperative consumer can retry or do other work.
function M.asyncReader(transport)
  return coroutine.wrap(function()
    while true do
      local frame = transport.receive()
      if frame then
        local body = M.readFrame(frame)
        if body then coroutine.yield(body) end
      else
        coroutine.yield(nil)
      end
    end
  end)
end

-- In-memory FIFO transport for tests + example.
function M.memTransport()
  local q, head = {}, 1
  return {
    deliver = function(frame) q[#q + 1] = frame end,
    receive = function()
      if head <= #q then
        local f = q[head]
        q[head] = nil
        head = head + 1
        return f
      end
      return nil
    end,
  }
end

return M
