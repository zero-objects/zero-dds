# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 ZeroDDS Contributors
#
# Reliable XRCE stream (spec §8.4.10/§8.4.11) for the Nim endpoint SDK.
# Self-contained: the sender/receiver state machines + the HEARTBEAT / ACKNACK
# frame codec, byte-identical to `crates/xrce` and `endpoints/c`. Importable
# beside `zerodds.nim`; the reliable WRITE_DATA frame is the same 8-byte header
# as `zerodds.writeFrame(0x80, 0x80, seq, sample)`.
#
# Frame layout (little-endian): [0]=session [1]=stream [2..4]=seq LE
#   [4]=submessage id [5]=flags [6..8]=body-len LE [8..]=body
#   WRITE_DATA id=0x07 flags=0x03 body=sample; header seq = RFC-1982 sample seq
#   HEARTBEAT  id=0x0B flags=0x01 body(5)= first i16 LE, last i16 LE, stream
#   ACKNACK    id=0x0A flags=0x01 body(9)= DHEADER u32 LE (=5), first_unacked i16 LE,
#              nack[0], nack[1], stream. ACKNACK_Payload has no @extensibility, so it is
#              APPENDABLE (DDS-XTypes 1.3 7.3.1.2.1.8) and carries a DHEADER in XCDR2
#              (`UnannotatedTypes.Appendable`, the default). With
#              `UnannotatedTypes.Final` the body is the 5-octet FINAL form of the
#              reference implementation (no DHEADER, length field 5).
#              The parser accepts both forms.

import std/[tables, options]

const
  SessionNoKey* = 0x80'u8
  StreamNone* = 0x00'u8
  ReliableStreamId* = 0x80'u8
  HeartbeatPeriodMs* = 500
  SenderWindow* = 16
  ReceiverBuffer* = 64
  MaxPayload* = 65535
  SmWriteData = 0x07'u8
  SmAckNack = 0x0A'u8
  SmHeartbeat = 0x0B'u8
  ## E flag (little-endian body) + DataFormat FORMAT_DATA (0b000) in bits 1..3
  ## -- DDS-XRCE 1.0 8.3.5.8.
  FlagWrite = 0x01'u8
  ## Wire size of the BaseObjectRequest that precedes the sample (7.7.8).
  BaseSize* = 4
  FlagELe = 0x01'u8

type
  UnannotatedTypes* {.pure.} = enum
    ## Encoding of the DDS-XRCE 1.0 Annex A types without `@extensibility`
    ## (APPENDABLE by DDS-XTypes 1.3 7.3.1.2.1.8). `Appendable` (default): a
    ## DHEADER in front of the members. `Final`: the form of the reference
    ## implementation (eProsima), no DHEADER. This SDK writes one such type:
    ## ACKNACK_Payload.
    Appendable, Final
  HeartbeatBody* = object
    first*, last*: uint16
    streamId*: uint8
  AckNackBody* = object
    firstUnacked*: uint16
    nackLo*, nackHi*: uint8
    streamId*: uint8
  SubmitStatus* = enum
    srOk, srTooLarge, srWindowFull

# --- RFC-1982 16-bit serial-number comparison ---

proc seqLt*(a, b: uint16): bool =
  ## `a < b` in RFC-1982 order (half-window = 32768).
  let d = (b.uint32 - a.uint32) and 0xffff
  d != 0'u32 and d < 0x8000'u32

proc seqGt*(a, b: uint16): bool =
  seqLt(b, a)

# --- frame codec ---

proc reliableWriteFrame*(seqNo: uint16, objectId: uint16, sample: seq[byte]): seq[byte] =
  ## Reliable WRITE_DATA — identical bytes to
  ## `writeFrame(0x80, 0x80, seq, objectId, sample)`: the submessage body is the
  ## BaseObjectRequest (7.7.8: request_id = the sample's own seq, object_id =
  ## the target DataWriter) followed by the sample.
  let n = BaseSize + sample.len
  result = @[SessionNoKey, ReliableStreamId,
             byte(seqNo and 0xff'u16), byte((seqNo shr 8) and 0xff'u16),
             SmWriteData, FlagWrite,
             byte(n and 0xff), byte((n shr 8) and 0xff),
             byte((seqNo shr 8) and 0xff'u16), byte(seqNo and 0xff'u16),
             byte((objectId shr 8) and 0xff'u16), byte(objectId and 0xff'u16)]
  for x in sample:
    result.add(x)

