// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// reliable.js -- XRCE reliable-stream endpoint (DDS-XRCE 1.0 §8.4.10/§8.4.11)
// for the native pure-Node SDK (ADR 0013). Byte-identical to endpoints/c
// (zerodds_endpoint.c / zerodds_reliable_async.c), endpoints/java
// (org.zerodds.endpoint.*) and crates/xrce: WRITE_DATA carries the RFC-1982
// sample sequence in the 2-byte header field; HEARTBEAT/ACKNACK use the golden
// control-frame layout (session 0x80, stream NONE, msg-seq 1) that
// endpoints/golden-gen emits. The peer inspects only byte[4] (submessage id)
// and the body at offset 8.
//
// Frame = 8-byte header + body, little-endian:
//   [0]=session [1]=stream [2..4)=seq LE [4]=submsg id [5]=flags
//   [6..8)=body-len LE [8..)=body
//
// The AsyncReliableWriter mirrors the C SPSC-ring/drain-thread and the Java
// AsyncReliableWriter, but in the idiomatic Node model: the producer `submit`
// is an async enqueue into a bounded queue (Promise-based backpressure on
// overflow -- no sample is ever dropped and no producer hangs forever), a
// single drain "loop" (event-loop driven, not a thread) owns the ReliableSender
// state, frames + sends WRITE_DATA over a real `dgram` socket, fires HEARTBEAT
// on a period gate, and on every inbound ACKNACK prunes acknowledged samples
// and retransmits the ones the peer still marks missing.

'use strict';

const dgram = require('node:dgram');

// --- constants (mirror ReliableWire.java / zerodds_endpoint.h) ---

const SESSION_NOKEY = 0x80;
const STREAM_RELIABLE = 0x80; // reliable stream id (bit 7 set)
const STREAM_NONE = 0x00; // control-frame stream in the golden layout

const SM_WRITE_DATA = 0x07;
const SM_ACKNACK = 0x0a;
const SM_HEARTBEAT = 0x0b;
const FLAGS_WRITE = 0x03; // E-flag LE + data-present
const FLAGS_CTRL = 0x01; // E-flag LE only

const WINDOW = 16; // sender in-flight cap (matches the 16-bit ACKNACK bitmap)
const RECV_BUF = 64; // receiver out-of-order buffer cap (DoS bound)
const MAX_PAYLOAD = 65535; // u16 submessage-length limit
const HEARTBEAT_PERIOD_MS = 500;

// --- RFC-1982 16-bit circular sequence-number comparisons ---

// a < b per RFC-1982 (wrapping), for a,b in [0,65535].
function seqLt(a, b) {
  return ((a - b) & 0xffff) >= 0x8000;
}
// a > b per RFC-1982 (wrapping).
function seqGt(a, b) {
  const d = (a - b) & 0xffff;
  return d !== 0 && d < 0x8000;
}

// --- frame builders (byte-identical to the golden control-frame layout) ---

// Reliable WRITE_DATA frame: the 2-byte header seq carries the RFC-1982 sample
// sequence. `sample` is a Buffer/Uint8Array.
function writeFrame(seq, sample) {
  const out = Buffer.alloc(8 + sample.length);
  out[0] = SESSION_NOKEY;
  out[1] = STREAM_RELIABLE;
  out[2] = seq & 0xff;
  out[3] = (seq >>> 8) & 0xff;
  out[4] = SM_WRITE_DATA;
  out[5] = FLAGS_WRITE;
  out[6] = sample.length & 0xff;
  out[7] = (sample.length >>> 8) & 0xff;
  Buffer.from(sample).copy(out, 8);
  return out;
}

// golden control-frame header: session, stream=NONE, msg-seq=1, submsg, flags, len=5.
function ctrlFrame(submsg, b8, b9, b10, b11, b12) {
  return Buffer.from([
    SESSION_NOKEY, STREAM_NONE, 0x01, 0x00,
    submsg, FLAGS_CTRL, 0x05, 0x00,
    b8, b9, b10, b11, b12,
  ]);
}

// HEARTBEAT frame: body = first i16 LE, last i16 LE, stream.
function heartbeatFrame(hb) {
  return ctrlFrame(
    SM_HEARTBEAT,
    hb.first & 0xff, (hb.first >>> 8) & 0xff,
    hb.last & 0xff, (hb.last >>> 8) & 0xff,
    hb.stream & 0xff,
  );
}

