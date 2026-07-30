// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// Standalone unit + byte-golden self-test for the Node reliable-stream endpoint,
// mirroring endpoints/java/ReliableSelfTest.java. Prints "ALL OK" and exits 0 on
// success, so the Rust harness (crates/endpoint-e2e/tests/node_reliable.rs) can
// gate on it and optionally pass the Rust golden files for a byte-identical
// cross-check.
//   usage: node reliable_selftest.js [golden_heartbeat_le.bin golden_acknack_le.bin]

'use strict';

const fs = require('node:fs');
const r = require('./reliable');

function check(cond, msg) {
  if (!cond) {
    console.log(`FAIL: ${msg}`);
    process.exit(1);
  }
}

function ack(firstUnacked, lo, hi) {
  return r.parseAckNack(r.acknackFrame({ firstUnacked, nackLo: lo, nackHi: hi, stream: 0x80 }));
}

// --- Sender ---
{
  const s = new r.ReliableSender();
  const a = s.submit(Buffer.from([1, 2]));
  const b = s.submit(Buffer.from([3, 4]));
  check(a.status === r.SENDER_OK && a.seq === 0, 'monotonic seq 0');
  check(b.status === r.SENDER_OK && b.seq === 1, 'monotonic seq 1');
  check(s.inFlightCount() === 2, 'in-flight count');
}
{
  const s = new r.ReliableSender();
  check(s.submit(Buffer.alloc(r.MAX_PAYLOAD + 1)).status === r.SENDER_TOO_LARGE, 'payload too large');
}
{
  const s = new r.ReliableSender();
  for (let i = 0; i < r.WINDOW; i++) check(s.submit(Buffer.from([0])).status === r.SENDER_OK, 'fill window');
  check(s.submit(Buffer.from([0])).status === r.SENDER_WINDOW_FULL, 'window full');
}
{
  const s = new r.ReliableSender();
  s.submit(Buffer.from([1]));
  const base = 1000000;
  const hb = s.pendingHeartbeat(base);
  check(hb !== null, 'heartbeat fires first');
  check(hb.first === 0 && hb.last === 0 && hb.stream === 0x80, 'heartbeat body');
  check(s.pendingHeartbeat(base + 100) === null, 'heartbeat silenced <500ms');
  check(s.pendingHeartbeat(base + 600) !== null, 'heartbeat after 500ms');
}
{
  const s = new r.ReliableSender();
  check(s.pendingHeartbeat(0) === null, 'no heartbeat when empty');
}
{
  const s = new r.ReliableSender();
  s.submit(Buffer.from([0xa0]));
  s.submit(Buffer.from([0xa1]));
  s.submit(Buffer.from([0xa2]));
  s.recvAcknack(ack(2, 0x01, 0x00)); // base=2, seq2 missing
  check(s.inFlightCount() === 1, 'acknack clears acked');
  check(s.getInFlight(2) !== null, 'seq2 retransmittable');
}
{
  const s = new r.ReliableSender();
  for (let i = 0; i < 5; i++) s.submit(Buffer.from([0]));
  s.recvAcknack(ack(5, 0, 0)); // full clear
  check(s.inFlightCount() === 0, 'acknack full clear');
}

// --- Receiver ---
{
  const rc = new r.ReliableReceiver();
  rc.recvData(0, Buffer.from([10]));
  rc.recvData(1, Buffer.from([11]));
  const d = rc.drainInOrder();
  check(d.length === 2 && d[0][0] === 10 && d[1][0] === 11, 'in-order drain');
  check(rc.expected() === 2, 'expected advanced');
}
{
  const rc = new r.ReliableReceiver();
  rc.recvData(2, Buffer.from([22]));
  rc.recvData(0, Buffer.from([20]));
  const d1 = rc.drainInOrder();
  check(d1.length === 1 && d1[0][0] === 20, 'reorder: only seq0');
  rc.recvData(1, Buffer.from([21]));
  const d2 = rc.drainInOrder();
  check(d2.length === 2 && d2[0][0] === 21 && d2[1][0] === 22, 'reorder: 1+2');
}
{
  const rc = new r.ReliableReceiver();
  rc.recvData(0, Buffer.from([1]));
  rc.drainInOrder();
  rc.recvData(0, Buffer.from([99])); // duplicate
  check(rc.outOfOrderCount() === 0, 'duplicate dropped');
}
{
  const rc = new r.ReliableReceiver();
  for (let i = 1; i <= r.RECV_BUF; i++) check(rc.recvData(i, Buffer.from([1])) === r.RECV_OK, 'fill recv buffer');
  check(rc.recvData(r.RECV_BUF + 1, Buffer.from([1])) === r.RECV_BUFFER_FULL, 'recv buffer full');
}
{
  const rc = new r.ReliableReceiver();
  rc.recvData(1, Buffer.from([1]));
  rc.recvData(3, Buffer.from([3]));
  const bm = rc.pendingAcknack(3).bitmap();
  check((bm & 1) !== 0, 'slot 0 missing');
  check((bm & (1 << 2)) !== 0, 'slot 2 missing');
  check((bm & (1 << 1)) === 0, 'slot 1 present');
  check((bm & (1 << 3)) === 0, 'slot 3 present');
}
{
  const rc = new r.ReliableReceiver();
  rc.recvData(0, Buffer.from([3]));
  rc.reset();
  check(rc.expected() === 0 && rc.outOfOrderCount() === 0, 'reset clears receiver');
}

