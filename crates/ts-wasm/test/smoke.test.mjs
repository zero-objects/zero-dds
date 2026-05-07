// smoke.test.mjs — WASM-Codec-Roundtrip im Node.
// Browser-Smoke (welt 4b 2nd half) waere identisch ueber import "../pkg-web/...".

import test from "node:test";
import assert from "node:assert/strict";
import { CdrEncoder, CdrDecoder, version } from "../pkg-node/dds_ts_wasm.js";

test("wasm version string", () => {
  const v = version();
  assert.match(v, /^zerodds-wasm/);
  console.log("OK:", v);
});

test("wasm encode-decode primitive roundtrip (LE)", () => {
  const enc = new CdrEncoder(0); // little-endian
  enc.writeU8(0x42);
  enc.align(2);
  enc.writeU16(0xbeef);
  enc.align(4);
  enc.writeU32(0xcafebabe);
  enc.align(8);
  enc.writeU64(0x1122334455667788n);
  enc.writeString("Hallo, ZeroDDS!");
  const bytes = enc.finish();
  console.log("encoded", bytes.length, "bytes");

  const dec = new CdrDecoder(bytes, 0);
  assert.equal(dec.readU8(), 0x42);
  assert.equal(dec.readU16(), 0xbeef);
  assert.equal(dec.readU32(), 0xcafebabe);
  assert.equal(dec.readU64(), 0x1122334455667788n);
  assert.equal(dec.readString(), "Hallo, ZeroDDS!");
});

test("wasm encode-decode big-endian", () => {
  const enc = new CdrEncoder(1); // big-endian
  enc.writeU32(0x01020304);
  const bytes = enc.finish();
  // BE: bytes sind 01 02 03 04
  assert.deepEqual(Array.from(bytes), [0x01, 0x02, 0x03, 0x04]);
  const dec = new CdrDecoder(bytes, 1);
  assert.equal(dec.readU32(), 0x01020304);
});

test("wasm encode bytes blob", () => {
  const enc = new CdrEncoder(0);
  const payload = new Uint8Array([0xde, 0xad, 0xbe, 0xef]);
  enc.writeBytes(payload);
  const bytes = enc.finish();
  assert.deepEqual(Array.from(bytes), [0xde, 0xad, 0xbe, 0xef]);
});
