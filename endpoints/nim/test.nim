# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 ZeroDDS Contributors
#
# Tests for the native Nim endpoint: byte-identity vs the Rust goldens, plus
# sync + async loopback. Run with `nim c -r test.nim` (GOLDEN_DIR=build).

import std/[os, options, asyncdispatch]
import ./zerodds

let goldenDir = getEnv("GOLDEN_DIR", "build")

proc fixture(endian: Endian): seq[byte] =
  var w = initWriter(endian)
  w.putU32(0xA1B2C3D4'u32)
  w.putU16(0x1234)
  w.putU8(0x5A)
  w.putF32(3.5'f32)
  w.putU64(0x0102030405060708'u64)
  w.putString("bay-12")
  w.putSeqU8(@[0xDE'u8, 0xAD, 0xBE, 0xEF])
  w.bytes()

proc sampleBody(id: uint32): seq[byte] =
  var w = initWriter(eLE)
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
let c = newClient(t)
for i in 0 .. 4:
  c.write(sampleBody(0x3000'u32 + uint32(i)))
for i in 0 .. 4:
  let body = c.poll()
  doAssert body.isSome, "sync: no sample"
  var rd = initReader(body.get, eLE)
  doAssert rd.getU32() == 0x3000'u32 + uint32(i), "sync: id mismatch"
echo "sync loopback: 5 samples OK"

# async loopback (asyncdispatch)
let t2 = memTransport()
let w = newClient(t2)
for i in 0 .. 4:
  w.write(sampleBody(0x1000'u32 + uint32(i)))
let r = newAsyncReader(t2)
for i in 0 .. 4:
  let body = waitFor r.recv()
  var rd = initReader(body, eLE)
  doAssert rd.getU32() == 0x1000'u32 + uint32(i), "async: id mismatch"
echo "async loopback: 5 samples OK"

echo "ALL OK"