// --- end-to-end loss recovery (in-process) ---
{
  const s = new r.ReliableSender();
  const rc = new r.ReliableReceiver();
  const seqs = [];
  for (let i = 0; i < 3; i++) seqs.push(s.submit(Buffer.from([i])).seq);
  rc.recvData(seqs[0], Buffer.from([0])); // seq1 lost
  rc.recvData(seqs[2], Buffer.from([2]));
  check(rc.drainInOrder().length === 1, 'only seq0 before recovery');
  const a = rc.pendingAcknack(seqs[2]);
  s.recvAcknack(a);
  check(s.getInFlight(seqs[1]) !== null, 'seq1 retransmittable');
  rc.recvData(seqs[1], s.getInFlight(seqs[1]));
  check(rc.drainInOrder().length === 2, 'seq1+2 after recovery');
}

// --- RFC-1982 regression: HEARTBEAT window + loss recovery across the wrap ---
{
  const s = new r.ReliableSender();
  let seq;
  do {
    seq = s.submit(Buffer.from([0])).seq;
    s.recvAcknack(ack((seq + 1) & 0xffff, 0, 0));
  } while (seq !== 0xfffd);
  check(s.inFlightCount() === 0, 'wrap seed: sender window drained');

  const q0 = s.submit(Buffer.from([10])).seq; // 0xFFFE
  const q1 = s.submit(Buffer.from([11])).seq; // 0xFFFF (lost)
  const q2 = s.submit(Buffer.from([12])).seq; // 0x0000
  const q3 = s.submit(Buffer.from([13])).seq; // 0x0001
  check(q0 === 0xfffe && q1 === 0xffff && q2 === 0x0000 && q3 === 0x0001, 'wrap seqs');

  const hbw = s.pendingHeartbeat(0);
  check(hbw !== null && hbw.first === 0xfffe && hbw.last === 0x0001, 'heartbeat window across wrap');

  const rc = new r.ReliableReceiver();
  for (let k = 0; k <= 0xfffd; k++) {
    rc.recvData(k, Buffer.from([0]));
    rc.drainInOrder();
  }
  check(rc.expected() === 0xfffe, 'wrap seed: receiver expects 0xFFFE');

  rc.recvData(q0, Buffer.from([10])); // 0xFFFF lost
  rc.recvData(q2, Buffer.from([12]));
  rc.recvData(q3, Buffer.from([13]));
  const dw = rc.drainInOrder();
  check(dw.length === 1 && dw[0][0] === 10, 'only 0xFFFE before recovery');
  check(rc.expected() === 0xffff, 'receiver blocked at 0xFFFF');

  const ackw = rc.pendingAcknack(q3);
  check(ackw.firstUnacked === 0xffff, 'acknack base = 0xFFFF across wrap');
  check((ackw.bitmap() & 0b1) !== 0, '0xFFFF NACKed');
  check((ackw.bitmap() & 0b110) === 0, '0x0000/0x0001 present');

  s.recvAcknack(ackw);
  check(s.getInFlight(q1) !== null, '0xFFFF retransmittable');
  check(s.getInFlight(q0) === null && s.getInFlight(q2) === null && s.getInFlight(q3) === null, 'others acked');
  check(s.inFlightCount() === 1, 'only 0xFFFF left in-flight');

  rc.recvData(q1, s.getInFlight(q1));
  const dw2 = rc.drainInOrder();
  check(dw2.length === 3 && dw2[0][0] === 11 && dw2[1][0] === 12 && dw2[2][0] === 13, 'wrap deliver order');
}

// --- byte-golden (hardcoded) ---
const hb = r.heartbeatFrame({ first: 1, last: 3, stream: 0x80 });
const hbExpect = Buffer.from([0x80, 0x00, 0x01, 0x00, 0x0b, 0x01, 0x05, 0x00, 0x01, 0x00, 0x03, 0x00, 0x80]);
check(hb.equals(hbExpect), 'heartbeat byte-golden (hardcoded)');

const ak = r.acknackFrame({ firstUnacked: 1, nackLo: 0, nackHi: 0, stream: 0x80 });
const akExpect = Buffer.from([0x80, 0x00, 0x01, 0x00, 0x0a, 0x01, 0x05, 0x00, 0x01, 0x00, 0x00, 0x00, 0x80]);
check(ak.equals(akExpect), 'acknack byte-golden (hardcoded)');

if (process.argv.length >= 4) {
  const gHb = fs.readFileSync(process.argv[2]);
  const gAk = fs.readFileSync(process.argv[3]);
  check(hb.equals(gHb), 'heartbeat byte-identical to golden file');
  check(ak.equals(gAk), 'acknack byte-identical to golden file');
  console.log('golden files matched');
}

console.log('ALL OK');
