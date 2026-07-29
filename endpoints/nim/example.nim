# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 ZeroDDS Contributors
#
# Runnable example for the native Nim endpoint: sync (poll) and async
# (asyncdispatch Future). Run with `nim c -r example.nim`.

import std/[options, asyncdispatch, strutils]
import ./zerodds

proc sample(id: uint32, label: string): seq[byte] =
  var w = initWriter(eLE)
  w.putU32(id)
  w.putString(label)
  w.bytes()

# sync
let t = memTransport()
let c = newClient(t)
c.write(sample(0x42'u32, "sync-hello"))
let body = c.poll()
if body.isSome:
  var rd = initReader(body.get, eLE)
  echo "sync: received id=0x", toHex(rd.getU32())

# async
let t2 = memTransport()
let w = newClient(t2)
for i in 0 .. 2:
  w.write(sample(0x100'u32 + uint32(i), "async"))
let r = newAsyncReader(t2)
for _ in 0 .. 2:
  let b = waitFor r.recv()
  var rd = initReader(b, eLE)
  echo "async: received id=0x", toHex(rd.getU32())

echo "ALL OK"
