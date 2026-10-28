// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

/**
 * The four conformance tests of DDS-TS 1.0 Annex C, §C.5.
 *
 * C.5.1 is a publish/subscribe round trip through the binding; C.5.2 is
 * a listener that fires and does **not** fire inside `writeSample`;
 * C.5.3 is the cascade a `deleteParticipant` performs and the throw a
 * stale or double-deleted handle produces; C.5.4 is a boxed `any` that
 * survives the trip.
 *
 * All four are run against `createTestBackend()` — the in-memory data
 * path in `src/test_backend.ts` — and not against a `.wasm` module. What
 * §C.5 asks about is the **binding layer**: the invalid-handle sentinel,
 * the cascade, the borrowed buffers, the deferral. Running it against a
 * compiled module would measure the module too, in a test that needs a
 * build of it to say anything, and none of the four clauses is about the
 * module.
 *
 * The octets are real. The sample is written with the runtime's own
 * XCDR2 writer and read back with its reader, because §C.3.1 makes the
 * octet sequence the canonical wire form and a test that passed an
 * object across would be measuring a boundary the Profile forbids.
 */

import { Endianness } from "../src/endianness.js";
import { Reader } from "../src/reader.js";
import { Writer } from "../src/writer.js";
import { bindWasmBackend, boxAny, createTestBackend, registerType, unboxAny } from "../src/index.js";
import type { DataReaderHandle, DdsAny, DdsTypeDescriptor, Sample } from "../src/index.js";

declare const process: { exit(code: number): never };

let checked = 0;
let failures = 0;

/** One conformance test, with its number. */
function test(name: string, run: () => void): void {
  checked += 1;
  try {
    run();
  } catch (error) {
    failures += 1;
    console.log(`${name}: ${String(error)}`);
  }
}

/** The asynchronous form, for the one test that has to yield a turn. */
async function testAsync(name: string, run: () => Promise<void>): Promise<void> {
  checked += 1;
  try {
    await run();
  } catch (error) {
    failures += 1;
    console.log(`${name}: ${String(error)}`);
  }
}

function assert(held: boolean, said: string): void {
  if (!held) {
    throw new Error(said);
  }
}

/** That a call throws a `RangeError` naming the handle (§C.5.3). */
function throwsRangeError(run: () => unknown, naming: string, said: string): void {
  try {
    run();
  } catch (error) {
    assert(error instanceof RangeError, `${said}: threw ${String(error)}, not a RangeError`);
    assert(
      String(error).includes(naming),
      `${said}: the message does not name ${naming} — ${String(error)}`,
    );
    return;
  }
  throw new Error(said);
}

/**
 * The XCDR2 form of `struct PrimitiveSample { long v; };`.
 *
 * `maxAlign` of 4 is XCDR2's (§7.4.1); little-endian is the default the
 * Profile takes for `--xcdr-endianness` (§C.3.1).
 */
function encodePrimitiveSample(v: number): Uint8Array {
  const w = new Writer(Endianness.Little, 4);
  w.writeI32(v);
  return w.finish();
}

/** The other direction. */
function decodePrimitiveSample(bytes: Uint8Array): number {
  return new Reader(bytes, Endianness.Little, 4).readI32();
}

// ---------------------------------------------------------------------
// C.5.1 — round-trip publish/subscribe.
// ---------------------------------------------------------------------

