# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 ZeroDDS Contributors
#
# Deeper SYNC example for the native Nim endpoint: a sensor-telemetry publisher
# writes typed Reading samples; a subscriber polls with a timeout and decodes
# every field. Run: `nim c -r example_sync.nim`

import std/[options, strformat]
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

const total = 5
let t = memTransport()
let c = newClient(t)

# Publisher: frame + deliver 5 typed readings with varying values.
for i in 0 ..< total:
  let r = Reading(id: 0x1000'u32 + uint32(i), value: 20.0'f32 + float32(i) * 0.5'f32,
                  label: &"bay-{i:02}")
  c.write(r.marshal(eLE))

# Subscriber: poll with a bounded retry; decode every field; stop at `total`.
var got = 0
var tries = 0
while got < total and tries < 1000:
  let body = c.poll()
  if body.isSome:
    let r = decodeReading(body.get)
    echo &"sync reading {got}: id=0x{r.id:x} value={r.value:.1f} label=\"{r.label}\""
    inc got
  else:
    inc tries
if got != total:
  quit("incomplete: got " & $got & "/" & $total, 1)
echo "ALL OK"
