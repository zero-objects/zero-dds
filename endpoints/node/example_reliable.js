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

'use strict';

const { AsyncReliableWriter } = require('./reliable');

async function main() {
  const port = parseInt(process.argv[2], 10);
  const n = process.argv[3] ? parseInt(process.argv[3], 10) : 12;

  const writer = new AsyncReliableWriter('127.0.0.1', port);
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
