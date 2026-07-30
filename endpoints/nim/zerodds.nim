# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 ZeroDDS Contributors
#
# Native pure-Nim endpoint SDK (ADR 0013): a from-scratch XCDR wire-core,
# byte-identical to the Rust core and the other SDKs. Sync (poll) and async
# (std/asyncdispatch — async/await/Future, the idiomatic Nim model).

import std/[options, asyncdispatch, deques]

type Endian* = enum
  eLE
  eBE

# --- Writer: a byte seq + endian; alignment relative to buffer start, cap 4 ---

type Writer* = object
  buf: seq[byte]
  endian: Endian

proc initWriter*(endian: Endian): Writer =
  Writer(buf: @[], endian: endian)

proc align(w: var Writer, a: int) =
  let cap = min(a, 4)
  let pad = (cap - (w.buf.len mod cap)) mod cap
  for _ in 0 ..< pad:
    w.buf.add(0'u8)

# le: the value already encoded little-endian; reversed for eBE.
proc put(w: var Writer, a: int, le: seq[byte]) =
  w.align(a)
  if w.endian == eBE:
    for i in countdown(le.high, 0):
      w.buf.add(le[i])
  else:
    for b in le:
      w.buf.add(b)

proc leBytes(v: uint64, n: int): seq[byte] =
  result = newSeq[byte](n)
  for i in 0 ..< n:
    result[i] = byte((v shr (8 * i)) and 0xff)

proc putU8*(w: var Writer, v: int) = w.buf.add(byte(v and 0xff))
proc putU16*(w: var Writer, v: int) = w.put(2, leBytes(uint64(v), 2))
proc putU32*(w: var Writer, v: uint32) = w.put(4, leBytes(uint64(v), 4))
proc putU64*(w: var Writer, v: uint64) = w.put(4, leBytes(v, 8))
proc putF32*(w: var Writer, v: float32) = w.put(4, leBytes(uint64(cast[uint32](v)), 4))

proc putString*(w: var Writer, s: string) =
  w.putU32(uint32(s.len + 1))
  for c in s:
    w.buf.add(byte(c))
  w.putU8(0)

proc putSeqU8*(w: var Writer, b: seq[byte]) =
  w.putU32(uint32(b.len))
  for x in b:
    w.buf.add(x)

proc bytes*(w: Writer): seq[byte] = w.buf

# --- Reader: a byte cursor ---

type Reader* = object
  data: seq[byte]
  pos: int
  endian: Endian

proc initReader*(data: seq[byte], endian: Endian): Reader =
  Reader(data: data, pos: 0, endian: endian)

proc take(r: var Reader, a, n: int): seq[byte] =
  let cap = min(a, 4)
  let pad = (cap - (r.pos mod cap)) mod cap
  r.pos += pad
  result = r.data[r.pos ..< r.pos + n]
  r.pos += n
  if r.endian == eBE:
    var rev = newSeq[byte](n)
    for i in 0 ..< n:
      rev[i] = result[n - 1 - i]
    result = rev

proc getU32*(r: var Reader): uint32 =
  let b = r.take(4, 4)
  uint32(b[0]) or (uint32(b[1]) shl 8) or (uint32(b[2]) shl 16) or (uint32(b[3]) shl 24)

# --- primitives beyond getU32 — the byte-exact inverse of the Writer ---

proc getU8*(r: var Reader): uint8 =
  result = r.data[r.pos]
  r.pos += 1

proc getU16*(r: var Reader): uint16 =
  let b = r.take(2, 2)
  uint16(b[0]) or (uint16(b[1]) shl 8)

proc getU64*(r: var Reader): uint64 =
  let b = r.take(4, 8)
  var v: uint64 = 0
  for i in 0 ..< 8:
    v = v or (uint64(b[i]) shl (8 * i))
  v

proc getF32*(r: var Reader): float32 =
  cast[float32](r.getU32())

proc getString*(r: var Reader): string =
  let n = int(r.getU32())
  result = newString(n - 1)
  for i in 0 ..< n - 1:
    result[i] = char(r.data[r.pos + i])
  r.pos += n

proc getSeqU8*(r: var Reader): seq[byte] =
  let n = int(r.getU32())
  result = r.data[r.pos ..< r.pos + n]
  r.pos += n

# --- XRCE WRITE_DATA framing ---

const SessionNoKey* = 0x80'u8
const StreamBestEffort* = 0x01'u8

proc writeFrame*(session, stream: uint8, seqNo: int, sample: seq[byte]): seq[byte] =
  # The 16-bit submessage_length field cannot describe a sample larger than
  # 0xFFFF; refuse (empty seq) rather than truncate the length while appending
  # the full payload (mirrors the C SDK's rejection).
  if sample.len > 0xFFFF:
    return @[]
  let n = sample.len
  result = @[session, stream,
             byte(seqNo and 0xff), byte((seqNo shr 8) and 0xff),
             0x07'u8, 0x03'u8,
             byte(n and 0xff), byte((n shr 8) and 0xff)]
  for x in sample:
    result.add(x)

## Frame layout: [session, stream, seq_lo, seq_hi, sm_id, flags, len_lo, len_hi]
## then the sample body. The 16-bit little-endian submessage_length (bytes 6..7)
## bounds the body exactly: frame[8 ..< 8+smLen], never frame[8 ..< frame.len],
## so trailing padding or an appended submessage is not folded into the sample.
##
## Accepts WRITE_DATA (0x07, endpoint->hub / loopback) and DATA (0x09,
## hub->endpoint) — the hub pushes samples with DATA. See DDS-XRCE spec 8.3.5.
## Returns none for a short header, a wrong submessage id, or a declared length
## that runs past the datagram (truncation / wrong length).
proc readFrame*(frame: seq[byte]): Option[seq[byte]] =
  if frame.len < 8 or (frame[4] != 0x07'u8 and frame[4] != 0x09'u8):
    return none(seq[byte])
  let smLen = int(frame[6]) or (int(frame[7]) shl 8)
  if 8 + smLen > frame.len:
    return none(seq[byte])
  some(frame[8 ..< 8 + smLen])

# --- transport ---

type Transport* = object
  deliver*: proc(frame: seq[byte]) {.closure.}
  receive*: proc(): Option[seq[byte]] {.closure.}

# --- sync client ---

type Client* = ref object
  transport: Transport
  session, stream: uint8
  seqNo: int

proc newClient*(t: Transport): Client =
  Client(transport: t, session: SessionNoKey, stream: StreamBestEffort, seqNo: 1)

proc write*(c: Client, sample: seq[byte]) =
  let frame = writeFrame(c.session, c.stream, c.seqNo, sample)
  c.transport.deliver(frame)
  c.seqNo = (c.seqNo + 1) and 0xffff

# One non-blocking receive: returns the sample body, or none.
proc poll*(c: Client): Option[seq[byte]] =
  let f = c.transport.receive()
  if f.isSome: readFrame(f.get) else: none(seq[byte])

# --- async reader: an async/await Future (idiomatic Nim) ---

type AsyncReader* = ref object
  transport: Transport

proc newAsyncReader*(t: Transport): AsyncReader =
  AsyncReader(transport: t)

proc recv*(r: AsyncReader): Future[seq[byte]] {.async.} =
  while true:
    let f = r.transport.receive()
    if f.isNone:
      await sleepAsync(1)
    else:
      let body = readFrame(f.get)
      if body.isSome:
        return body.get

# In-memory FIFO transport for tests + example.
proc memTransport*(): Transport =
  var q = initDeque[seq[byte]]()
  Transport(
    deliver: proc(frame: seq[byte]) = q.addLast(frame),
    receive: proc(): Option[seq[byte]] =
      if q.len > 0: some(q.popFirst()) else: none(seq[byte])
  )
