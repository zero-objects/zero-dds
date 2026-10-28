# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 ZeroDDS Contributors
#
# A single-producer single-consumer ring for the decoupled send path
# (zerodds-endpoint-nim-1.0 §5): the producer hands a sample to the drain
# thread without a lock, without a syscall and without an allocation of the
# queue's own.
#
# `tryPush` and `tryPop` each finish in a bounded number of steps whatever
# the other thread is doing: a relaxed load of the index this side owns, an
# acquire load of the other side's, a move, a release store. The release
# store of `tail` publishes the slot to the consumer; the release store of
# `head` hands it back to the producer. Neither side writes the other's
# index, so there is nothing to retry.
#
# Bounded, so it can be full. `push` then waits for the drain thread to free
# a slot: that is backpressure, and the one place the producer waits. It
# spins briefly and then gives the CPU up, because on a loaded host the
# drain thread may be waiting for exactly that CPU. It counts how often it
# found the ring full, and the bench prints the count beside the enqueue
# time.
#
# A slot holds a `seq[byte]` that is moved in and moved out, so the payload
# crosses the threads without a copy. The ring itself must sit in memory both
# threads reach — `createShared` — and not in a GC'd global.

import std/[atomics, os]

type
  SpscRing*[N: static int] = object
    ## `N` slots, a power of two. `head` and `tail` sit on cache lines of
    ## their own: the two threads write one each, and a shared line would
    ## bounce between the cores on every operation.
    head {.align(64).}: Atomic[int] ## the next slot to pop; the consumer's
    tail {.align(64).}: Atomic[int] ## the next slot to push; the producer's
    slots {.align(64).}: array[N, seq[byte]]

proc tryPush*[N: static int](r: var SpscRing[N], v: var seq[byte]): bool =
  ## Moves `v` into the ring and answers true, or answers false and leaves
  ## `v` alone when the ring is full. Producer side only.
  static: doAssert N > 0 and (N and (N - 1)) == 0, "the capacity is a power of two"
  let t = r.tail.load(moRelaxed)
  if t - r.head.load(moAcquire) == N:
    return false
  r.slots[t and (N - 1)] = move(v)
  r.tail.store(t + 1, moRelease)
  true

proc push*[N: static int](r: var SpscRing[N], v: sink seq[byte]): int =
  ## Moves `v` into the ring, waiting while it is full. Answers how many
  ## times it found it full. Producer side only.
  var v = v
  while not r.tryPush(v):
    inc result
    if result mod 64 == 0:
      sleep(0)
    else:
      cpuRelax()

proc tryPop*[N: static int](r: var SpscRing[N], v: var seq[byte]): bool =
  ## Moves the oldest sample into `v` and answers true, or answers false when
  ## the ring is empty. Consumer side only.
  let h = r.head.load(moRelaxed)
  if h == r.tail.load(moAcquire):
    return false
  v = move(r.slots[h and (N - 1)])
  r.head.store(h + 1, moRelease)
  true
