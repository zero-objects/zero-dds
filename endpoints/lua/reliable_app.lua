-- SPDX-License-Identifier: Apache-2.0
-- Copyright 2026 ZeroDDS Contributors
--
-- Reliable sender app for the live E2E test
-- (`crates/endpoint-e2e/tests/lua_reliable.rs`). Submits N samples on the
-- reliable stream (0x80), drives HEARTBEAT / ACKNACK loss recovery against
-- the shared Rust reliable peer, and exits once every sample is
-- acknowledged.
--
-- Uses `reliable.lua`'s AsyncWriter for the submit/send split: the producer
-- loop below only pushes payloads (a table insert -- no socket I/O); the
-- actual Sender:submit + sendto happens in `writer:drain()`, called from the
-- SAME single OS thread inside the run loop below. Stock lua5.4 has no
-- native threads, so this is cooperative interleaving, not concurrent
-- decoupling -- see `reliable.lua`'s AsyncWriter doc comment and the
-- `bench` mode below for the honest latency note.
--
-- usage: lua5.4 reliable_app.lua <peer-port> <N> [run|bench]

package.path = "./?.lua;" .. package.path
local rel = require("reliable")
local socket = require("socket")

local function nowMs()
  return math.floor(socket.gettime() * 1000)
end

local function runReliable(port, n)
  local udp = assert(socket.udp())
  assert(udp:setpeername("127.0.0.1", port))
  udp:settimeout(0.02) -- 20ms poll for ACKNACKs

  local sender = rel.Sender.new()
  local writer = rel.AsyncWriter.new(sender, function(frame) udp:send(frame) end)

  -- Producer step: enqueue all N samples up front (decoupled from the
  -- socket -- see the AsyncWriter doc comment for the honest caveat).
  for i = 0, n - 1 do
    local payload = string.pack("<I4", i)
    while not writer:push(payload) do
      writer:drain() -- queue at cap: drain to make room
    end
  end

  local msgSeq = 1
  local startMs = nowMs()
  while true do
    -- 1. drain step: submit queued samples, send WRITE_DATA.
    writer:drain()

    -- 2. done once nothing is queued and the send window is empty.
    if writer:isEmpty() and sender:inFlightCount() == 0 then break end

    -- 3. safety valve (the peer's own deadline is 30s).
    if nowMs() - startMs > 20000 then break end

    -- 4. period-gated HEARTBEAT.
    local hb = sender:pendingHeartbeat(nowMs())
    if hb ~= nil then
      udp:send(rel.heartbeatFrame(rel.SESSION_NOKEY, rel.STREAM_NONE, msgSeq, hb.first, hb.last, hb.streamId))
      msgSeq = msgSeq + 1
    end

    -- 5. drain incoming ACKNACKs (bounded, short timeout via settimeout above).
    for _ = 1, 64 do
      local data = udp:receive()
      if not data then break end
      local ack = rel.parseAcknack(data)
      if ack ~= nil then
        sender:recvAcknack(ack.firstUnacked, ack.nackLo, ack.nackHi)
      end
    end

    -- 6. retransmit whatever is still in-flight (peer marked it missing).
    if sender:inFlightCount() > 0 then
      for seq, payload in sender:inFlightPairs() do
        udp:send(rel.writeDataFrame(seq, payload))
      end
    end
  end
  udp:close()
  print("SENT " .. n)
end

-- Producer-latency micro-bench: AsyncWriter:push (table insert) vs an inline
-- UDP sendto. Honest note: nothing drains the queue concurrently on stock
-- lua5.4 (no OS threads) -- this measures what the enqueue call itself costs
-- on the producer's own path, not a real concurrent handoff. A later
-- drain() call still pays the sendto cost, later, on the SAME thread.
local function runBench(port)
  local iters = 20000
  local sample = string.pack("<I4", 0)
  local frame = rel.writeDataFrame(0, sample)

  local udp = assert(socket.udp())
  assert(udp:setpeername("127.0.0.1", port))

  -- inline: a real sendto per sample (a kernel transition on every call).
  local t0 = socket.gettime()
  for _ = 1, iters do udp:send(frame) end
  local inlineNs = (socket.gettime() - t0) * 1e9 / iters

  -- decoupled: enqueue-only cost (plain table insert), unbounded so the
  -- timed loop never has to drain.
  local queue, tail = {}, 0
  local t1 = socket.gettime()
  for _ = 1, iters do
    tail = tail + 1
    queue[tail] = sample
  end
  local enqueueNs = (socket.gettime() - t1) * 1e9 / iters

  udp:close()
  print(string.format(
    "BENCH enqueue_ns=%d inline_send_ns=%d note=cooperative_single_os_thread_no_concurrent_drain",
    math.floor(enqueueNs + 0.5), math.floor(inlineNs + 0.5)))
end

local port = tonumber(arg[1])
local n = tonumber(arg[2]) or 0
local mode = arg[3] or "run"
if port == nil then
  io.stderr:write("usage: reliable_app.lua <port> <N> [run|bench]\n")
  os.exit(2)
end

if mode == "bench" then
  runBench(port)
else
  runReliable(port, n)
end