// ACKNACK frame: body = first_unacked i16 LE, nackLo, nackHi, stream.
function acknackFrame(ack) {
  return ctrlFrame(
    SM_ACKNACK,
    ack.firstUnacked & 0xff, (ack.firstUnacked >>> 8) & 0xff,
    ack.nackLo & 0xff, ack.nackHi & 0xff,
    ack.stream & 0xff,
  );
}

// --- frame parsers (peer contract: byte[4]=submsg-id, body at offset 8) ---

function i16(b, off) {
  return (b[off] & 0xff) | ((b[off + 1] & 0xff) << 8);
}

function parseWrite(f) {
  if (f.length < 8 || f[4] !== SM_WRITE_DATA) return null;
  return { seq: i16(f, 2), sample: Buffer.from(f.subarray(8)) };
}

function parseHeartbeat(f) {
  if (f.length < 13 || f[4] !== SM_HEARTBEAT) return null;
  return { first: i16(f, 8), last: i16(f, 10), stream: f[12] & 0xff };
}

function parseAckNack(f) {
  if (f.length < 13 || f[4] !== SM_ACKNACK) return null;
  return {
    firstUnacked: i16(f, 8),
    nackLo: f[10] & 0xff,
    nackHi: f[11] & 0xff,
    stream: f[12] & 0xff,
    bitmap() {
      return (this.nackLo & 0xff) | ((this.nackHi & 0xff) << 8);
    },
  };
}

// --- sender half of the state machine (mirrors ReliableSender.java) ---

const SENDER_OK = 'OK';
const SENDER_TOO_LARGE = 'PAYLOAD_TOO_LARGE';
const SENDER_WINDOW_FULL = 'WINDOW_FULL';

class ReliableSender {
  constructor(stream = STREAM_RELIABLE) {
    this.stream = stream & 0xff;
    this.nextSeq = 0;
    this.inFlight = new Map(); // seq -> Buffer
    this.haveHeartbeat = false;
    this.lastHeartbeatMs = 0;
  }

  // Submits a new sample. Assigns the next monotonic seqnr on success.
  submit(payload) {
    if (payload.length > MAX_PAYLOAD) return { status: SENDER_TOO_LARGE, seq: -1 };
    if (this.inFlight.size >= WINDOW) return { status: SENDER_WINDOW_FULL, seq: -1 };
    const seq = this.nextSeq;
    this.inFlight.set(seq, Buffer.from(payload));
    this.nextSeq = (this.nextSeq + 1) & 0xffff;
    return { status: SENDER_OK, seq };
  }

  inFlightCount() {
    return this.inFlight.size;
  }

  // In-flight payload lookup (for retransmit). null if not (or no longer) in-flight.
  getInFlight(seq) {
    return this.inFlight.get(seq & 0xffff) || null;
  }

  // Tick: returns a HEARTBEAT {first,last,stream} if the period elapsed and
  // in-flight samples exist; null otherwise. The window bounds are an RFC-1982
  // scan (oldest/newest unacked), NOT the numeric min/max -- those are wrong
  // across a 16-bit wrap (0xFFFE..0x0001 -> base 0xFFFE / end 0x0001).
  pendingHeartbeat(nowMs) {
    if (this.inFlight.size === 0) return null;
    const due = !this.haveHeartbeat || nowMs - this.lastHeartbeatMs >= HEARTBEAT_PERIOD_MS;
    if (!due) return null;
    this.haveHeartbeat = true;
    this.lastHeartbeatMs = nowMs;
    let first = 0;
    let last = 0;
    let seen = false;
    for (const k of this.inFlight.keys()) {
      if (!seen) {
        first = k;
        last = k;
        seen = true;
      } else {
        if (seqLt(k, first)) first = k;
        if (seqGt(k, last)) last = k;
      }
    }
    return { first, last, stream: this.stream };
  }

  // Processes an incoming ACKNACK: everything strictly before firstUnacked is
  // acknowledged (dropped), and within the 16-slot bitmap window a clear bit
  // means acknowledged (prune) while a set bit means still missing (retain).
  recvAcknack(ack) {
    const base = ack.firstUnacked & 0xffff;
    const bitmap = ack.bitmap();
    for (const k of Array.from(this.inFlight.keys())) {
      if (seqLt(k, base)) this.inFlight.delete(k);
    }
    for (let i = 0; i < 16; i++) {
      if (((bitmap >> i) & 1) === 0) {
        this.inFlight.delete((base + i) & 0xffff);
      }
    }
  }

