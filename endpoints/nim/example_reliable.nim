# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 ZeroDDS Contributors
#
# Runnable reliable-stream demo (in-process, no sockets): an aggregator submits
# N samples over a lossy link (every 3rd dropped on first pass); the receiver
# recovers the gaps via ACKNACK + retransmit and prints the contiguous sequence
# it actually delivered. Run: nim c -r example_reliable.nim

import std/[options, strutils]
import ./reliable

proc main() =
  const N = 12
  var sender = newSender()
  var receiver = newReceiver()
  for i in 0 ..< N:
    discard sender.submit(@[byte(i)])

  var delivered: seq[int] = @[]
  var pass = 0
  var droppedOnce: seq[uint16] = @[]
  while delivered.len < N:
    var frames: seq[tuple[seq: uint16, payload: seq[byte]]] = @[]
    for pr in sender.inFlightPairs:
      frames.add(pr)
    var idx = 0
    for f in frames:
      inc idx
      if pass == 0 and idx mod 3 == 0 and f.seq notin droppedOnce:
        droppedOnce.add(f.seq) # simulate loss on the first pass
        continue
      discard receiver.recvData(f.seq, f.payload)
    for got in receiver.drainInOrder():
      delivered.add(int(got.payload[0]))
    # receiver → ACKNACK; sender purges acked, keeps missing for retransmit
    sender.recvAcknack(receiver.pendingAcknack(none(uint16)))
    inc pass

  var order: seq[string] = @[]
  for v in delivered:
    order.add($v)
  echo "delivered: ", order.join(" ")
  echo "dropped on first pass: ", droppedOnce.len, " — recovered after ", pass, " passes"
  var ok = delivered.len == N
  for i in 0 ..< N:
    if delivered[i] != i: ok = false
  echo (if ok: "RELIABLE OK" else: "RELIABLE FAIL")

main()
