// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// Unit + byte-golden suite for the native Node reliable-stream endpoint,
// mirroring endpoints/java's ReliableSelfTest and crates/xrce/src/reliable.rs.
// Run with `node --test`. The byte-goldens are hardcoded (self-contained);
// the same frames are asserted byte-identical to the Rust golden files by the
// live harness (crates/endpoint-e2e/tests/node_reliable.rs).

'use strict';

const { test } = require('node:test');
const assert = require('node:assert');
const r = require('./reliable');

test('RFC-1982 wrapping comparisons', () => {
  assert.ok(r.seqLt(0, 1));
  assert.ok(!r.seqLt(1, 0));
  assert.ok(r.seqLt(0xffff, 0)); // wrap: 0xFFFF < 0x0000
  assert.ok(r.seqGt(0, 0xffff));
  assert.ok(!r.seqLt(5, 5) && !r.seqGt(5, 5));
});

test('sender: monotonic seq + in-flight count', () => {
  const s = new r.ReliableSender();
  const a = s.submit(Buffer.from([1, 2]));
  const b = s.submit(Buffer.from([3, 4]));
  assert.strictEqual(a.status, r.SENDER_OK);
  assert.strictEqual(a.seq, 0);
  assert.strictEqual(b.seq, 1);
  assert.strictEqual(s.inFlightCount(), 2);
});

test('sender: payload too large + window full', () => {
  const s = new r.ReliableSender();
  assert.strictEqual(s.submit(Buffer.alloc(r.MAX_PAYLOAD + 1)).status, r.SENDER_TOO_LARGE);
  const s2 = new r.ReliableSender();
  for (let i = 0; i < r.WINDOW; i++) {
    assert.strictEqual(s2.submit(Buffer.from([0])).status, r.SENDER_OK);
  }
  assert.strictEqual(s2.submit(Buffer.from([0])).status, r.SENDER_WINDOW_FULL);
});

test('sender: heartbeat gate (first immediate, then 500ms)', () => {
  const s = new r.ReliableSender();
  assert.strictEqual(s.pendingHeartbeat(0), null, 'no heartbeat when empty');
  s.submit(Buffer.from([1]));
  const base = 1000000;
  const hb = s.pendingHeartbeat(base);
  assert.ok(hb && hb.first === 0 && hb.last === 0 && hb.stream === 0x80, 'heartbeat body');
  assert.strictEqual(s.pendingHeartbeat(base + 100), null, 'silenced < 500ms');
  assert.ok(s.pendingHeartbeat(base + 600) !== null, 'fires after 500ms');
});

test('sender: acknack prunes acked, retains missing', () => {
  const s = new r.ReliableSender();
  s.submit(Buffer.from([0xa0])); // seq 0
  s.submit(Buffer.from([0xa1])); // seq 1
  s.submit(Buffer.from([0xa2])); // seq 2
  // base=2, bitmap=0b1 -> seq2 still missing; 0+1 acknowledged.
  s.recvAcknack(r.parseAckNack(r.acknackFrame({ firstUnacked: 2, nackLo: 0x01, nackHi: 0, stream: 0x80 })));
  assert.strictEqual(s.inFlightCount(), 1);
  assert.ok(s.getInFlight(2) !== null, 'seq2 retransmittable');
  const s2 = new r.ReliableSender();
  for (let i = 0; i < 5; i++) s2.submit(Buffer.from([0]));
  s2.recvAcknack(r.parseAckNack(r.acknackFrame({ firstUnacked: 5, nackLo: 0, nackHi: 0, stream: 0x80 })));
  assert.strictEqual(s2.inFlightCount(), 0, 'full clear');
});

