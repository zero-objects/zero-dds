-- SPDX-License-Identifier: Apache-2.0
-- Copyright 2026 ZeroDDS Contributors
--
-- Runnable in-process demo: a Sender submits 12 samples on the reliable
-- stream; every 3rd delivery attempt is dropped once; ACKNACK-driven
-- retransmit recovers the loss; the Receiver delivers all 12 gap-free, in
-- order. No sockets -- frames are round-tripped through the wire encoder /
-- decoder (`writeDataFrame` / `parseWriteData`), so the demo also exercises
-- the codec, not just the in-memory state machine.
--
-- Run: lua5.4 example_reliable.lua

package.path = "./?.lua;" .. package.path
local rel = require("reliable")

local N = 12
local sender = rel.Sender.new()
local receiver = rel.Receiver.new()

local seqs = {}
for i = 0, N - 1 do
  local payload = string.pack("<I4", i)
  local seq = assert(sender:submit(payload))
  seqs[#seqs + 1] = seq
end

local droppedOnce = {}
local attempts = 0

-- Simulated lossy channel: every 3rd delivery attempt is dropped exactly
-- once per sequence number (so the run still converges).
local function deliver(frame)
  local seq, sample = rel.parseWriteData(frame)
  assert(seq ~= nil, "not a WRITE_DATA frame")
  attempts = attempts + 1
  if attempts % 3 == 0 and not droppedOnce[seq] then
    droppedOnce[seq] = true
    return
  end
  assert(receiver:recvData(seq, sample))
end

local delivered = {}
local function drain()
  for _, item in ipairs(receiver:drainInOrder()) do
    delivered[#delivered + 1] = item.payload
  end
end

-- Round 1: send everything once.
for _, seq in ipairs(seqs) do
  deliver(rel.writeDataFrame(seq, sender:getInFlight(seq)))
end
drain()

-- ACKNACK-driven retransmit rounds until every sample is delivered.
local rounds = 0
while #delivered < N do
  rounds = rounds + 1
  assert(rounds <= 20, "loss recovery did not converge")
  local ack = receiver:pendingAcknack(nil)
  sender:recvAcknack(ack.firstUnacked, ack.nackLo, ack.nackHi)
  for seq, payload in sender:inFlightPairs() do
    deliver(rel.writeDataFrame(seq, payload))
  end
  drain()
end

for i, payload in ipairs(delivered) do
  local v = string.unpack("<I4", payload)
  assert(v == i - 1, string.format("out of order at index %d: got %d", i - 1, v))
end

print(string.format(
  "RELIABLE OK: %d/%d delivered gap-free in %d round(s), sequence 0..%d verified in order",
  #delivered, N, rounds, N - 1))
