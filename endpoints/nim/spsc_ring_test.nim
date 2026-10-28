# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 ZeroDDS Contributors
#
# The ring `reliable_app.nim` decouples its producer through: full and empty
# answered on one thread, then every sample across two threads in order and
# without a gap.

import ./spsc_ring

var failed = 0
proc check(cond: bool, name: string) =
  if cond:
    echo "ok   ", name
  else:
    echo "FAIL ", name
    inc failed

proc word(i: int): seq[byte] =
  @[byte(i and 0xff), byte((i shr 8) and 0xff),
    byte((i shr 16) and 0xff), byte((i shr 24) and 0xff)]

proc unword(s: seq[byte]): int =
  int(s[0]) or (int(s[1]) shl 8) or (int(s[2]) shl 16) or (int(s[3]) shl 24)

# --- one thread: the two edges ---------------------------------------------

block:
  let ring = createShared(SpscRing[4])
  var popped: seq[byte]
  check(not ring[].tryPop(popped), "an empty ring pops nothing")
  var accepted = 0
  for i in 0 ..< 5:
    var v = word(i)
    if ring[].tryPush(v):
      inc accepted
    else:
      check(v == word(i), "a refused push leaves the sample with the caller")
  check(accepted == 4, "four slots take four samples and refuse the fifth")
  var order = true
  for i in 0 ..< 4:
    order = order and ring[].tryPop(popped) and unword(popped) == i
  check(order, "samples come out in the order they went in")
  check(not ring[].tryPop(popped), "and the ring is empty again")
  # Past the end of the slot array: the indices wrap and the order holds.
  var wrapped = true
  for i in 0 ..< 10:
    var v = word(100 + i)
    wrapped = wrapped and ring[].tryPush(v) and ring[].tryPop(popped) and
      unword(popped) == 100 + i
  check(wrapped, "the indices wrap past the capacity")
  freeShared(ring)

# --- two threads: order and completeness ------------------------------------

const Samples = 200_000
type
  Ring = SpscRing[64]
  Tally = object
    received: int
    outOfOrder: int

var consumer: Thread[tuple[ring: ptr Ring, tally: ptr Tally]]

proc consume(args: tuple[ring: ptr Ring, tally: ptr Tally]) {.thread.} =
  var msg: seq[byte]
  var expected = 0
  while expected < Samples:
    if args.ring[].tryPop(msg):
      if unword(msg) != expected:
        inc args.tally.outOfOrder
      inc expected
  args.tally.received = expected

block:
  # Sixty-four slots against two hundred thousand samples: the producer
  # finds the ring full many times, which is the path `push` waits on.
  let ring = createShared(Ring)
  let tally = createShared(Tally)
  createThread(consumer, consume, (ring, tally))
  var fullWaits = 0
  for i in 0 ..< Samples:
    fullWaits += ring[].push(word(i))
  joinThread(consumer)
  check(tally.received == Samples, "every sample crosses the threads")
  check(tally.outOfOrder == 0, "in the order it was pushed, with no gap")
  echo "     full_waits=", fullWaits
  freeShared(tally)
  freeShared(ring)

if failed == 0:
  echo "ALL OK"
  quit(0)
else:
  echo failed, " FAILED"
  quit(1)
