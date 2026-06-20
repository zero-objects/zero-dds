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

// Fluent instance-method facade (the @zerodds/node quickstart API): factory
// instance() -> participant.createBytesTopic/createPublisher/createSubscriber ->
// publisher.createBytesWriter / subscriber.createBytesReader, async match waits,
// waitForData + iterable take().
test("fluent facade: bytes pub/sub roundtrip", async () => {
  const participant = DomainParticipantFactory.instance().createParticipant(7);
  try {
    const topic = participant.createBytesTopic("FacadeChatter");
    const writer = participant.createPublisher().createBytesWriter(topic);
    const reader = participant.createSubscriber().createBytesReader(topic);

    await writer.waitForMatchedSubscription(1, 5000);
    await reader.waitForMatchedPublication(1, 5000);

    writer.write(new TextEncoder().encode("hello"));
    const ready = await reader.waitForData(3000);
    assert.equal(ready, true, "waitForData timed out");

    const got = [...reader.take()].map((s) => new TextDecoder().decode(s));
    assert.ok(got.includes("hello"), `expected 'hello', got ${JSON.stringify(got)}`);
  } finally {
    participant.destroy();
  }
});

// Async layer: writeAsync (Promise), takeAsync (Promise<Sample[]>),
// streamSamples (AsyncIterable).
test("async layer: writeAsync / takeAsync / streamSamples", async () => {
  const participant = DomainParticipantFactory.instance().createParticipant(8);
  try {
    const topic = participant.createBytesTopic("AsyncChatter");
    const writer = participant.createPublisher().createBytesWriter(topic);
    const reader = participant.createSubscriber().createBytesReader(topic);

    await writer.waitForMatchedSubscription(1, 5000);
    await reader.waitForMatchedPublication(1, 5000);

    await writer.writeAsync(new TextEncoder().encode("a1"));
    await reader.waitForData(3000);
    const taken = await reader.takeAsync();
    assert.ok(taken.length >= 1, "takeAsync returned no samples");
    assert.equal(new TextDecoder().decode(taken[0]), "a1");

    await writer.writeAsync(new TextEncoder().encode("s1"));
    let seen = 0;
    for await (const sample of reader.streamSamples()) {
      assert.equal(new TextDecoder().decode(sample), "s1");
      if (++seen >= 1) break;
    }
    assert.equal(seen, 1, "streamSamples yielded nothing");
  } finally {
    participant.destroy();
  }
});

// Typed facade: createTypedTopic + createTypedWriter with a TypeSupport.
test("typed facade: createTypedTopic + createTypedWriter roundtrip", async () => {
  interface Temperature { celsius: number; sensor_id: string; }
  const TemperatureTypeSupport = {
    typeName: "Temperature",
    encode(s: Temperature): Uint8Array {
      const idEnc = new TextEncoder().encode(s.sensor_id);
      const buf = new Uint8Array(4 + idEnc.length);
      new DataView(buf.buffer).setInt32(0, s.celsius, true);
      buf.set(idEnc, 4);
      return buf;
    },
    decode(b: Uint8Array): Temperature {
      const celsius = new DataView(b.buffer, b.byteOffset, b.byteLength).getInt32(0, true);
      const sensor_id = new TextDecoder().decode(b.subarray(4));
      return { celsius, sensor_id };
    },
  };
  const participant = DomainParticipantFactory.instance().createParticipant(9);
  try {
    const topic = participant.createTypedTopic("Temp", TemperatureTypeSupport);
    const writer = participant.createPublisher().createTypedWriter(topic);
    const reader = participant.createSubscriber().createTypedReader(topic);

    await writer.waitForMatchedSubscription(1, 5000);
    await reader.waitForMatchedPublication(1, 5000);

    await writer.write({ celsius: 23, sensor_id: "A7" });
    await reader.waitForData(3000);
    const got = reader.take();
    assert.ok(got.length >= 1, "typed take returned nothing");
    assert.deepEqual(got[0], { celsius: 23, sensor_id: "A7" });
  } finally {
    participant.destroy();
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
