# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 ZeroDDS Contributors
#
# Unit tests for reliable.nim (mirror crates/xrce/src/reliable.rs) + byte-golden
# assertion against golden_heartbeat_le.bin / golden_acknack_le.bin.
# Run: nim c -r reliable_test.nim [golden_dir]

import std/[options, os]
import ./reliable

proc fileBytes(path: string): seq[byte] =
  let s = readFile(path)
  result = newSeq[byte](s.len)
  for i in 0 ..< s.len:
    result[i] = byte(s[i])

var failed = 0
proc check(cond: bool, name: string) =
  if cond:
    echo "ok   ", name
  else:
    echo "FAIL ", name
    inc failed

# --- sender ---

block:
  var s = newSender()
  let a = s.submit(@[1'u8, 2])
  let b = s.submit(@[3'u8, 4])
  check(a.status == srOk and a.seq == 0'u16, "submit_assigns_monotonic_seq_0")
  check(b.status == srOk and b.seq == 1'u16, "submit_assigns_monotonic_seq_1")
  check(s.inFlightCount == 2, "submit_two_in_flight")

block:
  var s = newSender()
  let huge = newSeq[byte](MaxPayload + 1)
  check(s.submit(huge).status == srTooLarge, "submit_rejects_payload_too_large")

block:
  var s = newSender()
  for _ in 0 ..< SenderWindow:
    discard s.submit(@[0'u8])
  check(s.submit(@[0'u8]).status == srWindowFull, "submit_rejects_when_window_full")

block:
  var s = newSender()
  discard s.submit(@[1'u8])
  check(s.pendingHeartbeat(0).isSome, "heartbeat_fires_first_time")
  let hb = s.pendingHeartbeat(0)
  discard hb

block:
  var s = newSender()
  discard s.submit(@[1'u8])
  let first = s.pendingHeartbeat(0)
  check(first.isSome and first.get.first == 0'u16 and first.get.last == 0'u16 and
        first.get.streamId == ReliableStreamId, "heartbeat_body_first_last_stream")
  check(s.pendingHeartbeat(100).isNone, "heartbeat_silenced_before_period")
  check(s.pendingHeartbeat(600).isSome, "heartbeat_fires_after_period")

block:
  var s = newSender()
  check(s.pendingHeartbeat(0).isNone, "heartbeat_none_when_window_empty")

block:
  var s = newSender()
  discard s.submit(@[0xA0'u8]) # seq 0
  discard s.submit(@[0xA1'u8]) # seq 1
  discard s.submit(@[0xA2'u8]) # seq 2
  # base=2, bitmap bit0 set → seq2 missing; 0+1 acked
  s.recvAcknack(AckNackBody(firstUnacked: 2'u16, nackLo: 0x01, nackHi: 0x00,
                            streamId: ReliableStreamId))
  check(s.inFlightCount == 1 and s.getInFlight(2'u16).isSome, "acknack_clears_acked_keeps_missing")

block:
  var s = newSender()
  for _ in 0 ..< 5:
    discard s.submit(@[0'u8])
  s.recvAcknack(AckNackBody(firstUnacked: 5'u16, nackLo: 0, nackHi: 0,
                            streamId: ReliableStreamId))
  check(s.inFlightCount == 0, "acknack_full_clear_when_no_bits_set")

# --- receiver ---

block:
  var r = newReceiver()
  discard r.recvData(0'u16, @[10'u8])
  discard r.recvData(1'u16, @[11'u8])
  let d = r.drainInOrder()
  check(d.len == 2 and d[0].seq == 0'u16 and d[1].seq == 1'u16 and r.expected == 2'u16,
        "recv_data_buffers_in_order")

block:
  var r = newReceiver()
  discard r.recvData(2'u16, @[22'u8])
  discard r.recvData(0'u16, @[20'u8])
  let d1 = r.drainInOrder()
  check(d1.len == 1 and d1[0].seq == 0'u16, "recv_data_reorder_blocks_on_gap")
  discard r.recvData(1'u16, @[21'u8])
  let d2 = r.drainInOrder()
  check(d2.len == 2 and d2[0].seq == 1'u16 and d2[1].seq == 2'u16, "recv_data_reorder_delivers")

block:
  var r = newReceiver()
  discard r.recvData(0'u16, @[1'u8])
  discard r.drainInOrder()
  discard r.recvData(0'u16, @[99'u8]) # duplicate < expected
  check(r.outOfOrderCount == 0, "recv_data_drops_duplicates")

block:
  var r = newReceiver()
  for i in 1'u16 .. uint16(ReceiverBuffer): # fill (expected=0, seqs 1..64)
    discard r.recvData(i, @[1'u8])
  check(r.recvData(uint16(ReceiverBuffer) + 1'u16, @[1'u8]) == false,
        "recv_data_rejects_when_buffer_full")

block:
  var r = newReceiver()
  discard r.recvData(1'u16, @[1'u8])
  discard r.recvData(3'u16, @[3'u8])
  let a = r.pendingAcknack(some(3'u16))
  let bitmap = uint16(a.nackLo) or (uint16(a.nackHi) shl 8)
  check((bitmap and 0x01'u16) != 0 and (bitmap and 0x04'u16) != 0 and
        (bitmap and 0x02'u16) == 0 and (bitmap and 0x08'u16) == 0,
        "pending_acknack_marks_missing_slots")

block:
  var s = newSender()
  var r = newReceiver()
  discard s.submit(@[1'u8, 2])
  discard r.recvData(0'u16, @[3'u8])
  s.reset()
  r.reset()
  check(s.inFlightCount == 0 and r.outOfOrderCount == 0 and r.expected == 0'u16,
        "reset_clears_state")

# --- end-to-end loss recovery (mirror reliable.rs) ---

block:
  var sender = newSender()
  var receiver = newReceiver()
  let s0 = sender.submit(@[10'u8]).seq
  let s1 = sender.submit(@[11'u8]).seq
  let s2 = sender.submit(@[12'u8]).seq
  discard receiver.recvData(s0, @[10'u8])
  discard receiver.recvData(s2, @[12'u8]) # s1 lost
  let d = receiver.drainInOrder()
  check(d.len == 1 and d[0].payload == @[10'u8], "e2e_drain_blocks_on_lost_s1")
  let ack = receiver.pendingAcknack(some(s2))
  sender.recvAcknack(ack)
  check(sender.getInFlight(s1).isSome, "e2e_s1_retransmittable")
  discard receiver.recvData(s1, sender.getInFlight(s1).get)
  let d2 = receiver.drainInOrder()
  check(d2.len == 2 and d2[0].payload == @[11'u8] and d2[1].payload == @[12'u8],
        "e2e_delivers_all_after_retransmit")

# --- byte-golden ---

if paramCount() >= 1:
  let dir = paramStr(1)
  let hbPath = dir / "golden_heartbeat_le.bin"
  let anPath = dir / "golden_acknack_le.bin"
  if fileExists(hbPath) and fileExists(anPath):
    let goldHb = fileBytes(hbPath)
    let goldAn = fileBytes(anPath)
    # golden = MessageHeader(session 0x80, stream NONE, msgSeq 1) + submessage
    let hb = heartbeatFrame(0x80'u8, StreamNone, 1,
                            HeartbeatBody(first: 1'u16, last: 3'u16, streamId: 0x80'u8))
    let an = acknackFrame(0x80'u8, StreamNone, 1,
                          AckNackBody(firstUnacked: 1'u16, nackLo: 0, nackHi: 0, streamId: 0x80'u8))
    check(hb == goldHb, "byte_golden_heartbeat")
    check(an == goldAn, "byte_golden_acknack")
    # round-trip: parse the golden and re-encode
    let phb = parseHeartbeat(goldHb)
    check(phb.isSome and phb.get.first == 1'u16 and phb.get.last == 3'u16, "golden_heartbeat_parse")
    let pan = parseAcknack(goldAn)
    check(pan.isSome and pan.get.firstUnacked == 1'u16, "golden_acknack_parse")
  else:
    echo "skip byte-golden: goldens not found in ", dir
else:
  echo "skip byte-golden: no golden dir arg"

if failed == 0:
  echo "ALL OK"
  quit(0)
else:
  echo failed, " FAILED"
  quit(1)