proc heartbeatFrame*(session, streamHdr: uint8, msgSeq: int, hb: HeartbeatBody): seq[byte] =
  ## byte-identical to `golden_heartbeat_le.bin` for (0x80, 0x00, 1, {1,3,0x80}).
  @[session, streamHdr, byte(msgSeq and 0xff), byte((msgSeq shr 8) and 0xff),
    SmHeartbeat, FlagELe, 5'u8, 0'u8,
    byte(hb.first and 0xff'u16), byte((hb.first shr 8) and 0xff'u16),
    byte(hb.last and 0xff'u16), byte((hb.last shr 8) and 0xff'u16),
    hb.streamId]

proc acknackFrame*(session, streamHdr: uint8, msgSeq: int, ack: AckNackBody,
                   mode: UnannotatedTypes = UnannotatedTypes.Appendable): seq[byte] =
  ## `Appendable` (default): 17-octet frame, length field 9, DHEADER = 5;
  ## byte-identical to `golden_acknack_le.bin` for (0x80, 0x00, 1, {1,0,0,0x80}).
  ## `Final`: 13-octet frame, length field 5, no DHEADER (reference implementation).
  let payload = @[byte(ack.firstUnacked and 0xff'u16), byte((ack.firstUnacked shr 8) and 0xff'u16),
                  ack.nackLo, ack.nackHi, ack.streamId]
  case mode
  of UnannotatedTypes.Appendable:
    result = @[session, streamHdr, byte(msgSeq and 0xff), byte((msgSeq shr 8) and 0xff),
               SmAckNack, FlagELe, 9'u8, 0'u8,
               5'u8, 0'u8, 0'u8, 0'u8]
  of UnannotatedTypes.Final:
    result = @[session, streamHdr, byte(msgSeq and 0xff), byte((msgSeq shr 8) and 0xff),
               SmAckNack, FlagELe, 5'u8, 0'u8]
  result.add(payload)

proc parseHeartbeat*(frame: seq[byte]): Option[HeartbeatBody] =
  if frame.len < 13 or frame[4] != SmHeartbeat:
    return none(HeartbeatBody)
  some(HeartbeatBody(
    first: uint16(frame[8]) or (uint16(frame[9]) shl 8),
    last: uint16(frame[10]) or (uint16(frame[11]) shl 8),
    streamId: frame[12]))

proc parseAcknack*(frame: seq[byte]): Option[AckNackBody] =
  ## Accepts body length 9 (4-octet DHEADER, then the 5-octet payload) and the
  ## legacy FINAL form with body length 5.
  if frame.len < 13 or frame[4] != SmAckNack:
    return none(AckNackBody)
  var o = 8
  if frame[6] == 9'u8 and frame[7] == 0'u8:
    if frame.len < 17:
      return none(AckNackBody)
    o = 12
  elif not (frame[6] == 5'u8 and frame[7] == 0'u8):
    return none(AckNackBody)
  some(AckNackBody(
    firstUnacked: uint16(frame[o]) or (uint16(frame[o + 1]) shl 8),
    nackLo: frame[o + 2], nackHi: frame[o + 3], streamId: frame[o + 4]))

# --- sender ---

type Sender* = object
  nextSeq: uint16
  inFlight: Table[uint16, seq[byte]]
  lastHeartbeatMs: int ## -1 = never sent