test("C.5.1", () => {
  const dds = bindWasmBackend(createTestBackend());
  const participant = dds.createParticipant(0);
  const topic = dds.createTopic(participant, "Echo", "PrimitiveSample");
  const publisher = dds.createPublisher(participant);
  const subscriber = dds.createSubscriber(participant);
  const writer = dds.createDataWriter(publisher, topic);
  const reader = dds.createDataReader(subscriber, topic);

  dds.writeSample(writer, encodePrimitiveSample(42));
  const samples: ReadonlyArray<Sample> = dds.takeSamples(reader, 10);
  assert(samples.length === 1, `${samples.length} samples came back, not 1`);
  const sample = samples[0];
  assert(sample !== undefined, "the sample is not there");
  if (sample === undefined) {
    return;
  }
  assert(sample.info.validData, "the sample says its data is not valid");
  assert(decodePrimitiveSample(sample.bytes) === 42, "the value came back changed");
  assert(sample.info.sequenceNumber === 1n, "the first sample is not sequence 1");
  assert(sample.info.publicationHandle.prefix.length === 12, "the GUID prefix is not 12 octets");

  // §C.3.2: the module treats what it is handed as borrowed, and what
  // comes back is owned by JavaScript. Writing into either buffer
  // afterwards must not reach the other.
  const octets = encodePrimitiveSample(7);
  dds.writeSample(writer, octets);
  octets[0] = 0xff;
  const second = dds.takeSamples(reader, 1)[0];
  assert(second !== undefined && decodePrimitiveSample(second.bytes) === 7, "the buffer was aliased");

  dds.deleteParticipant(participant);
});

// ---------------------------------------------------------------------
// C.5.2 — the listener fires, and not inside `writeSample`.
// ---------------------------------------------------------------------

const listenerTest = testAsync("C.5.2", async () => {
  const dds = bindWasmBackend(createTestBackend());
  const participant = dds.createParticipant(0);
  const topic = dds.createTopic(participant, "Echo", "PrimitiveSample");
  const publisher = dds.createPublisher(participant);
  const subscriber = dds.createSubscriber(participant);
  const writer = dds.createDataWriter(publisher, topic);
  const reader = dds.createDataReader(subscriber, topic);

  let fired = 0;
  let firedFor: DataReaderHandle | undefined;
  dds.setDataAvailableListener(reader, (r) => {
    fired += 1;
    firedFor = r;
  });

  dds.writeSample(writer, encodePrimitiveSample(1));
  // The backend calls its listener straight through, in this turn. The
  // binding queues, so nothing has run yet — which is the clause.
  assert(fired === 0, "the listener ran inside the synchronous writeSample");
  await Promise.resolve();
  assert(fired === 1, `the listener ran ${fired} times, not once`);
  assert(firedFor === reader, "the listener was told about another reader");

  // `null` unregisters (§C.2.4).
  dds.setDataAvailableListener(reader, null);
  dds.writeSample(writer, encodePrimitiveSample(2));
  await Promise.resolve();
  assert(fired === 1, "the unregistered listener ran again");

  dds.deleteParticipant(participant);
});

// ---------------------------------------------------------------------
// C.5.3 — resource cleanup.
// ---------------------------------------------------------------------

test("C.5.3", () => {
  const dds = bindWasmBackend(createTestBackend());
  const participant = dds.createParticipant(0);
  const topic = dds.createTopic(participant, "Echo", "PrimitiveSample");
  const publisher = dds.createPublisher(participant);
  const subscriber = dds.createSubscriber(participant);
  const writer = dds.createDataWriter(publisher, topic);
  const reader = dds.createDataReader(subscriber, topic);

  dds.deleteParticipant(participant);

  // Every derived handle is invalidated, down two levels: the writer
  // was made on a publisher that was made on the participant.
  throwsRangeError(
    () => dds.writeSample(writer, encodePrimitiveSample(1)),
    String(writer),
    "a writer under a deleted participant still wrote",
  );
  throwsRangeError(
    () => dds.takeSamples(reader, 1),
    String(reader),
    "a reader under a deleted participant still took",
  );
  throwsRangeError(
    () => dds.deleteTopic(topic),
    String(topic),
    "a topic under a deleted participant was deleted again",
  );
  throwsRangeError(
    () => dds.deletePublisher(publisher),
    String(publisher),
    "a publisher under a deleted participant was deleted again",
  );
  throwsRangeError(
    () => dds.deleteSubscriber(subscriber),
    String(subscriber),
    "a subscriber under a deleted participant was deleted again",
  );
  // A double delete throws rather than silently succeeding.
  throwsRangeError(
    () => dds.deleteParticipant(participant),
    String(participant),
    "a participant was deleted twice",
  );

  // The zero sentinel is run-time enforced, because `0 as Handle`
  // type-checks (§C.1.1).
  const second = bindWasmBackend(createTestBackend());
  throwsRangeError(
    () => second.deleteParticipant(0 as never),
    "0",
    "handle 0 was accepted",
  );
});

