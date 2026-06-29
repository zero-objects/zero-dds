// qos.test.ts — behavioral coverage for the DDS QoS + keyed-lifecycle surface
// threaded through the binding (OMG DDS-DCPS 1.4 §2.2.3). Each test drives the
// real native DCPS loopback (no mocking) and asserts an observable QoS effect.
//
// Part of the default `npm test` gate (run serially via --test-concurrency=1).
// All tests share ONE DomainParticipant on a single fixed low domain so the
// suite stays robust against SPDP discovery-socket contention from other DDS
// processes on the host. The same behaviors are also proven by the
// cross-language QoS conformance harness at
// zerodds-examples/idl-conformance/_qos/typescript/qos_conformance.ts.

import test from "node:test";
import assert from "node:assert/strict";
import {
  DomainParticipantFactory, DomainParticipant,
  CftFieldKind,
  ReliabilityKind, DurabilityKind, HistoryKind,
  defaultDataWriterQos, defaultDataReaderQos,
  type TypeSupport,
} from "../src/index.js";

const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));

// A keyed test type: { id (key), seq, value } encoded as 3x int32, FINAL
// extensibility (no DHEADER) so the CFT positional schema maps 1:1.
interface Rec { id: number; seq: number; value: number; }
const RecTS: TypeSupport<Rec> = {
  typeName: "QosRec",
  isKeyed: true,
  encode(s: Rec): Uint8Array {
    const b = new Uint8Array(12);
    const dv = new DataView(b.buffer);
    dv.setInt32(0, s.id, true);
    dv.setInt32(4, s.seq, true);
    dv.setInt32(8, s.value, true);
    return b;
  },
  decode(b: Uint8Array): Rec {
    const dv = new DataView(b.buffer, b.byteOffset, b.byteLength);
    return { id: dv.getInt32(0, true), seq: dv.getInt32(4, true), value: dv.getInt32(8, true) };
  },
  keyHash(s: Rec): Uint8Array {
    // 16-byte big-endian key hash over the single key field `id`.
    const h = new Uint8Array(16);
    new DataView(h.buffer).setInt32(0, s.id, false);
    return h;
  },
};

// All tests share ONE participant on a single low-numbered domain (the same
// shape as the rock-solid QoS conformance harness, which runs every check on a
// single domain-0 participant). DDS permits unlimited topics/pubs/subs per
// participant, so a single participant is created lazily on first use and
// reused — minimising SPDP discovery-socket pressure. The lazy create is
// guarded so a participant-creation error surfaces in the first test rather
// than at import time.
const DOMAIN = 55; // fixed low domain, distinct from the smoke suite's domains
let SHARED: DomainParticipant | null = null;
let SHARED_TRIED = false;

async function withParticipant(body: (p: DomainParticipant) => Promise<void>): Promise<void> {
  if (!SHARED && !SHARED_TRIED) {
    SHARED_TRIED = true;
    SHARED = DomainParticipantFactory.instance().createParticipant(DOMAIN);
  }
  if (!SHARED) throw new Error("shared participant unavailable");
  await body(SHARED);
}

test("qos: RELIABLE reliability is selectable + lossless on loopback", async () => {
  await withParticipant(async (p) => {
    const t = p.createTypedTopic("RelRec", RecTS);
    const wq = defaultDataWriterQos(); wq.reliability.kind = ReliabilityKind.Reliable;
    const rq = defaultDataReaderQos(); rq.reliability.kind = ReliabilityKind.Reliable;
    const w = p.createPublisher().createTypedWriter(t, wq);
    const r = p.createSubscriber().createTypedReader(t, rq);
    await w.waitForMatchedSubscription(1, 5000);
    await r.waitForMatchedPublication(1, 5000);
    const N = 50;
    for (let i = 0; i < N; i++) w.write({ id: 1, seq: i, value: i });
    const got: Rec[] = [];
    for (let tries = 0; tries < 30 && got.length < N; tries++) {
      if (await r.waitForData(300)) got.push(...r.take());
    }
    assert.equal(got.length, N, `expected ${N} reliable samples, got ${got.length}`);
    assert.ok(got.every((s, i) => s.seq === i), "samples out of order");
  });
});

test("qos: TRANSIENT_LOCAL durability delivers to a late joiner", async () => {
  await withParticipant(async (p) => {
    const t = p.createTypedTopic("DurRec", RecTS);
    const wq = defaultDataWriterQos();
    wq.durability.kind = DurabilityKind.TransientLocal;
    wq.reliability.kind = ReliabilityKind.Reliable;
    const w = p.createPublisher().createTypedWriter(t, wq);
    w.write({ id: 7, seq: 500, value: 9 });
    await sleep(150);
    const rq = defaultDataReaderQos();
    rq.durability.kind = DurabilityKind.TransientLocal;
    rq.reliability.kind = ReliabilityKind.Reliable;
    const late = p.createSubscriber().createTypedReader(t, rq);
    await late.waitForMatchedPublication(1, 3000);
    const got = (await late.waitForData(2000)) ? late.take() : [];
    assert.ok(got.some((s) => s.id === 7 && s.seq === 500),
      `late joiner missed the retained TRANSIENT_LOCAL sample, got ${JSON.stringify(got)}`);
  });
});