  reset() {
    this.nextSeq = 0;
    this.inFlight.clear();
    this.haveHeartbeat = false;
  }
}

// --- receiver half of the state machine (mirrors ReliableReceiver.java) ---

const RECV_OK = 'OK';
const RECV_BUFFER_FULL = 'BUFFER_FULL';

class ReliableReceiver {
  constructor(stream = STREAM_RELIABLE) {
    this.stream = stream & 0xff;
    this.expectedSeq = 0;
    this.received = new Map(); // seq -> Buffer
  }

  // A sample (seq + payload) arrived: buffer it (dup < expected dropped).
  recvData(seq, payload) {
    seq &= 0xffff;
    if (seqLt(seq, this.expectedSeq)) return RECV_OK; // duplicate, already delivered
    if (this.received.has(seq)) return RECV_OK; // already buffered
    if (this.received.size >= RECV_BUF) return RECV_BUFFER_FULL;
    this.received.set(seq, Buffer.from(payload));
    return RECV_OK;
  }

  // Returns all contiguous samples available from `expected`, advancing it.
  drainInOrder() {
    const out = [];
    for (;;) {
      const payload = this.received.get(this.expectedSeq);
      if (payload === undefined) break;
      this.received.delete(this.expectedSeq);
      out.push(payload);
      this.expectedSeq = (this.expectedSeq + 1) & 0xffff;
    }
    return out;
  }

  expected() {
    return this.expectedSeq;
  }

  outOfOrderCount() {
    return this.received.size;
  }

  // ACKNACK for the missing slots in [expected, expected+16). `hint` (nullable)
  // is the last seqnr the sender is known to have sent (from a HEARTBEAT); slots
  // beyond it are not marked missing.
  pendingAcknack(hint) {
    let bitmap = 0;
    for (let i = 0; i < 16; i++) {
      const seq = (this.expectedSeq + i) & 0xffff;
      if (hint !== null && hint !== undefined && seqGt(seq, hint)) continue;
      if (!this.received.has(seq)) bitmap |= 1 << i;
    }
    return {
      firstUnacked: this.expectedSeq,
      nackLo: bitmap & 0xff,
      nackHi: (bitmap >> 8) & 0xff,
      stream: this.stream,
      bitmap() {
        return (this.nackLo & 0xff) | ((this.nackHi & 0xff) << 8);
      },
    };
  }

  reset() {
    this.expectedSeq = 0;
    this.received.clear();
  }
}

// --- async-decoupled reliable writer over a real UDP socket ---

const QUEUE_CAPACITY = 4096;
const SOCKET_POLL_MS = 5; // event-loop yield between drain iterations
const DRAIN_DEADLINE_MS = 20000;

class AsyncReliableWriter {
  constructor(peerHost, peerPort, drainDeadlineMs = DRAIN_DEADLINE_MS) {
    this.peerHost = peerHost;
    this.peerPort = peerPort;
    this.drainDeadlineMs = drainDeadlineMs;
    this.sender = new ReliableSender();
    this._queue = []; // producer -> drain
    this._spaceWaiters = []; // resolvers awaiting queue space (backpressure)
    this._inbox = []; // inbound ACKNACK frames (dgram 'message')
    this._producerDone = false;
    this._stopped = false;
    this._drained = false;
    this._deliveredCount = 0;
    this._drainError = null;
    this._finished = null; // Promise resolved when the drain loop ends
    this.socket = dgram.createSocket('udp4');
    this.socket.on('message', (msg) => {
      this._inbox.push(Buffer.from(msg));
    });
    this.socket.on('error', (err) => {
      this._drainError = err;
    });
  }

  start() {
    this._finished = this._drainLoop();
  }

  // Producer hot path: async enqueue only (no syscall). Awaits queue space when
  // full -- never drops a sample (no loss) and never hangs (the drain loop keeps
  // freeing slots; a stopped writer resolves waiters immediately).
  async submit(payload) {
    while (this._queue.length >= QUEUE_CAPACITY && !this._stopped) {
      await new Promise((resolve) => this._spaceWaiters.push(resolve));
    }
    if (this._stopped) return;
    this._queue.push(Buffer.from(payload));
  }

