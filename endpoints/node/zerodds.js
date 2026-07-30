// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// Native pure-Node endpoint SDK (ADR 0013): a from-scratch XCDR wire-core in
// plain JavaScript, byte-identical to the Rust core and the other SDKs. Sync
// (poll) and async (an async iterator — the idiomatic Node model).

'use strict';

const LITTLE = 0;
const BIG = 1;

class Writer {
  constructor(endian) {
    this.buf = [];
    this.endian = endian;
  }
  _align(a) {
    const cap = a > 4 ? 4 : a;
    while (this.buf.length % cap !== 0) this.buf.push(0);
  }
  _putLE(a, le) {
    this._align(a);
    if (this.endian === BIG) {
      for (let i = le.length - 1; i >= 0; i--) this.buf.push(le[i]);
    } else {
      for (const b of le) this.buf.push(b);
    }
  }
  putU8(v) { this.buf.push(v & 0xff); }
  putU16(v) { this._putLE(2, [v & 0xff, (v >>> 8) & 0xff]); }
  putU32(v) { this._putLE(4, [v & 0xff, (v >>> 8) & 0xff, (v >>> 16) & 0xff, (v >>> 24) & 0xff]); }
  putU64(v) { // v: BigInt
    const le = [];
    for (let i = 0n; i < 8n; i++) le.push(Number((v >> (8n * i)) & 0xffn));
    this._putLE(4, le);
  }
  putF32(v) {
    const b = Buffer.alloc(4);
    b.writeFloatLE(v, 0); // logical little-endian bit pattern
    this._putLE(4, [b[0], b[1], b[2], b[3]]);
  }
  putBytes(b) { for (const x of b) this.buf.push(x & 0xff); }
  // Wire rule (docs/specs/zerodds-xcdr2-node-1.0.md §"string"): uint32 (UTF-8
  // byte count incl. NUL) + UTF-8 bytes + NUL. The length prefix is the BYTE
  // count of the UTF-8 encoding, not the JS string length (UTF-16 code units) —
  // multibyte characters occupy more than one byte. Byte-identical to the C/Rust
  // core.
  putString(s) {
    const raw = Buffer.from(s, 'utf8');
    this.putU32(raw.length + 1);
    this.putBytes(raw);
    this.putU8(0);
  }
  putSeqU8(b) { this.putU32(b.length); this.putBytes(b); }
  bytes() { return Buffer.from(this.buf); }
}

class Reader {
  constructor(buf, endian) {
    this.buf = buf;
    this.pos = 0;
    this.endian = endian;
  }
  _getLE(a, n) {
    const cap = a > 4 ? 4 : a;
    while (this.pos % cap !== 0) this.pos++;
    const out = Array.from(this.buf.subarray(this.pos, this.pos + n));
    this.pos += n;
    if (this.endian === BIG) out.reverse();
    return out;
  }
  getU32() {
    const le = this._getLE(4, 4);
    return (le[0] | (le[1] << 8) | (le[2] << 16) | (le[3] << 24)) >>> 0;
  }
  // --- primitives beyond getU32 — the byte-exact inverse of the Writer ---
  getU8() {
    const v = this.buf[this.pos];
    this.pos++;
    return v;
  }
  getU16() {
    const le = this._getLE(2, 2);
    return (le[0] | (le[1] << 8)) >>> 0;
  }
  getU64() {
    // returns BigInt (JS numbers lose precision above 2^53).
    const le = this._getLE(4, 8);
    let v = 0n;
    for (let i = 0; i < 8; i++) v |= BigInt(le[i]) << (8n * BigInt(i));
    return v;
  }
  getF32() {
    const le = this._getLE(4, 4);
    return Buffer.from(le).readFloatLE(0);
  }
  // Inverse of putString: read n bytes (n incl. the NUL), decode the first n-1
  // as UTF-8.
  getString() {
    const n = this.getU32();
    const s = this.buf.subarray(this.pos, this.pos + n - 1).toString('utf8');
    this.pos += n;
    return s;
  }
  getSeqU8() {
    const n = this.getU32();
    const b = Buffer.from(this.buf.subarray(this.pos, this.pos + n));
    this.pos += n;
    return b;
  }
}

// --- XRCE framing ---

const XRCE_SESSION_NOKEY = 0x80;
const XRCE_STREAM_BEST_EFFORT = 0x01;

// Returns null when the sample is larger than the 16-bit submessage_length
// field can describe (0xFFFF): refuse rather than truncate the length while
// copying the full payload (the C SDK rejects the same way).
function xrceWriteFrame(session, stream, seq, sample) {
  if (sample.length > 0xffff) return null;
  const out = Buffer.alloc(8 + sample.length);
  out[0] = session; out[1] = stream;
  out[2] = seq & 0xff; out[3] = (seq >>> 8) & 0xff;
  out[4] = 0x07; out[5] = 0x03;
  out[6] = sample.length & 0xff; out[7] = (sample.length >>> 8) & 0xff;
  Buffer.from(sample).copy(out, 8);
  return out;
}

// Direction (DDS-XRCE spec §8.3.5): WRITE_DATA (0x07) is client->agent — what
// this endpoint SENDS (xrceWriteFrame); DATA (0x09) is agent->client — what it
// RECEIVES from the hub. The reader accepts both (parity with the C/Python
// SDKs): 0x09 for real hub traffic, 0x07 for loopback. Returns null — never
// throws, never a bogus sample — on a short header, an unknown submessage id, or
// a declared length that runs past the datagram (truncated / wrong length); the
// body is bounded by the declared length so trailing bytes never leak in.
function xrceReadFrame(frame) {
  if (frame.length < 8) return null;
  if (frame[4] !== 0x09 && frame[4] !== 0x07) return null;
  const smLen = frame[6] | (frame[7] << 8);
  if (8 + smLen > frame.length) return null;
  return frame.subarray(8, 8 + smLen);
}

// --- sync client ---

class Client {
  constructor(transport, session = XRCE_SESSION_NOKEY, stream = XRCE_STREAM_BEST_EFFORT) {
    this.transport = transport;
    this.session = session;
    this.stream = stream;
    this.seq = 1;
  }
  write(sample) {
    const frame = xrceWriteFrame(this.session, this.stream, this.seq, sample);
    this.seq = (this.seq + 1) & 0xffff;
    return this.transport.deliver(frame);
  }
  // One non-blocking receive: returns the sample body, or null.
  poll() {
    const frame = this.transport.receive();
    return frame === null ? null : xrceReadFrame(frame);
  }
}

// --- async reader: an async iterator (idiomatic Node) ---

class AsyncReader {
  constructor(transport) {
    this.transport = transport;
    this._stop = false;
  }
  // for await (const body of reader.stream()) { ... }
  async *stream() {
    while (!this._stop) {
      const frame = this.transport.receive();
      if (frame === null) {
        await new Promise((r) => setTimeout(r, 1));
        continue;
      }
      const body = xrceReadFrame(frame);
      if (body) yield body;
    }
  }
  close() { this._stop = true; }
}

// A simple in-memory transport for tests + example.
class MemTransport {
  constructor() { this.q = []; }
  deliver(frame) { this.q.push(Buffer.from(frame)); return true; }
  receive() { return this.q.length ? this.q.shift() : null; }
}

module.exports = {
  LITTLE, BIG, Writer, Reader,
  XRCE_SESSION_NOKEY, XRCE_STREAM_BEST_EFFORT,
  xrceWriteFrame, xrceReadFrame,
  Client, AsyncReader, MemTransport,
};
