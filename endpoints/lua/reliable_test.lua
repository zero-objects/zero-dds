-- SPDX-License-Identifier: Apache-2.0
-- Copyright 2026 ZeroDDS Contributors
--
-- Unit tests for reliable.lua (mirrors crates/xrce/src/reliable.rs) + a
-- byte-golden assertion against golden_heartbeat_le.bin /
-- golden_acknack_le.bin.
--
-- Run: lua5.4 reliable_test.lua [golden_dir]

package.path = "./?.lua;" .. package.path
local rel = require("reliable")

local failed = 0
local function check(cond, name)
  if cond then
    print("ok   " .. name)
  else
    print("FAIL " .. name)
    failed = failed + 1
  end
end

-- --- sender ---

do
  local s = rel.Sender.new()
  local a = s:submit("\1\2")
  local b = s:submit("\3\4")
  check(a == 0, "submit_assigns_monotonic_seq_0")
  check(b == 1, "submit_assigns_monotonic_seq_1")
  check(s:inFlightCount() == 2, "submit_two_in_flight")
end

do
  local s = rel.Sender.new()
  local huge = string.rep("\0", rel.MAX_PAYLOAD + 1)
  local seq, err = s:submit(huge)
  check(seq == nil and err == "too_large", "submit_rejects_payload_too_large")
end

do
  local s = rel.Sender.new()
  for _ = 1, rel.SENDER_WINDOW do s:submit("\0") end
  local seq, err = s:submit("\0")
  check(seq == nil and err == "window_full", "submit_rejects_when_window_full")
end

do
  local s = rel.Sender.new()
  s:submit("\1")
  check(s:pendingHeartbeat(0) ~= nil, "heartbeat_fires_first_time")
end

do
  local s = rel.Sender.new()
  s:submit("\1")
  local first = s:pendingHeartbeat(0)
  check(first ~= nil and first.first == 0 and first.last == 0 and first.streamId == rel.RELIABLE_STREAM_ID,
    "heartbeat_body_first_last_stream")
  check(s:pendingHeartbeat(100) == nil, "heartbeat_silenced_before_period")
  check(s:pendingHeartbeat(600) ~= nil, "heartbeat_fires_after_period")
end

do
  local s = rel.Sender.new()
  check(s:pendingHeartbeat(0) == nil, "heartbeat_none_when_window_empty")
end

do
  local s = rel.Sender.new()
  s:submit("\xA0") -- seq 0
  s:submit("\xA1") -- seq 1
  s:submit("\xA2") -- seq 2
  -- base=2, bitmap bit0 set -> seq2 still missing; seq0/seq1 acked.
  s:recvAcknack(2, 0x01, 0x00)
  check(s:inFlightCount() == 1 and s:getInFlight(2) ~= nil, "acknack_clears_acked_keeps_missing")
end

do
  local s = rel.Sender.new()
  for _ = 1, 5 do s:submit("\0") end
  s:recvAcknack(5, 0, 0)
  check(s:inFlightCount() == 0, "acknack_full_clear_when_no_bits_set")
end

