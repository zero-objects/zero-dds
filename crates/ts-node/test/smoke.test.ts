// smoke.test.ts — TypeScript pub/sub roundtrip over the C-FFI.

import test from "node:test";
import assert from "node:assert/strict";
import {
  Reader, Runtime, Writer,
  DomainParticipantFactory, Topic, Publisher, DataWriter, Subscriber, DataReader,
  GuardCondition, WaitSet, ByteSeqTraits,
} from "../src/index.js";

const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));

// NOTE: previously skipped on darwin — that skip masked the koffi `_Out_` bug in
// Reader.take() (it never ran on the dev machine). The roundtrip MUST exercise
// take() on every platform, otherwise the binding regresses silently.
test("ts pub-sub roundtrip", async () => {
  console.log("zerodds version:", Runtime.version());

  const domain = 100 + (process.pid % 50);
  const topic = "TsSmokeTopic";
  const typeName = "RawBytes";

  const rt = new Runtime(domain);
  try {
    const w = new Writer(rt, topic, typeName, true);
    const r = new Reader(rt, topic, typeName, true);
    try {
      const matched = w.waitForMatched(1, 5000);
      assert.equal(matched, true, "wait_for_matched timeout");

      const payload = new Uint8Array([0xde, 0xad, 0xbe, 0xef]);
      for (let i = 0; i < 5; i++) {
        w.write(payload);
        await sleep(10);
      }

      let received = 0;
      for (let i = 0; i < 100; i++) {
        const sample = r.take();
        if (sample && sample.length > 0) {
          received++;
        } else {
          await sleep(20);
        }
      }
      console.log(`OK: ${received} samples received`);
      assert.ok(received >= 1, `expected ≥1 sample, got ${received}`);
    } finally {
      w.destroy();
      r.destroy();
    }
  } finally {
    rt.destroy();
  }
});

test("ts version string is non-empty", () => {
  const v = Runtime.version();
  assert.ok(v.length > 0, "version empty");
});

test("dds-psm-cxx api: participant lifecycle", () => {
  const dp = DomainParticipantFactory.createParticipant(0);
  try {
    assert.equal(dp.domainId(), 0, "domain id roundtrip");
  } finally {
    dp.destroy();
  }
});

test("dds-psm-cxx api: topic + writer/reader lifecycle", () => {
  const dp = DomainParticipantFactory.createParticipant(1);
  try {
    const t = Topic.create(dp, "TsTopic", ByteSeqTraits);
    const pub = Publisher.create(dp);
    const dw = DataWriter.create(pub, t);
    const sub = Subscriber.create(dp);
    const dr = DataReader.create(sub, t);
    dr.destroy();
    sub.destroy();
    dw.destroy();
    pub.destroy();
    t.destroy();
  } finally {
    dp.destroy();
  }
});

test("dds-psm-cxx api: GuardCondition + WaitSet", () => {
  const ws = new WaitSet();
  const gc = new GuardCondition();
  try {
    assert.equal(gc.getTriggerValue(), false, "guard initial false");
    gc.setTriggerValue(true);
    assert.equal(gc.getTriggerValue(), true, "guard set true");
    ws.attach(gc);
  } finally {
    gc.destroy();
    ws.destroy();
  }
});
