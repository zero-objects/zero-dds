// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// Deeper ASYNC example for the native Node endpoint: the same telemetry
// publisher, but the subscriber consumes via the async iterator (`for await`)
// and decodes every field. Run: `node example_async.js`

'use strict';

const z = require('./zerodds');

class Reading {
  constructor(id, value, label) {
    this.id = id;
    this.value = value;
    this.label = label;
  }
  marshal(endian) {
    const w = new z.Writer(endian);
    w.putU32(this.id);
    w.putF32(this.value);
    w.putString(this.label);
    return w.bytes();
  }
}

function decodeReading(body) {
  const r = new z.Reader(body, z.LITTLE);
  return new Reading(r.getU32(), r.getF32(), r.getString());
}

async function main() {
  const total = 5;
  const t = new z.MemTransport();
  const w = new z.Client(t);

  // Publisher.
  for (let i = 0; i < total; i++) {
    const label = `sensor-${String(i).padStart(2, '0')}`;
    w.write(new Reading(0x2000 + i, 100.0 - i, label).marshal(z.LITTLE));
  }

  // Subscriber: iterate the async stream; decode; stop at total.
  const reader = new z.AsyncReader(t);
  let got = 0;
  for await (const body of reader.stream()) {
    const r = decodeReading(body);
    console.log(`async reading ${got}: id=0x${r.id.toString(16)} value=${r.value.toFixed(1)} label="${r.label}"`);
    got++;
    if (got === total) {
      reader.close();
      break;
    }
  }
  console.log('ALL OK');
}

main();
