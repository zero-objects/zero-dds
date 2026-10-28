# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 ZeroDDS Contributors
#
# Reliable sender app for the live E2E test. The producer enqueues samples into
# a single-producer single-consumer ring (`spsc_ring.nim`; the decoupled path —
# it never touches the socket); a drain Thread owns the reliable Sender state +
# the UDP socket, sends WRITE_DATA, emits HEARTBEAT, and retransmits on ACKNACK
# until the send window drains.
#
# argv: <peer-port> <N> [run|bench]
#
# The ring takes no lock and allocates nothing of its own: an enqueue is two
# atomic loads, a move and an atomic store. It replaced the stdlib `Channel`,
# which takes a lock and copies on every send. The bench prints the enqueue
# time beside an inline `sendto`, and how often the producer found the ring
# full and waited.

import std/[net, nativesockets, options, os, strutils, times]
import ./reliable
import ./spsc_ring

proc toStr(b: seq[byte]): string =
  result = newString(b.len)
  for i, x in b:
    result[i] = char(x)

proc toBytes(s: string, n: int): seq[byte] =
  result = newSeq[byte](n)
  for i in 0 ..< n:
    result[i] = byte(s[i])

proc nowMs(): int = int(epochTime() * 1000.0)

# Producer → drain ring; empty seq = "no more samples" sentinel.
const QueueSlots = 4096
type
  Queue = SpscRing[QueueSlots]
  DrainArgs = tuple[port: int, queue: ptr Queue]

proc drain(args: DrainArgs) {.thread.} =
  let queue = args.queue
  var sock = newSocket(AF_INET, SOCK_DGRAM, IPPROTO_UDP)
  sock.connect("127.0.0.1", Port(args.port))
  var sender = newSender()
  var msgSeq = 1
  var closed = false
  let startMs = nowMs()
  while true:
    # 1. pull enqueued samples: submit + send WRITE_DATA
    var msg: seq[byte]
    while queue[].tryPop(msg):
      if msg.len == 0:
        closed = true
      else:
        let sub = sender.submit(msg)
        if sub.status == srOk:
          sock.send(toStr(reliableWriteFrame(sub.seq, 0x0015'u16, msg)))
    # 2. done when closed and the window has drained
    if closed and sender.inFlightCount == 0:
      break
    # 3. safety valve (peer's own deadline is 30 s)
    if nowMs() - startMs > 20000:
      break
    # 4. period-gated HEARTBEAT
    let hb = sender.pendingHeartbeat(nowMs())
    if hb.isSome:
      sock.send(toStr(heartbeatFrame(SessionNoKey, StreamNone, msgSeq, hb.get)))
      inc msgSeq
    # 5. drain incoming ACKNACKs (bounded, short timeout)
    for _ in 0 ..< 64:
      var buf = newString(2048)
      var got = false
      try:
        let n = sock.recv(buf, 2048, timeout = 20)
        if n > 0:
          let ack = parseAcknack(toBytes(buf, n))
          if ack.isSome:
            sender.recvAcknack(ack.get)
          got = true
      except TimeoutError:
        discard
      except OSError:
        discard
      if not got: break
    # 6. retransmit whatever is still in-flight (peer marked it missing)
    if sender.inFlightCount > 0:
      var pending: seq[tuple[seq: uint16, payload: seq[byte]]] = @[]
      for pr in sender.inFlightPairs:
        pending.add(pr)
      for pr in pending:
        sock.send(toStr(reliableWriteFrame(pr.seq, 0x0015'u16, pr.payload)))
  sock.close()

proc runReliable(port, n: int) =
  let queue = createShared(Queue)
  var t: Thread[DrainArgs]
  createThread(t, drain, (port, queue))
  for i in 0 ..< n:
    let v = uint32(i)
    discard queue[].push(@[byte(v and 0xff), byte((v shr 8) and 0xff),
                           byte((v shr 16) and 0xff), byte((v shr 24) and 0xff)])
  discard queue[].push(@[]) # sentinel
  joinThread(t)
  freeShared(queue)
  echo "SENT ", n

proc sink(queue: ptr Queue) {.thread.} =
  var msg: seq[byte]
  while true:
    if queue[].tryPop(msg) and msg.len == 0:
      break

proc runBench(port, n: int) =
  let iters = 20000
  let sample = @[0'u8, 0, 0, 0]
  # inline: producer does the send syscall itself
  var sock = newSocket(AF_INET, SOCK_DGRAM, IPPROTO_UDP)
  sock.connect("127.0.0.1", Port(port))
  var t0 = epochTime()
  for i in 0 ..< iters:
    sock.send(toStr(reliableWriteFrame(uint16(i and 0xffff), 0x0015'u16, sample)))
  let inlineNs = (epochTime() - t0) * 1e9 / float(iters)
  sock.close()
  # decoupled: producer only enqueues (a drain thread would do the I/O)
  let queue = createShared(Queue)
  var t: Thread[ptr Queue]
  createThread(t, sink, queue)
  var fullWaits = 0
  t0 = epochTime()
  for i in 0 ..< iters:
    fullWaits += queue[].push(sample)
  let enqueueNs = (epochTime() - t0) * 1e9 / float(iters)
  discard queue[].push(@[])
  joinThread(t)
  freeShared(queue)
  echo "BENCH enqueue_ns=", enqueueNs.int, " inline_send_ns=", inlineNs.int,
    " full_waits=", fullWaits

proc main() =
  if paramCount() < 2:
    quit("usage: reliable_app <port> <N> [run|bench]", 2)
  let port = parseInt(paramStr(1))
  let n = parseInt(paramStr(2))
  let mode = if paramCount() >= 3: paramStr(3) else: "run"
  if mode == "bench":
    runBench(port, n)
  else:
    runReliable(port, n)

main()
