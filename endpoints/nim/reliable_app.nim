# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 ZeroDDS Contributors
#
# Reliable sender app for the live E2E test. The producer enqueues samples into
# a Channel (the decoupled path — it never touches the socket); a drain Thread
# owns the reliable Sender state + the UDP socket, sends WRITE_DATA, emits
# HEARTBEAT, and retransmits on ACKNACK until the send window drains.
#
# argv: <peer-port> <N> [run|bench]
#
# Nim's stdlib `Channel` uses a lock (not a wait-free ring), so the enqueue
# still pays a lock+copy; it does keep the producer off the send syscall, which
# is what the latency bench measures. A true wait-free ring would improve the
# enqueue figure further — reported honestly.

import std/[net, nativesockets, options, os, strutils, times]
import ./reliable

proc toStr(b: seq[byte]): string =
  result = newString(b.len)
  for i, x in b:
    result[i] = char(x)

proc toBytes(s: string, n: int): seq[byte] =
  result = newSeq[byte](n)
  for i in 0 ..< n:
    result[i] = byte(s[i])

proc nowMs(): int = int(epochTime() * 1000.0)

# Producer → drain channel; empty seq = "no more samples" sentinel.
var gChan: Channel[seq[byte]]

proc drain(port: int) {.thread.} =
  var sock = newSocket(AF_INET, SOCK_DGRAM, IPPROTO_UDP)
  sock.connect("127.0.0.1", Port(port))
  var sender = newSender()
  var msgSeq = 1
  var closed = false
  let startMs = nowMs()
  while true:
    # 1. pull enqueued samples: submit + send WRITE_DATA
    while true:
      let tr = gChan.tryRecv()
      if not tr.dataAvailable: break
      if tr.msg.len == 0:
        closed = true
      else:
        let sub = sender.submit(tr.msg)
        if sub.status == srOk:
          sock.send(toStr(reliableWriteFrame(sub.seq, tr.msg)))
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
        sock.send(toStr(reliableWriteFrame(pr.seq, pr.payload)))
  sock.close()

proc runReliable(port, n: int) =
  gChan.open()
  var t: Thread[int]
  createThread(t, drain, port)
  for i in 0 ..< n:
    let v = uint32(i)
    gChan.send(@[byte(v and 0xff), byte((v shr 8) and 0xff),
                 byte((v shr 16) and 0xff), byte((v shr 24) and 0xff)])
  gChan.send(@[]) # sentinel
  joinThread(t)
  gChan.close()
  echo "SENT ", n

var gSinkStop: bool

proc sink(unused: int) {.thread.} =
  while true:
    let tr = gChan.tryRecv()
    if tr.dataAvailable and tr.msg.len == 0:
      break

proc runBench(port, n: int) =
  let iters = 20000
  let sample = @[0'u8, 0, 0, 0]
  # inline: producer does the send syscall itself
  var sock = newSocket(AF_INET, SOCK_DGRAM, IPPROTO_UDP)
  sock.connect("127.0.0.1", Port(port))
  var t0 = epochTime()
  for i in 0 ..< iters:
    sock.send(toStr(reliableWriteFrame(uint16(i and 0xffff), sample)))
  let inlineNs = (epochTime() - t0) * 1e9 / float(iters)
  sock.close()
  # decoupled: producer only enqueues (a drain thread would do the I/O)
  gChan.open()
  var t: Thread[int]
  createThread(t, sink, 0)
  t0 = epochTime()
  for i in 0 ..< iters:
    gChan.send(sample)
  let enqueueNs = (epochTime() - t0) * 1e9 / float(iters)
  gChan.send(@[])
  joinThread(t)
  gChan.close()
  echo "BENCH enqueue_ns=", enqueueNs.int, " inline_send_ns=", inlineNs.int

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
