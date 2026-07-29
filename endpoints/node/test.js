// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// Tests for the native Node endpoint: byte-identity vs the Rust goldens, plus
// sync + async loopback. Run with `node --test` (GOLDEN_DIR=build).

'use strict';

const { test } = require('node:test');
const assert = require('node:assert');
const fs = require('node:fs');
const z = require('./zerodds');

const goldenDir = process.env.GOLDEN_DIR || 'build';

function fixture(w) {
  w.putU32(0xA1B2C3D4);
  w.putU16(0x1234);
  w.putU8(0x5A);
  w.putF32(3.5);
  w.putU64(0x0102030405060708n);
  w.putString('bay-12');
  w.putSeqU8(Buffer.from([0xDE, 0xAD, 0xBE, 0xEF]));
}

function sampleBody(id) {
  const w = new z.Writer(z.LITTLE);
  w.putU32(id);
  w.putU16(0);
  w.putU8(0);
  return w.bytes();
}

test('byte identity vs Rust goldens', () => {
  for (const [endian, file] of [[z.LITTLE, 'golden_le.bin'], [z.BIG, 'golden_be.bin']]) {
    const w = new z.Writer(endian);
    fixture(w);
    const golden = fs.readFileSync(`${goldenDir}/${file}`);
    assert.ok(w.bytes().equals(golden), `${file}: not byte-identical (got ${w.bytes().length}, want ${golden.length})`);
  }
});

test('sync loopback (pull)', () => {
  const t = new z.MemTransport();
  const c = new z.Client(t);
  for (let i = 0; i < 5; i++) assert.ok(c.write(sampleBody(0x3000 + i)));
  for (let i = 0; i < 5; i++) {
    const body = c.poll();
    assert.ok(body, `sync: no sample ${i}`);
    assert.strictEqual(new z.Reader(body, z.LITTLE).getU32(), 0x3000 + i);
  }
});

test('async loopback (async iterator)', async () => {
  const t = new z.MemTransport();
  const w = new z.Client(t);
  for (let i = 0; i < 5; i++) assert.ok(w.write(sampleBody(0x1000 + i)));
  const r = new z.AsyncReader(t);
  let got = 0;
  for await (const body of r.stream()) {
    assert.strictEqual(new z.Reader(body, z.LITTLE).getU32(), 0x1000 + got);
    if (++got === 5) break;
  }
  r.close();
  assert.strictEqual(got, 5);
});