proc newSender*(): Sender =
  Sender(nextSeq: 0'u16, inFlight: initTable[uint16, seq[byte]](), lastHeartbeatMs: -1)

proc inFlightCount*(s: Sender): int = s.inFlight.len

proc submit*(s: var Sender, payload: seq[byte]): tuple[status: SubmitStatus, seq: uint16] =
  ## Buffers a new sample. Errors when the payload is too large or the window
  ## (16 in-flight) is full — the caller must process ACKNACKs first.
  if payload.len > MaxPayload:
    return (srTooLarge, 0'u16)
  if s.inFlight.len >= SenderWindow:
    return (srWindowFull, 0'u16)
  let sq = s.nextSeq
  s.inFlight[sq] = payload
  s.nextSeq = s.nextSeq + 1'u16
  (srOk, sq)

proc getInFlight*(s: Sender, sq: uint16): Option[seq[byte]] =
  if s.inFlight.hasKey(sq): some(s.inFlight[sq]) else: none(seq[byte])

iterator inFlightPairs*(s: Sender): tuple[seq: uint16, payload: seq[byte]] =
  for k, v in s.inFlight.pairs:
    yield (k, v)

proc pendingHeartbeat*(s: var Sender, nowMs: int): Option[HeartbeatBody] =
  ## `some(HEARTBEAT)` when in-flight samples exist and the period elapsed
  ## (fires on the first call, then every `HeartbeatPeriodMs`).
  if s.inFlight.len == 0:
    return none(HeartbeatBody)
  let due = s.lastHeartbeatMs < 0 or (nowMs - s.lastHeartbeatMs) >= HeartbeatPeriodMs
  if not due:
    return none(HeartbeatBody)
  s.lastHeartbeatMs = nowMs
  # RFC-1982 window base (oldest unacked) + end (newest unacked); NOT numeric
  # min/max, which is wrong across a 16-bit wrap: window
  # 0xFFFE,0xFFFF,0x0000,0x0001 -> base 0xFFFE / end 0x0001, not 0x0000 /
  # 0xFFFF. Mirrors window_base / serial_max_in_flight in crates/xrce.
  var mn = 0'u16
  var mx = 0'u16
  var seen = false
  for k in s.inFlight.keys:
    if not seen:
      mn = k; mx = k; seen = true
    else:
      if seqLt(k, mn): mn = k
      if seqGt(k, mx): mx = k
  some(HeartbeatBody(first: mn, last: mx, streamId: ReliableStreamId))

proc recvAcknack*(s: var Sender, a: AckNackBody) =
  ## `base = firstUnacked`; everything `< base` (RFC-1982) is acknowledged and
  ## dropped; in `[base, base+16)` a set bit means missing (keep), a clear bit
  ## means acked (drop).
  let base = a.firstUnacked
  let bitmap = uint16(a.nackLo) or (uint16(a.nackHi) shl 8)
  var toRemove: seq[uint16] = @[]
  for k in s.inFlight.keys:
    let diff = (base.uint32 - k.uint32) and 0xffff
    if diff != 0'u32 and diff < 0x8000'u32:
      toRemove.add(k)
  for k in toRemove:
    s.inFlight.del(k)
  for i in 0'u16 ..< 16'u16:
    let sq = base + i
    if ((bitmap shr i) and 1'u16) == 0'u16 and s.inFlight.hasKey(sq):
      s.inFlight.del(sq)

# --- receiver ---

type Receiver* = object
  expected: uint16
  received: Table[uint16, seq[byte]]
  unannotated: UnannotatedTypes

proc newReceiver*(unannotatedTypes: UnannotatedTypes = UnannotatedTypes.Appendable): Receiver =
  ## `unannotatedTypes` selects the encoding of the ACKNACK_Payload this
  ## receiver puts on the wire (see `UnannotatedTypes`).
  Receiver(expected: 0'u16, received: initTable[uint16, seq[byte]](),
           unannotated: unannotatedTypes)

proc unannotatedTypes*(r: Receiver): UnannotatedTypes = r.unannotated

proc expected*(r: Receiver): uint16 = r.expected
proc outOfOrderCount*(r: Receiver): int = r.received.len

proc recvData*(r: var Receiver, sq: uint16, payload: seq[byte]): bool =
  ## Buffers an incoming sample. Returns `false` only when the receiver buffer
  ## is full (DoS bound); duplicates are silently accepted as a no-op.
  if seqLt(sq, r.expected):
    return true # already delivered → drop
  if r.received.hasKey(sq):
    return true
  if r.received.len >= ReceiverBuffer:
    return false
  r.received[sq] = payload
  true

proc drainInOrder*(r: var Receiver): seq[tuple[seq: uint16, payload: seq[byte]]] =
  ## Delivers contiguously from `expected`, advancing `expected`.
  result = @[]
  while r.received.hasKey(r.expected):
    result.add((r.expected, r.received[r.expected]))
    r.received.del(r.expected)
    r.expected = r.expected + 1'u16

proc pendingAcknack*(r: Receiver, hintLastSeen: Option[uint16]): AckNackBody =
  ## Bitmap of the missing slots in `[expected, expected+16)`; slots beyond
  ## `hintLastSeen` are not marked missing.
  let base = r.expected
  var bitmap: uint16 = 0
  for i in 0'u16 ..< 16'u16:
    let sq = base + i
    if hintLastSeen.isSome and seqGt(sq, hintLastSeen.get):
      continue
    if not r.received.hasKey(sq):
      bitmap = bitmap or (1'u16 shl i)
  AckNackBody(firstUnacked: base,
              nackLo: byte(bitmap and 0xff'u16),
              nackHi: byte((bitmap shr 8) and 0xff'u16),
              streamId: ReliableStreamId)

proc acknackFrame*(r: Receiver, session, streamHdr: uint8, msgSeq: int,
                   hintLastSeen: Option[uint16]): seq[byte] =
  ## ACKNACK frame for the current window, encoded per the receiver's
  ## `unannotatedTypes`.
  acknackFrame(session, streamHdr, msgSeq, r.pendingAcknack(hintLastSeen), r.unannotated)

proc reset*(s: var Sender) =
  s.nextSeq = 0'u16
  s.inFlight.clear()
  s.lastHeartbeatMs = -1

proc reset*(r: var Receiver) =
  r.expected = 0'u16
  r.received.clear()
