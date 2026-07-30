# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 ZeroDDS Contributors
#
# Deeper ASYNC example for the native Nim endpoint: the same telemetry publisher,
# but the subscriber consumes via the asyncdispatch Future (await) and decodes
# every field. Run: `nim c -r example_async.nim`

import std/[options, asyncdispatch, strformat]
import ./zerodds

type Reading = object
  id: uint32
  value: float32
  label: string

proc marshal(r: Reading, endian: Endian): seq[byte] =
  var w = initWriter(endian)
  w.putU32(r.id)
  w.putF32(r.value)
  w.putString(r.label)
  w.bytes()

proc decodeReading(body: seq[byte]): Reading =
  var rd = initReader(body, eLE)
  Reading(id: rd.getU32(), value: rd.getF32(), label: rd.getString())

proc main() {.async.} =
  const total = 5
  let t = memTransport()
  let w = newClient(t)

  # Publisher.
  for i in 0 ..< total:
    let r = Reading(id: 0x2000'u32 + uint32(i), value: 100.0'f32 - float32(i),
                    label: &"sensor-{i:02}")
    w.write(r.marshal(eLE))

  # Subscriber: await the AsyncReader Future per sample; decode; stop at `total`.
  let reader = newAsyncReader(t)
  for got in 0 ..< total:
    let body = await reader.recv()
    let r = decodeReading(body)
    echo &"async reading {got}: id=0x{r.id:x} value={r.value:.1f} label=\"{r.label}\""

  echo "ALL OK"

waitFor main()