test("qos: KEEP_LAST(1) caps per-instance retained samples", async () => {
  await withParticipant(async (p) => {
    const t = p.createTypedTopic("HistRec", RecTS);
    const wq = defaultDataWriterQos();
    wq.durability.kind = DurabilityKind.TransientLocal;
    wq.reliability.kind = ReliabilityKind.Reliable;
    wq.history.kind = HistoryKind.KeepLast;
    wq.history.depth = 1;
    const w = p.createPublisher().createTypedWriter(t, wq);
    for (let i = 0; i < 20; i++) w.write({ id: 3, seq: i, value: i });
    await sleep(150);
    const rq = defaultDataReaderQos();
    rq.durability.kind = DurabilityKind.TransientLocal;
    rq.reliability.kind = ReliabilityKind.Reliable;
    rq.history.kind = HistoryKind.KeepLast;
    rq.history.depth = 1;
    const late = p.createSubscriber().createTypedReader(t, rq);
    await late.waitForMatchedPublication(1, 3000);
    let got: Rec[] = [];
    if (await late.waitForData(2000)) got = late.take().filter((s) => s.id === 3);
    await sleep(100); got.push(...late.take().filter((s) => s.id === 3));
    assert.equal(got.length, 1, `KEEP_LAST(1) should retain exactly 1, got ${got.length}`);
    assert.equal(got[0].seq, 19, "retained sample should be the last written");
  });
});

test("qos: PARTITION isolates publisher/subscriber by name", async () => {
  await withParticipant(async (p) => {
    const t = p.createTypedTopic("PartRec", RecTS);
    const wq = defaultDataWriterQos(); wq.reliability.kind = ReliabilityKind.Reliable;
    const rq = defaultDataReaderQos(); rq.reliability.kind = ReliabilityKind.Reliable;
    const w = p.createPublisher({ partition: { names: ["A"] } }).createTypedWriter(t, wq);
    const rA = p.createSubscriber({ partition: { names: ["A"] } }).createTypedReader(t, rq);
    const rB = p.createSubscriber({ partition: { names: ["B"] } }).createTypedReader(t, rq);
    await w.waitForMatchedSubscription(1, 3000);
    await rA.waitForMatchedPublication(1, 3000);
    w.write({ id: 9, seq: 0, value: 7 });
    await sleep(300);
    const gotA = rA.take().filter((s) => s.id === 9);
    const gotB = rB.take().filter((s) => s.id === 9);
    assert.ok(gotA.length > 0, "partition A reader received nothing");
    assert.equal(gotB.length, 0, "partition B reader should be isolated");
  });
});

test("qos: ContentFilteredTopic applies the SQL filter", async () => {
  await withParticipant(async (p) => {
    const base = p.createTypedTopic("CftRec", RecTS);
    // FINAL extensibility (no DHEADER): the positional schema maps directly.
    const cft = p.createContentFilteredTopic(
      "CftRecFiltered", base, "seq > 10", [],
      [
        { name: "id", kind: CftFieldKind.Int32 },
        { name: "seq", kind: CftFieldKind.Int32 },
        { name: "value", kind: CftFieldKind.Int32 },
      ],
    );
    const wq = defaultDataWriterQos(); wq.reliability.kind = ReliabilityKind.Reliable;
    const w = p.createPublisher().createTypedWriter(base, wq);
    const r = p.createSubscriber().createFilteredReader(cft);
    await w.waitForMatchedSubscription(1, 3000);
    await r.waitForMatchedPublication(1, 3000);
    for (let i = 0; i < 20; i++) w.write({ id: 8, seq: i, value: i });
    await sleep(300);
    let got: Rec[] = [];
    for (let k = 0; k < 5; k++) { got.push(...r.take()); await sleep(60); }
    got = got.filter((s) => s.id === 8);
    assert.ok(got.length > 0, "CFT reader received nothing");
    assert.ok(got.every((s) => s.seq > 10), `CFT let through seq<=10: ${got.map((s) => s.seq)}`);
  });
});

test("qos: keyed dispose surfaces NOT_ALIVE_DISPOSED via takeWithInfo", async () => {
  await withParticipant(async (p) => {
    const t = p.createTypedTopic("LifeRec", RecTS);
    const wq = defaultDataWriterQos(); wq.reliability.kind = ReliabilityKind.Reliable;
    const rq = defaultDataReaderQos(); rq.reliability.kind = ReliabilityKind.Reliable;
    const w = p.createPublisher().createTypedWriter(t, wq);
    const r = p.createSubscriber().createTypedReader(t, rq);
    await w.waitForMatchedSubscription(1, 3000);
    await r.waitForMatchedPublication(1, 3000);
    w.write({ id: 10, seq: 0, value: 1 });
    await sleep(150);
    const alive = r.takeWithInfo();
    assert.ok(alive.some((s) => s.validData && s.instanceState === 1), "no ALIVE sample observed");
    w.dispose({ id: 10, seq: 0, value: 1 });
    await sleep(200);
    const after = r.takeWithInfo();
    assert.ok(after.some((s) => s.instanceState === 2),
      `dispose did not surface NOT_ALIVE_DISPOSED(=2); states=${after.map((s) => s.instanceState)}`);
  });
});

test("qos: reader status getters are callable", async () => {
  await withParticipant(async (p) => {
    const t = p.createTypedTopic("StatusRec", RecTS);
    const r = p.createSubscriber().createTypedReader(t);
    const dl = r.getRequestedDeadlineMissedStatus();
    assert.equal(typeof dl.totalCount, "number");
    const lv = r.getLivelinessChangedStatus();
    assert.equal(typeof lv.aliveCount, "number");
  });
});

test("qos: zzz teardown shared participant", () => {
  // Registered last so it runs after every QoS test: releases the single
  // shared participant created lazily by withParticipant().
  if (SHARED) { SHARED.destroy(); SHARED = null; }
});
