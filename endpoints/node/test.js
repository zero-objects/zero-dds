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

// Builds an XRCE frame with an arbitrary submessage id + declared body length,
// for exercising the reader's direction + validation logic.
function frame(id, body, declaredLen = body.length) {
  const out = Buffer.alloc(8 + body.length);
  out[0] = 0x80; out[1] = 0x01; out[2] = 1; out[3] = 0;
  out[4] = id; out[5] = 0x03;
  out[6] = declaredLen & 0xff; out[7] = (declaredLen >>> 8) & 0xff;
  Buffer.from(body).copy(out, 8);
  return out;
}

test('string is UTF-8 with byte-count length prefix', () => {
  // "grüße": g r [ü=C3 BC] [ß=C3 9F] e = 7 UTF-8 bytes, but only 5 chars.
  // The length prefix must be the BYTE count + 1 (NUL), not the char count.
  const s = 'grüße';
  const utf8 = [0x67, 0x72, 0xC3, 0xBC, 0xC3, 0x9F, 0x65];
  const w = new z.Writer(z.LITTLE);
  w.putString(s);
  const wire = w.bytes();
  // 4-byte LE length prefix = byte count incl. NUL = 8, NOT the char count (5).
  assert.strictEqual(wire.readUInt32LE(0), utf8.length + 1, 'length prefix must be UTF-8 byte count + 1');
  // UTF-8 body then NUL — byte-identical to the C/Rust core.
  assert.deepStrictEqual(Array.from(wire.subarray(4, 4 + utf8.length)), utf8, 'UTF-8 body');
  assert.strictEqual(wire[4 + utf8.length], 0, 'NUL terminator');
  assert.strictEqual(wire.length, 4 + utf8.length + 1, 'total wire length');
  // Symmetric decode.
  assert.strictEqual(new z.Reader(wire, z.LITTLE).getString(), s, 'roundtrip');
});

test('reader accepts hub DATA (0x09)', () => {
  // Hub -> endpoint direction is DATA (0x09); the reader must accept it.
  const body = Buffer.from([0xAA, 0xBB, 0xCC]);
  const got = z.xrceReadFrame(frame(0x09, body));
  assert.ok(got && Buffer.from(got).equals(body), 'DATA/0x09 body');
});

test('reader accepts loopback WRITE_DATA (0x07)', () => {
  const body = Buffer.from([0x01, 0x02]);
  const got = z.xrceReadFrame(frame(0x07, body));
  assert.ok(got && Buffer.from(got).equals(body), 'WRITE_DATA/0x07 body');
});

test('reader rejects negative frame vectors', () => {
  // Unknown submessage id.
  assert.strictEqual(z.xrceReadFrame(frame(0x0A, Buffer.from([1, 2]))), null);
  // Truncated header (fewer than 8 bytes).
  for (let len = 0; len < 8; len++) {
    assert.strictEqual(z.xrceReadFrame(Buffer.alloc(len)), null, `len ${len}`);
  }
  // Declared length runs past the datagram.
  assert.strictEqual(z.xrceReadFrame(frame(0x09, Buffer.from([1, 2]), 100)), null);
  // Trailing bytes past the declared length must not leak into the body.
  const bounded = z.xrceReadFrame(frame(0x09, Buffer.from([1, 2, 3, 4]), 2));
  assert.ok(bounded && Buffer.from(bounded).equals(Buffer.from([1, 2])), 'bounded body');
});

test('writer refuses a sample larger than the 16-bit length field', () => {
  // Refuse (null) rather than silently truncate the length (matches the C SDK).
  assert.strictEqual(z.xrceWriteFrame(0x80, 0x01, 1, Buffer.alloc(0x10000)), null);
  assert.ok(z.xrceWriteFrame(0x80, 0x01, 1, Buffer.alloc(0xffff)) !== null, '0xFFFF still encodes');
});
