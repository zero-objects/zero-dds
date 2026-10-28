// generated-typesupport.test.ts — a TypeSupport that `zerodds-idlc generate
// --lang typescript` emitted, through the typed facade.
//
// The hand-written TypeSupports of smoke.test.ts and qos.test.ts ignore every
// argument after the bytes, so they could not show what the facade passes to a
// generated decoder: the C ABI's SampleInfo carries the XCDR *version*
// (0 = XCDR1, 1 = XCDR2), while a generated TypeSupport takes XTypes 1.3
// §7.6.3.1.1's DataRepresentationId (XCDR = 0, XCDR2 = 2) and refuses any
// other. Measured before the fix in zerodds-examples/idl-conformance/
// typescript: `reader.take()` threw "this generated codec does not implement
// the requested XCDR representation" for every sample.

import test from "node:test";
import assert from "node:assert/strict";
import { DomainParticipantFactory, HistoryKind, defaultDataReaderQos, defaultDataWriterQos } from "../src/index.js";
import { InstanceState } from "../src/native.js";
import { probe } from "./gen/typed_probe.js";
import { uniqueDomain } from "./support/test-domains.js";

// One domain of the band of `crates/test-domains`, leased for the life of the
// process (`support/test-domains.ts`).
const DOMAIN = uniqueDomain();

test("a generated TypeSupport reads back through take() what its writer wrote", async () => {
  const participant = DomainParticipantFactory.instance().createParticipant(DOMAIN);
  try {
    const topic = participant.createTypedTopic("GeneratedTypeSupportProbe", probe.ReadingTypeSupport);
    const writer = participant.createPublisher().createTypedWriter(topic);
    const reader = participant.createSubscriber().createTypedReader(topic);
    await writer.waitForMatchedSubscription(1, 5000);
    await reader.waitForMatchedPublication(1, 5000);

    const sent: probe.Reading = { id: 7, value: 2.5, label: "seven" };
    writer.write(sent);
    assert.equal(await reader.waitForData(3000), true, "the sample never arrived");
    const got = reader.take();
    assert.deepEqual(got, [sent]);

    // The loan-batch path decodes through the same adapter.
    const second: probe.Reading = { id: 8, value: -1.25, label: "eight" };
    writer.write(second);
    assert.equal(await reader.waitForData(3000), true, "the second sample never arrived");
    const infos = reader.takeWithInfo();
    assert.deepEqual(infos.map((s) => s.data), [second]);
  } finally {
    participant.destroy();
  }
});

// `waitForData()` takes what it finds out of the native reader. It kept only
// the payloads of valid samples, for `take()`, and threw away their
// SampleInfo and every lifecycle-only marker — so `waitForData()` followed by
// `takeWithInfo()` saw neither the sample nor the dispose.
test("waitForData() keeps the samples and the lifecycle markers it drains for takeWithInfo()", async () => {
  const participant = DomainParticipantFactory.instance().createParticipant(DOMAIN);
  try {
    const topic = participant.createTypedTopic("GeneratedTypeSupportLifecycle", probe.ReadingTypeSupport);
    const writer = participant.createPublisher().createTypedWriter(topic);
    const reader = participant.createSubscriber().createTypedReader(topic);
    await writer.waitForMatchedSubscription(1, 5000);
    await reader.waitForMatchedPublication(1, 5000);

    const sample: probe.Reading = { id: 11, value: 0.5, label: "eleven" };
    writer.write(sample);
    writer.dispose(sample);

    const seen: { data: probe.Reading | null; instanceState: number; validData: boolean }[] = [];
    const deadline = Date.now() + 3000;
    while (Date.now() < deadline && !seen.some((s) => !s.validData)) {
      if (await reader.waitForData(500)) seen.push(...reader.takeWithInfo());
    }
    assert.deepEqual(
      seen.filter((s) => s.validData).map((s) => s.data),
      [sample],
      "the written sample did not reach takeWithInfo()",
    );
    assert.ok(
      seen.some((s) => !s.validData && s.instanceState === InstanceState.NotAliveDisposed),
      `the dispose did not reach takeWithInfo(): ${JSON.stringify(seen, (_k, v) => (typeof v === "bigint" ? v.toString() : v))}`,
    );
  } finally {
    participant.destroy();
  }
});

// A keyed sample written through the typed facade carries its KeyHash:
// `zerodds_dw_write` writes every sample under the all-zero key, so two
// samples of one key arrived as two instances, and EXCLUSIVE ownership — which
// arbitrates per instance — never engaged (measured in zerodds-examples
// idl-conformance/_qos/typescript: key id=5 got instance handles 1 and 2).
test("samples of one key share an instance handle, and another key gets another", async () => {
  const participant = DomainParticipantFactory.instance().createParticipant(DOMAIN);
  try {
    const topic = participant.createTypedTopic("GeneratedTypeSupportInstances", probe.ReadingTypeSupport);
    // Two samples of key 5 have to arrive: under the default KEEP_LAST(1) the
    // writer and the reader keep only the instance's most recent one (DDS 1.4
    // §2.2.3.18), and two of the three arrived (measured).
    const wq = defaultDataWriterQos(); wq.history.kind = HistoryKind.KeepAll;
    const rq = defaultDataReaderQos(); rq.history.kind = HistoryKind.KeepAll;
    const writer = participant.createPublisher().createTypedWriter(topic, wq);
    const reader = participant.createSubscriber().createTypedReader(topic, rq);
    await writer.waitForMatchedSubscription(1, 5000);
    await reader.waitForMatchedPublication(1, 5000);

    writer.write({ id: 5, value: 1, label: "a" });
    writer.write({ id: 5, value: 2, label: "b" });
    writer.write({ id: 6, value: 3, label: "c" });
    const seen: { id: number; handle: bigint }[] = [];
    const deadline = Date.now() + 3000;
    while (Date.now() < deadline && seen.length < 3) {
      if (await reader.waitForData(500)) {
        for (const s of reader.takeWithInfo()) {
          if (s.validData && s.data) seen.push({ id: s.data.id, handle: s.instanceHandle });
        }
      }
    }
    const handlesOf = (id: number) => new Set(seen.filter((s) => s.id === id).map((s) => s.handle.toString()));
    assert.equal(seen.length, 3, `expected three samples, got ${seen.length}`);
    assert.equal(handlesOf(5).size, 1, `key 5 arrived as ${handlesOf(5).size} instances`);
    assert.notDeepEqual([...handlesOf(5)], [...handlesOf(6)], "keys 5 and 6 share an instance");
  } finally {
    participant.destroy();
  }
});
