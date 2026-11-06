// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// example_reliable app: argv = <peer-port> [N]. Submits N samples through the
// AsyncReliableWriter (reliable.js) -- the producer path is a pure async enqueue
// (Promise-based backpressure, no socket syscall); a drain loop owns the
// reliable sender state, sends WRITE_DATA over a real UDP socket, fires
// HEARTBEAT on a timer, and retransmits on ACKNACK until the send window drains.
// The peer (zerodds-endpoint-e2e's bind_reliable_peer/reliable_receive) injects
// loss and replies ACKNACK; loss recovery is proven when every sample lands
// gap-free and in order on the peer. Run: `node example_reliable.js <port> [N]`
//
// `node example_reliable.js <port> 0 bench`: producer latency, the decoupled
// enqueue (`offer`, no socket call) against the inline send of the same frame
// on the producer (reliable-endpoint 1.0 §5 point 4). Prints one `BENCH` line.

'use strict';

const dgram = require('dgram');
const { AsyncReliableWriter, writeFrame } = require('./reliable');

// The DataWriter of every frame's BaseObjectRequest (DDS-XRCE 1.0 §7.7.8).
const WRITER_ID = 0x0015;

// Below the writer's queue capacity, so no enqueue meets backpressure: what is
// measured is the producer's own cost, not the drain.
const BENCH_ITERS = 4000;

async function bench(port) {
  const sample = Buffer.from([0, 0, 0, 0]);

  // Inline: the producer frames the sample and sends it itself. Bound and
  // connected first, so every send goes out at once instead of waiting for an
  // implicit bind.
  const sock = dgram.createSocket('udp4');
  await new Promise((resolve) => sock.bind(0, '127.0.0.1', resolve));
  await new Promise((resolve) => sock.connect(port, '127.0.0.1', resolve));
  const t0 = process.hrtime.bigint();
  for (let i = 0; i < BENCH_ITERS; i++) {
    sock.send(writeFrame(i & 0xffff, WRITER_ID, sample));
  }
  const inlineNs = Number(process.hrtime.bigint() - t0) / BENCH_ITERS;
  await new Promise((resolve) => sock.close(resolve));

  // Decoupled: the producer only enqueues; the drain loop is not started, so
  // nothing but the enqueue runs on this thread.
  const writer = new AsyncReliableWriter('127.0.0.1', port, WRITER_ID);
  const t1 = process.hrtime.bigint();
  for (let i = 0; i < BENCH_ITERS; i++) {
    if (!writer.offer(sample)) throw new Error(`enqueue ${i} met a full queue`);
  }
  const enqueueNs = Number(process.hrtime.bigint() - t1) / BENCH_ITERS;
  writer.close();

  console.log(`BENCH enqueue_ns=${enqueueNs.toFixed(1)} inline_send_ns=${inlineNs.toFixed(1)}`);
}

async function main() {
  const port = parseInt(process.argv[2], 10);
  if (process.argv[4] === 'bench') {
    await bench(port);
    return;
  }
  const n = process.argv[3] ? parseInt(process.argv[3], 10) : 12;

  const writer = new AsyncReliableWriter('127.0.0.1', port, WRITER_ID);
  writer.start();

  for (let i = 0; i < n; i++) {
    // Sample i = its index as 4-byte little-endian (the peer/test decodes it).
    const sample = Buffer.from([i & 0xff, (i >> 8) & 0xff, (i >> 16) & 0xff, (i >> 24) & 0xff]);
    await writer.submit(sample);
  }

  const ok = await writer.finish(25000);
  writer.close();
  if (ok) {
    console.error(`RELIABLE OK: all ${n} acknowledged (delivered=${writer.delivered()})`);
    process.exit(0);
  } else {
    const err = writer.drainError();
    console.error(`RELIABLE INCOMPLETE${err ? `: ${err}` : ''}`);
    process.exit(1);
  }
}

main();
