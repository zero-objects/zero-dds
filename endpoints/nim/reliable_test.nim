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

# --- RFC-1982 regression: HEARTBEAT window + loss recovery across the 16-bit
#     wrap (mirror crates/xrce's wrap regression tests). Seeds sender/receiver
#     up to the wrap via the public API only (submit + full-ack / recvData +
#     drain), then straddles 0x0000.

block:
  var s = newSender()
  var seq: uint16 = 0'u16 # walk nextSeq to 0xFFFE: submit one, fully-ack it, repeat.
  while true:
    seq = s.submit(@[0'u8]).seq
    s.recvAcknack(AckNackBody(firstUnacked: seq + 1'u16, nackLo: 0'u8, nackHi: 0'u8, streamId: 0x80'u8))
    if seq == 0xFFFD'u16: break
  check(s.inFlightCount == 0, "wrap_seed_sender_drained")

  let q0 = s.submit(@[10'u8]).seq  # 0xFFFE
  let q1 = s.submit(@[11'u8]).seq  # 0xFFFF (lost)
  let q2 = s.submit(@[12'u8]).seq  # 0x0000
  let q3 = s.submit(@[13'u8]).seq  # 0x0001
  check(q0 == 0xFFFE'u16 and q1 == 0xFFFF'u16 and q2 == 0x0000'u16 and q3 == 0x0001'u16, "wrap_seqs")

  let hb = s.pendingHeartbeat(0)
  check(hb.isSome and hb.get.first == 0xFFFE'u16 and hb.get.last == 0x0001'u16,
        "heartbeat_window_across_wrap_is_rfc1982_not_numeric")

  var r = newReceiver()
  var k: uint16 = 0'u16 # seed expected to 0xFFFE
  while true:
    discard r.recvData(k, @[0'u8])
    discard r.drainInOrder()
    if k == 0xFFFD'u16: break
    k = k + 1'u16
  check(r.expected == 0xFFFE'u16, "wrap_seed_receiver_expects_fffe")

  discard r.recvData(q0, @[10'u8])  # 0xFFFF lost
  discard r.recvData(q2, @[12'u8])
  discard r.recvData(q3, @[13'u8])
  let d1 = r.drainInOrder()
  check(d1.len == 1 and d1[0].seq == 0xFFFE'u16, "wrap_only_first_delivered")
  check(r.expected == 0xFFFF'u16, "wrap_receiver_blocked_at_ffff")

  let ack = r.pendingAcknack(some(q3))
  check(ack.firstUnacked == 0xFFFF'u16, "wrap_ack_base_is_ffff")
  let bm = uint16(ack.nackLo) or (uint16(ack.nackHi) shl 8)
  check((bm and 0x1'u16) != 0 and (bm and 0x6'u16) == 0, "wrap_only_ffff_nacked")

  s.recvAcknack(ack)
  check(s.getInFlight(q1).isSome, "wrap_ffff_retransmittable")
  check(s.getInFlight(q0).isNone and s.inFlightCount == 1, "wrap_others_acked")

  discard r.recvData(q1, s.getInFlight(q1).get)
  let d2 = r.drainInOrder()
  check(d2.len == 3 and d2[0].seq == 0xFFFF'u16 and d2[1].seq == 0x0000'u16 and d2[2].seq == 0x0001'u16,
        "wrap_deliver_in_rfc1982_order_after_retransmit")

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
