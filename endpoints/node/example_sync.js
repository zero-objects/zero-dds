// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// Deeper SYNC example for the native Node endpoint: a sensor-telemetry publisher
// writes typed Reading samples; a subscriber polls and decodes every field.
// Run: `node example_sync.js`

'use strict';

const z = require('./zerodds');

// Reading mirrors an IDL `@final struct Reading { uint32 id; float value; string label; }`.
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

const total = 5;
const t = new z.MemTransport();
const c = new z.Client(t);

// Publisher: frame + deliver 5 typed readings with varying values.
for (let i = 0; i < total; i++) {
  const label = `bay-${String(i).padStart(2, '0')}`;
  c.write(new Reading(0x1000 + i, 20.0 + i * 0.5, label).marshal(z.LITTLE));
}

// Subscriber: poll; decode every field; stop at total.
let got = 0;
while (got < total) {
  const body = c.poll();
  if (body === null) break;
  const r = decodeReading(body);
  console.log(`sync reading ${got}: id=0x${r.id.toString(16)} value=${r.value.toFixed(1)} label="${r.label}"`);
  got++;
}
if (got !== total) {
  console.log('incomplete');
  process.exit(1);
}
console.log('ALL OK');