  // Non-blocking enqueue: returns false when the queue is full (bench/overflow).
  offer(payload) {
    if (this._queue.length >= QUEUE_CAPACITY || this._stopped) return false;
    this._queue.push(Buffer.from(payload));
    return true;
  }

  _wakeSpace() {
    const w = this._spaceWaiters.shift();
    if (w) w();
  }

  // Signals no more samples, then awaits (up to timeoutMs) a full drain of the
  // queue + reliable send window. Returns whether it fully drained.
  async finish(timeoutMs = DRAIN_DEADLINE_MS) {
    this.drainDeadlineMs = timeoutMs;
    this._producerDone = true;
    if (this._finished) await this._finished;
    return this._drained;
  }

  delivered() {
    return this._deliveredCount;
  }

  drainError() {
    return this._drainError;
  }

  async _drainLoop() {
    let drainDeadline = -1;
    try {
      while (!this._stopped) {
        // 1) Move queued samples into the send window: submit -> in-flight ->
        //    frame -> send WRITE_DATA.
        while (this.sender.inFlightCount() < WINDOW) {
          const payload = this._queue.shift();
          if (payload === undefined) break;
          this._wakeSpace();
          const r = this.sender.submit(payload);
          if (r.status !== SENDER_OK) break; // payload too large -> drop, don't wedge
          this._send(writeFrame(r.seq, payload));
          this._deliveredCount += 1;
        }

        // 2) HEARTBEAT timer (first immediately, then every 500ms while unacked).
        const hb = this.sender.pendingHeartbeat(Date.now());
        if (hb) this._send(heartbeatFrame(hb));

        // 3) Drain inbound ACKNACKs: prune acknowledged, retransmit still-missing.
        let frame;
        while ((frame = this._inbox.shift()) !== undefined) {
          const ack = parseAckNack(frame);
          if (ack) {
            this.sender.recvAcknack(ack);
            this._retransmitMissing(ack);
          }
        }

        // 4) Only after the producer signals done: drain the queue + window, but
        //    no longer than drainDeadlineMs so a dead peer cannot hang forever.
        if (this._producerDone) {
          if (this._queue.length === 0 && this.sender.inFlightCount() === 0) {
            this._drained = true;
            break;
          }
          if (drainDeadline < 0) drainDeadline = Date.now() + this.drainDeadlineMs;
          if (Date.now() >= drainDeadline) break;
        }

        // Yield to the event loop so inbound datagrams land in the inbox.
        await new Promise((resolve) => setTimeout(resolve, SOCKET_POLL_MS));
      }
    } catch (err) {
      this._drainError = err;
    } finally {
      this._stopped = true;
      // Release any producer awaiting queue space.
      while (this._spaceWaiters.length) this._wakeSpace();
    }
  }

  _retransmitMissing(ack) {
    const base = ack.firstUnacked & 0xffff;
    const bitmap = ack.bitmap();
    for (let i = 0; i < 16; i++) {
      if (((bitmap >> i) & 1) === 0) continue; // acknowledged / never sent
      const seq = (base + i) & 0xffff;
      const payload = this.sender.getInFlight(seq);
      if (payload) this._send(writeFrame(seq, payload));
    }
  }

  _send(frame) {
    if (this._stopped) return;
    this.socket.send(frame, this.peerPort, this.peerHost);
  }

  close() {
    this._stopped = true;
    while (this._spaceWaiters.length) this._wakeSpace();
    try {
      this.socket.close();
    } catch (_e) {
      // already closed
    }
  }
}

module.exports = {
  // constants
  SESSION_NOKEY, STREAM_RELIABLE, STREAM_NONE,
  SM_WRITE_DATA, SM_ACKNACK, SM_HEARTBEAT, FLAGS_WRITE, FLAGS_CTRL,
  WINDOW, RECV_BUF, MAX_PAYLOAD, HEARTBEAT_PERIOD_MS,
  // rfc-1982
  seqLt, seqGt,
  // wire
  writeFrame, heartbeatFrame, acknackFrame,
  parseWrite, parseHeartbeat, parseAckNack,
  // state machine
  ReliableSender, ReliableReceiver,
  SENDER_OK, SENDER_TOO_LARGE, SENDER_WINDOW_FULL,
  RECV_OK, RECV_BUFFER_FULL,
  // async writer
  AsyncReliableWriter,
};
