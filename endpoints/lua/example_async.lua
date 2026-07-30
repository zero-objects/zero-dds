-- SPDX-License-Identifier: Apache-2.0
-- Copyright 2026 ZeroDDS Contributors
--
-- Deep example (async): the same sensor-telemetry flow, but the subscriber does
-- not drive a plain poll loop. `asyncReader` is a coroutine producer; resuming
-- it yields the next decoded sample (or nil when momentarily empty) — the
-- idiomatic Lua concurrency model. Every field is decoded.

package.path = "./?.lua;" .. package.path
local z = require("zerodds")

-- Reading { id: uint32, value: float, label: string }

local function marshal(r, endian)
  local w = z.Writer.new(endian)
  w:putU32(r.id)
  w:putF32(r.value)
  w:putString(r.label)
  return w:bytes()
end

local function decode(body)
  local r = z.Reader.new(body, z.LE)
  return { id = r:getU32(), value = r:getF32(), label = r:getString() }
end

local total = 5
local t = z.memTransport()
local c = z.Client.new(t)
for i = 0, total - 1 do
  c:write(marshal({ id = 0x2000 + i, value = 100.0 - i, label = string.format("sensor-%02d", i) }, z.LE))
end

local reader = z.asyncReader(t)
for got = 0, total - 1 do
  local b
  repeat b = reader() until b ~= nil
  local r = decode(b)
  print(string.format('async reading %d: id=0x%x value=%.1f label="%s"', got, r.id, r.value, r.label))
end

print("ALL OK")