-- RFC-1982 regression: HEARTBEAT window + loss recovery across the 16-bit wrap
-- (mirrors crates/xrce's wrap regression tests). Window 0xFFFE,0xFFFF,0,1 ->
-- the old numeric min/max reported first=0/last=0xFFFF; correct is
-- first=0xFFFE, last=0x0001.
do
  local s = rel.Sender.new()
  s.nextSeq = 0xFFFE
  local q0 = s:submit("\10") -- 0xFFFE
  local q1 = s:submit("\11") -- 0xFFFF (lost)
  local q2 = s:submit("\12") -- 0x0000
  local q3 = s:submit("\13") -- 0x0001
  check(q0 == 0xFFFE and q1 == 0xFFFF and q2 == 0x0000 and q3 == 0x0001, "wrap_seqs")
  local hb = s:pendingHeartbeat(0)
  check(hb ~= nil and hb.first == 0xFFFE and hb.last == 0x0001,
    "heartbeat_window_across_wrap_is_rfc1982_not_numeric")

  local r = rel.Receiver.new()
  r.expected = 0xFFFE
  r:recvData(q0, "\10") -- 0xFFFF lost
  r:recvData(q2, "\12")
  r:recvData(q3, "\13")
  local d1 = r:drainInOrder()
  check(#d1 == 1 and d1[1].seq == 0xFFFE, "wrap_only_first_delivered")
  check(r.expected == 0xFFFF, "wrap_receiver_blocked_at_ffff")

  local ack = r:pendingAcknack(q3)
  check(ack.firstUnacked == 0xFFFF, "wrap_ack_base_is_ffff")
  local bitmap = ack.nackLo | (ack.nackHi << 8)
  check((bitmap & 0x1) ~= 0 and (bitmap & 0x6) == 0, "wrap_only_ffff_nacked")

  s:recvAcknack(ack.firstUnacked, ack.nackLo, ack.nackHi)
  check(s:getInFlight(q1) ~= nil, "wrap_ffff_retransmittable")
  check(s:getInFlight(q0) == nil and s:inFlightCount() == 1, "wrap_others_acked")

  r:recvData(q1, s:getInFlight(q1))
  local d2 = r:drainInOrder()
  check(#d2 == 3 and d2[1].seq == 0xFFFF and d2[2].seq == 0x0000 and d2[3].seq == 0x0001,
    "wrap_deliver_in_rfc1982_order_after_retransmit")
end

-- --- receiver ---

do
  local r = rel.Receiver.new()
  r:recvData(0, "\10")
  r:recvData(1, "\11")
  local d = r:drainInOrder()
  check(#d == 2 and d[1].seq == 0 and d[2].seq == 1 and r.expected == 2, "recv_data_buffers_in_order")
end

do
  local r = rel.Receiver.new()
  r:recvData(2, "\22")
  r:recvData(0, "\20")
  local d1 = r:drainInOrder()
  check(#d1 == 1 and d1[1].seq == 0, "recv_data_reorder_blocks_on_gap")
  r:recvData(1, "\21")
  local d2 = r:drainInOrder()
  check(#d2 == 2 and d2[1].seq == 1 and d2[2].seq == 2, "recv_data_reorder_delivers")
end

do
  local r = rel.Receiver.new()
  r:recvData(0, "\1")
  r:drainInOrder()
  r:recvData(0, "\99") -- duplicate < expected
  check(r:outOfOrderCount() == 0, "recv_data_drops_duplicates")
end

do
  local r = rel.Receiver.new()
  for i = 1, rel.RECEIVER_BUFFER do r:recvData(i, "\1") end -- expected=0, seqs 1..64
  check(r:recvData(rel.RECEIVER_BUFFER + 1, "\1") == false, "recv_data_rejects_when_buffer_full")
end

do
  local r = rel.Receiver.new()
  r:recvData(1, "\1")
  r:recvData(3, "\3")
  local a = r:pendingAcknack(3)
  local bitmap = a.nackLo | (a.nackHi << 8)
  check((bitmap & 0x01) ~= 0 and (bitmap & 0x04) ~= 0 and (bitmap & 0x02) == 0 and (bitmap & 0x08) == 0,
    "pending_acknack_marks_missing_slots")
end

do
  local s = rel.Sender.new()
  local r = rel.Receiver.new()
  s:submit("\1\2")
  r:recvData(0, "\3")
  s:reset()
  r:reset()
  check(s:inFlightCount() == 0 and r:outOfOrderCount() == 0 and r.expected == 0, "reset_clears_state")
end

-- --- end-to-end loss recovery (mirror reliable.rs) ---

do
  local sender = rel.Sender.new()
  local receiver = rel.Receiver.new()
  local s0 = sender:submit("\10")
  local s1 = sender:submit("\11")
  local s2 = sender:submit("\12")
  receiver:recvData(s0, "\10")
  receiver:recvData(s2, "\12") -- s1 lost
  local d = receiver:drainInOrder()
  check(#d == 1 and d[1].payload == "\10", "e2e_drain_blocks_on_lost_s1")
  local ack = receiver:pendingAcknack(s2)
  sender:recvAcknack(ack.firstUnacked, ack.nackLo, ack.nackHi)
  check(sender:getInFlight(s1) ~= nil, "e2e_s1_retransmittable")
  receiver:recvData(s1, sender:getInFlight(s1))
  local d2 = receiver:drainInOrder()
  check(#d2 == 2 and d2[1].payload == "\11" and d2[2].payload == "\12", "e2e_delivers_all_after_retransmit")
end

-- --- AsyncWriter (cooperative submit/drain split) ---

do
  local sent = {}
  local s = rel.Sender.new()
  local w = rel.AsyncWriter.new(s, function(frame) sent[#sent + 1] = frame end)
  check(w:push("\1") and w:push("\2"), "asyncwriter_push_ok")
  check(not w:isEmpty(), "asyncwriter_not_empty_before_drain")
  local n = w:drain()
  check(n == 2 and #sent == 2 and w:isEmpty(), "asyncwriter_drain_submits_and_sends")
  check(s:inFlightCount() == 2, "asyncwriter_drain_advances_sender")
end

do
  -- push cap mirrors SENDER_WINDOW; drain() must free room for more pushes.
  local s = rel.Sender.new()
  local w = rel.AsyncWriter.new(s, function() end)
  for _ = 1, rel.SENDER_WINDOW do
    check(w:push("\0"), "asyncwriter_push_within_window")
  end
  check(not w:push("\0"), "asyncwriter_push_rejects_at_cap")
  w:drain()
  check(w:push("\0"), "asyncwriter_push_after_drain")
end

-- --- byte-golden ---

local function readFile(path)
  local f = io.open(path, "rb")
  if not f then return nil end
  local data = f:read("*a")
  f:close()
  return data
end

if #arg >= 1 then
  local dir = arg[1]
  local goldHb = readFile(dir .. "/golden_heartbeat_le.bin")
  local goldAn = readFile(dir .. "/golden_acknack_le.bin")
  if goldHb and goldAn then
    local hb = rel.heartbeatFrame(rel.SESSION_NOKEY, rel.STREAM_NONE, 1, 1, 3, 0x80)
    local an = rel.acknackFrame(rel.SESSION_NOKEY, rel.STREAM_NONE, 1, 1, 0, 0, 0x80)
    check(hb == goldHb, "byte_golden_heartbeat")
    check(an == goldAn, "byte_golden_acknack")
    -- round-trip: parse the golden bytes and check the decoded fields.
    local phb = rel.parseHeartbeat(goldHb)
    check(phb ~= nil and phb.first == 1 and phb.last == 3 and phb.streamId == 0x80, "golden_heartbeat_parse")
    local pan = rel.parseAcknack(goldAn)
    check(pan ~= nil and pan.firstUnacked == 1 and pan.streamId == 0x80, "golden_acknack_parse")
  else
    print("skip byte-golden: goldens not found in " .. dir)
  end
else
  print("skip byte-golden: no golden dir arg")
end

if failed == 0 then
  print("ALL OK")
  os.exit(0)
else
  print(failed .. " FAILED")
  os.exit(1)
end
