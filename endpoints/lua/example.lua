-- SPDX-License-Identifier: Apache-2.0
-- Copyright 2026 ZeroDDS Contributors
--
-- Runnable example for the native Lua endpoint: sync (poll) and async
-- (coroutine producer). Run with `lua5.4 example.lua`.

package.path = "./?.lua;" .. package.path
local z = require("zerodds")

local function sample(id, label)
  local w = z.Writer.new(z.LE)
  w:putU32(id)
  w:putString(label)
  return w:bytes()
end

-- sync
local t = z.memTransport()
local c = z.Client.new(t)
c:write(sample(0x42, "sync-hello"))
local body = c:poll()
if body then
  print(string.format("sync: received id=0x%x", z.Reader.new(body, z.LE):getU32()))
end

-- async
local t2 = z.memTransport()
local w = z.Client.new(t2)
for i = 0, 2 do w:write(sample(0x100 + i, "async")) end
local reader = z.asyncReader(t2)
for _ = 0, 2 do
  local b
  repeat b = reader() until b ~= nil
  print(string.format("async: received id=0x%x", z.Reader.new(b, z.LE):getU32()))
end

print("ALL OK")
