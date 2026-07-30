-- SPDX-License-Identifier: Apache-2.0
-- Copyright 2026 ZeroDDS Contributors
--
-- Tests for the native Lua endpoint: byte-identity vs the Rust goldens, plus
-- sync + async loopback. Run with `lua5.4 test.lua` (GOLDEN_DIR=build).

package.path = "./?.lua;" .. package.path
local z = require("zerodds")

local goldenDir = os.getenv("GOLDEN_DIR") or "build"

local function fixture(endian)
  local w = z.Writer.new(endian)
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
  local w = z.Writer.new(z.LE)
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
local c = z.Client.new(t)
for i = 0, 4 do c:write(sampleBody(0x3000 + i)) end
for i = 0, 4 do
  local body = c:poll()
  assert(body ~= nil, "sync: no sample")
  assert(z.Reader.new(body, z.LE):getU32() == 0x3000 + i, "sync: id mismatch")
end
print("sync loopback: 5 samples OK")

-- async loopback (coroutine producer)
local t2 = z.memTransport()
local w = z.Client.new(t2)
for i = 0, 4 do w:write(sampleBody(0x1000 + i)) end
local reader = z.asyncReader(t2)
for i = 0, 4 do
  local body
  repeat body = reader() until body ~= nil
  assert(z.Reader.new(body, z.LE):getU32() == 0x1000 + i, "async: id mismatch")
end
print("async loopback: 5 samples OK")

-- negative frame vectors: length bounding + malformed reject
local first = z.writeFrame(0x80, 0x01, 1, "\xAA\xBB\xCC")
local second = z.writeFrame(0x80, 0x01, 2, "\xDD\xEE")
assert(z.readFrame(first .. second) == "\xAA\xBB\xCC",
  "appended submessage must not leak into sample")
local overlong = first:sub(1, 6) .. "\xFF\xFF" .. first:sub(9)
assert(z.readFrame(overlong) == nil, "over-long length must reject")
assert(z.readFrame("\x80\x01\x00\x00\x07") == nil, "truncated header must reject")
assert(z.writeFrame(0x80, 0x01, 1, string.rep("x", 0x10000)) == nil,
  "sample > 0xFFFF must be refused")
print("negative frame vectors: OK")

print("ALL OK")