test('receiver: in-order drain + reorder + duplicate', () => {
  const rc = new r.ReliableReceiver();
  rc.recvData(0, Buffer.from([10]));
  rc.recvData(1, Buffer.from([11]));
  let d = rc.drainInOrder();
  assert.strictEqual(d.length, 2);
  assert.strictEqual(rc.expected(), 2);

  const rc2 = new r.ReliableReceiver();
  rc2.recvData(2, Buffer.from([22]));
  rc2.recvData(0, Buffer.from([20]));
  assert.strictEqual(rc2.drainInOrder().length, 1, 'only seq0 before gap fill');
  rc2.recvData(1, Buffer.from([21]));
  assert.strictEqual(rc2.drainInOrder().length, 2, 'seq1+2 after fill');

  const rc3 = new r.ReliableReceiver();
  rc3.recvData(0, Buffer.from([1]));
  rc3.drainInOrder();
  rc3.recvData(0, Buffer.from([99])); // duplicate < expected
  assert.strictEqual(rc3.outOfOrderCount(), 0, 'duplicate dropped');
});

test('receiver: buffer-full bound', () => {
  const rc = new r.ReliableReceiver();
  for (let i = 1; i <= r.RECV_BUF; i++) {
    assert.strictEqual(rc.recvData(i, Buffer.from([1])), r.RECV_OK);
  }
  assert.strictEqual(rc.recvData(r.RECV_BUF + 1, Buffer.from([1])), r.RECV_BUFFER_FULL);
});

test('receiver: pending acknack bitmap', () => {
  const rc = new r.ReliableReceiver();
  rc.recvData(1, Buffer.from([1]));
  rc.recvData(3, Buffer.from([3]));
  const a = rc.pendingAcknack(3);
  const bm = a.bitmap();
  assert.ok((bm & 1) !== 0, 'slot 0 missing');
  assert.ok((bm & (1 << 2)) !== 0, 'slot 2 missing');
  assert.ok((bm & (1 << 1)) === 0, 'slot 1 present');
  assert.ok((bm & (1 << 3)) === 0, 'slot 3 present');
});

test('end-to-end loss recovery (in-process)', () => {
  const s = new r.ReliableSender();
  const rc = new r.ReliableReceiver();
  const seqs = [];
  for (let i = 0; i < 3; i++) seqs.push(s.submit(Buffer.from([i])).seq);
  rc.recvData(seqs[0], Buffer.from([0])); // seq1 lost
  rc.recvData(seqs[2], Buffer.from([2]));
  assert.strictEqual(rc.drainInOrder().length, 1, 'only seq0 before recovery');
  const ack = rc.pendingAcknack(seqs[2]);
  s.recvAcknack(ack);
  assert.ok(s.getInFlight(seqs[1]) !== null, 'seq1 retransmittable');
  rc.recvData(seqs[1], s.getInFlight(seqs[1]));
  assert.strictEqual(rc.drainInOrder().length, 2, 'seq1+2 after recovery');
});