// ---------------------------------------------------------------------
// C.5.4 — a boxed `any` across the boundary.
//
//   @final struct Pose { long x; long y; };
//   @final @topic("Envelope") struct Envelope { long id; any payload; };
//
// §C.3.3: there is no separate wire object for a `DdsAny`. The producer
// boxes, the whole compound topic is serialised, the consumer
// deserialises and unboxes. The serialiser below is what the generator
// emits for that shape: the id, then the box's `typeId`, then the
// payload's members.
// ---------------------------------------------------------------------

interface Pose {
  x: number;
  y: number;
}

function isPose(v: unknown): v is Pose {
  return (
    typeof v === "object" &&
    v !== null &&
    typeof (v as Pose).x === "number" &&
    typeof (v as Pose).y === "number"
  );
}

const PoseType: DdsTypeDescriptor<Pose> = {
  kind: "struct",
  name: "wasm::Pose",
  extensibility: "final",
  nested: true,
  fields: [
    {
      name: "x",
      id: 0,
      type: { kind: "primitive", name: "int32" },
      key: false,
      optional: false,
      mustUnderstand: false,
    },
    {
      name: "y",
      id: 1,
      type: { kind: "primitive", name: "int32" },
      key: false,
      optional: false,
      mustUnderstand: false,
    },
  ],
  typeGuard: isPose,
};

registerType(PoseType);

/** `struct Envelope { long id; any payload; }` as XCDR2 octets. */
function encodeEnvelope(id: number, payload: DdsAny): Uint8Array {
  const w = new Writer(Endianness.Little, 4);
  w.writeI32(id);
  w.writeString(payload.typeId);
  const pose = unboxAny(payload, PoseType);
  w.writeI32(pose.x);
  w.writeI32(pose.y);
  return w.finish();
}

/** The other direction, back to a box. */
function decodeEnvelope(bytes: Uint8Array): { id: number; payload: DdsAny } {
  const r = new Reader(bytes, Endianness.Little, 4);
  const id = r.readI32();
  const typeId = r.readString();
  const x = r.readI32();
  const y = r.readI32();
  return { id, payload: { typeId, value: { x, y } } };
}

test("C.5.4", () => {
  const dds = bindWasmBackend(createTestBackend());
  const participant = dds.createParticipant(0);
  const topic = dds.createTopic(participant, "Envelope", "Envelope");
  const publisher = dds.createPublisher(participant);
  const subscriber = dds.createSubscriber(participant);
  const writer = dds.createDataWriter(publisher, topic);
  const reader = dds.createDataReader(subscriber, topic);

  const sent: Pose = { x: 3, y: -4 };
  dds.writeSample(writer, encodeEnvelope(9, boxAny(PoseType, sent)));

  const sample = dds.takeSamples(reader, 1)[0];
  assert(sample !== undefined, "nothing came back");
  if (sample === undefined) {
    return;
  }
  const envelope = decodeEnvelope(sample.bytes);
  assert(envelope.id === 9, "the envelope's own member came back changed");
  const back = unboxAny(envelope.payload, PoseType);
  assert(PoseType.typeGuard(back), "the unboxed value is not what the descriptor describes");
  assert(back.x === sent.x && back.y === sent.y, "the boxed value came back changed");

  dds.deleteParticipant(participant);
});

await listenerTest;

console.log(`${checked - failures} of ${checked} WASM-binding conformance tests held`);
process.exit(failures === 0 ? 0 : 1);
