// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// Runnable example for the native Node endpoint SDK: sync (poll) and async
// (async iterator) over an in-memory transport.  node example.js
'use strict';

const z = require('./zerodds');

function sample(id, label) {
  const w = new z.Writer(z.LITTLE, z.XCDR2);
  w.putU32(id);
  w.putString(label);
  return w.bytes();
}

// How long the async part waits for its samples; a missing one ends the run
// with an error instead of an endless wait.
const TIMEOUT_MS = 10000;

async function main() {
  // --- sync ---
  const t = new z.MemTransport();
  const c = new z.Client(t, z.xrceObjectId(0x001, z.OBJK_DATAWRITER));
  c.write(sample(0x42, 'sync-hello'));
  const body = c.poll();
  if (!body) {
    console.error('sync: the sample did not arrive');
    process.exit(1);
  }
  console.log('sync: received id=0x' + new z.Reader(body, z.LITTLE, z.XCDR2).getU32().toString(16));

  // --- async (async iterator) ---
  const t2 = new z.MemTransport();
  const w = new z.Client(t2, z.xrceObjectId(0x001, z.OBJK_DATAWRITER));
  for (let i = 0; i < 3; i++) w.write(sample(0x100 + i, 'async'));
  const r = new z.AsyncReader(t2);
  let n = 0;
  const deadline = setTimeout(() => {
    console.error(`async: got ${n} of 3 samples within ${TIMEOUT_MS / 1000} s`);
    process.exit(1);
  }, TIMEOUT_MS);
  for await (const b of r.stream()) {
    console.log('async: received id=0x' + new z.Reader(b, z.LITTLE, z.XCDR2).getU32().toString(16));
    if (++n === 3) break;
  }
  clearTimeout(deadline);
  r.close();

  console.log('ALL OK');
}

main();