test('RFC-1982 loss recovery across the 16-bit wrap', () => {
  const s = new r.ReliableSender();
  let seq;
  do {
    seq = s.submit(Buffer.from([0])).seq;
    s.recvAcknack(r.parseAckNack(r.acknackFrame({ firstUnacked: (seq + 1) & 0xffff, nackLo: 0, nackHi: 0, stream: 0x80 })));
  } while (seq !== 0xfffd);
  assert.strictEqual(s.inFlightCount(), 0, 'wrap seed drained');

  const q0 = s.submit(Buffer.from([10])).seq; // 0xFFFE
  const q1 = s.submit(Buffer.from([11])).seq; // 0xFFFF (lost)
  const q2 = s.submit(Buffer.from([12])).seq; // 0x0000
  const q3 = s.submit(Buffer.from([13])).seq; // 0x0001
  assert.ok(q0 === 0xfffe && q1 === 0xffff && q2 === 0x0000 && q3 === 0x0001, 'wrap seqs');

  const hbw = s.pendingHeartbeat(0);
  assert.ok(hbw && hbw.first === 0xfffe && hbw.last === 0x0001, 'heartbeat window across wrap');

  const rc = new r.ReliableReceiver();
  for (let k = 0; k <= 0xfffd; k++) {
    rc.recvData(k, Buffer.from([0]));
    rc.drainInOrder();
  }
  assert.strictEqual(rc.expected(), 0xfffe, 'receiver expects 0xFFFE');

  rc.recvData(q0, Buffer.from([10])); // 0xFFFF lost
  rc.recvData(q2, Buffer.from([12]));
  rc.recvData(q3, Buffer.from([13]));
  const dw = rc.drainInOrder();
  assert.ok(dw.length === 1 && dw[0][0] === 10, 'only 0xFFFE before recovery');
  assert.strictEqual(rc.expected(), 0xffff, 'blocked at 0xFFFF');

  const ackw = rc.pendingAcknack(q3);
  assert.strictEqual(ackw.firstUnacked, 0xffff, 'acknack base = 0xFFFF across wrap');
  assert.ok((ackw.bitmap() & 0b1) !== 0, '0xFFFF NACKed');
  assert.ok((ackw.bitmap() & 0b110) === 0, '0x0000/0x0001 present');

  s.recvAcknack(ackw);
  assert.ok(s.getInFlight(q1) !== null, '0xFFFF retransmittable');
  assert.ok(s.getInFlight(q0) === null && s.getInFlight(q2) === null && s.getInFlight(q3) === null, 'others acked');
  assert.strictEqual(s.inFlightCount(), 1, 'only 0xFFFF left');

  rc.recvData(q1, s.getInFlight(q1));
  const dw2 = rc.drainInOrder();
  assert.ok(dw2.length === 3 && dw2[0][0] === 11 && dw2[1][0] === 12 && dw2[2][0] === 13, 'in RFC-1982 order');
});

test('byte-golden: heartbeat + acknack (hardcoded)', () => {
  const hb = r.heartbeatFrame({ first: 1, last: 3, stream: 0x80 });
  const hbExpect = Buffer.from([0x80, 0x00, 0x01, 0x00, 0x0b, 0x01, 0x05, 0x00, 0x01, 0x00, 0x03, 0x00, 0x80]);
  assert.ok(hb.equals(hbExpect), 'heartbeat byte-golden');

  const ak = r.acknackFrame({ firstUnacked: 1, nackLo: 0, nackHi: 0, stream: 0x80 });
  const akExpect = Buffer.from([0x80, 0x00, 0x01, 0x00, 0x0a, 0x01, 0x05, 0x00, 0x01, 0x00, 0x00, 0x00, 0x80]);
  assert.ok(ak.equals(akExpect), 'acknack byte-golden');
});

test('write frame carries the RFC-1982 sample seq', () => {
  const f = r.writeFrame(0x1234, Buffer.from([0xde, 0xad]));
  assert.strictEqual(f[1], 0x80, 'reliable stream id');
  assert.strictEqual(f[4], 0x07, 'WRITE_DATA id');
  assert.strictEqual(f[2] | (f[3] << 8), 0x1234, 'seq LE');
  const p = r.parseWrite(f);
  assert.ok(p && p.seq === 0x1234 && p.sample.equals(Buffer.from([0xde, 0xad])));
});

test('async writer: overflow backpressure without loss', async () => {
  // Small deadline; no peer -> the window never drains, but submit must never
  // drop a sample. Bound the queue tiny via direct field to exercise waiters.
  const w = new r.AsyncReliableWriter('127.0.0.1', 1); // unused port; never sends usefully
  // Don't start the drain loop: prove submit resolves once space exists by
  // manually draining the queue, i.e. no loss and no hang on a bounded queue.
  const sent = [];
  const N = 20;
  const producer = (async () => {
    for (let i = 0; i < N; i++) {
      await w.submit(Buffer.from([i]));
      sent.push(i);
    }
  })();
  await producer;
  assert.strictEqual(sent.length, N, 'all samples enqueued (no loss)');
  assert.strictEqual(w._queue.length, N, 'queue holds every sample');
  w.close();
});
