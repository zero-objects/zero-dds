-- SPDX-License-Identifier: Apache-2.0
-- Copyright 2026 ZeroDDS Contributors
--
-- Deep example (sync): a realistic sensor-telemetry flow. A publisher frames
-- five typed `Reading { id, value, label }` samples and delivers them; the
-- subscriber owns the run-loop and polls, decoding EVERY field byte-for-byte.

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
  c:write(marshal({ id = 0x1000 + i, value = 20.0 + i * 0.5, label = string.format("bay-%02d", i) }, z.LE))
end

local got = 0
while got < total do
  local body = c:poll()
  if body == nil then break end
  local r = decode(body)
  print(string.format('sync reading %d: id=0x%x value=%.1f label="%s"', got, r.id, r.value, r.label))
  got = got + 1
end

if got ~= total then
  io.stderr:write(string.format("incomplete: got %d of %d\n", got, total))
  os.exit(1)
end
print("ALL OK")
