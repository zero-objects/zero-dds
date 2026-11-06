// smoke.test.mjs — WASM-Codec-Roundtrip im Node.
// Browser smoke (welt 4b 2nd half) would be identical via import "../pkg-web/...".

import test from "node:test";
import assert from "node:assert/strict";
import { CdrEncoder, CdrDecoder, version } from "../pkg-node/zerodds_ts_wasm.js";

test("wasm version string", () => {
  const v = version();
  assert.match(v, /^zerodds-wasm/);
  console.log("OK:", v);
});

test("wasm encode-decode big-endian", () => {
  const enc = new CdrEncoder(1); // big-endian
  enc.writeU32(0x01020304);
  const bytes = enc.finish();
  // BE: bytes are 01 02 03 04
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
