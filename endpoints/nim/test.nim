# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 ZeroDDS Contributors
#
# Tests for the native Nim endpoint: byte-identity vs the Rust goldens, plus
# sync + async loopback. Run with `nim c -r test.nim` (GOLDEN_DIR=build).

import std/[os, options, asyncdispatch]
import ./zerodds
import ./surface_test
import ./xrce_client_test

let goldenDir = getEnv("GOLDEN_DIR", "build")

proc fixture(endian: Endian): seq[byte] =
  var w = initWriter(endian, eXcdr2)
  w.putU32(0xA1B2C3D4'u32)
  w.putU16(0x1234)
  w.putU8(0x5A)
  w.putF32(3.5'f32)
  w.putU64(0x0102030405060708'u64)
  w.putString("bay-12")
  w.putSeqU8(@[0xDE'u8, 0xAD, 0xBE, 0xEF])
  w.bytes()

proc sampleBody(id: uint32): seq[byte] =
  var w = initWriter(eLE, eXcdr2)
  w.putU32(id)
  w.putU16(0)
  w.putU8(0)
  w.bytes()

proc toBytes(s: string): seq[byte] =
  result = newSeq[byte](s.len)
  for i in 0 ..< s.len:
    result[i] = byte(s[i])

# byte identity
for (endian, file) in [(eLE, "golden_le.bin"), (eBE, "golden_be.bin")]:
  let got = fixture(endian)
  let golden = toBytes(readFile(goldenDir / file))
  doAssert got == golden, file & ": not byte-identical (got " & $got.len & " want " & $golden.len & ")"
  echo file, ": ", golden.len, " bytes byte-identical"

# sync loopback
let t = memTransport()
let c = newClient(t, xrceObjectId(0x001, ObjkDataWriter))
for i in 0 .. 4:
  c.write(sampleBody(0x3000'u32 + uint32(i)))
for i in 0 .. 4:
  let body = c.poll()
  doAssert body.isSome, "sync: no sample"
  var rd = initReader(body.get, eLE, eXcdr2)
  doAssert rd.getU32() == 0x3000'u32 + uint32(i), "sync: id mismatch"
echo "sync loopback: 5 samples OK"

# async loopback (asyncdispatch)
let t2 = memTransport()
let w = newClient(t2, xrceObjectId(0x001, ObjkDataWriter))
for i in 0 .. 4:
  w.write(sampleBody(0x1000'u32 + uint32(i)))
let r = newAsyncReader(t2)
for i in 0 .. 4:
  let body = waitFor r.recv()
  var rd = initReader(body, eLE, eXcdr2)
  doAssert rd.getU32() == 0x1000'u32 + uint32(i), "async: id mismatch"
echo "async loopback: 5 samples OK"

# negative frame vectors: length bounding + malformed reject
block:
  let WriterId = xrceObjectId(0x001, ObjkDataWriter)
  let first = writeFrame(0x80'u8, 0x01'u8, 1, WriterId, @[0xAA'u8, 0xBB'u8, 0xCC'u8])
  let second = writeFrame(0x80'u8, 0x01'u8, 2, WriterId, @[0xDD'u8, 0xEE'u8])
  let bounded = readFrame(first & second)
  doAssert bounded.isSome and bounded.get == @[0xAA'u8, 0xBB'u8, 0xCC'u8],
    "appended submessage must not leak into sample"
  var overlong = first
  overlong[6] = 0xFF'u8
  overlong[7] = 0xFF'u8
  doAssert readFrame(overlong).isNone, "over-long length must reject"
  doAssert readFrame(@[0x80'u8, 0x01'u8, 0x00'u8, 0x00'u8, 0x07'u8]).isNone,
    "truncated header must reject"
  # A refusal and no longer an empty `seq`: `Client.write` handed the empty one
  # to `deliver` and an empty datagram went out with nothing said.
  var refused = false
  try:
    discard writeFrame(0x80'u8, 0x01'u8, 1, WriterId, newSeq[byte](0x10000))
  except SampleTooLargeError:
    refused = true
  doAssert refused, "sample > 0xFFFF must be refused"
  # The bound is the field's and not one off it.
  doAssert writeFrame(0x80'u8, 0x01'u8, 1, WriterId,
      newSeq[byte](0xFFFF - XrceBaseSize)).len == 8 + 0xFFFF,
    "a maximal body must still frame"
echo "negative frame vectors: OK"

# The XCDR surface, against octets Cyclone, RTI, Fast DDS and omniORB wrote.
# Called from here and not from a test runner of its own, because `nim c -r
# test.nim` is the one command CI runs and a file nothing imports goes unrun.
surfaceTests()

# The DDS-XRCE 1.0 client: wire-encoder byte-identity + state-machine
# lifecycle rules, ported from crates/xrce-client.
xrceClientTests()

echo "ALL OK"
