// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

/**
 * A DDS data path in TypeScript, for measuring the binding layer against.
 *
 * DDS-TS 1.0 §C.5 requires a conformant WASM binding to pass four tests:
 * a publish/subscribe round trip, a listener that fires, a delete that
 * cascades, and a boxed `any` that survives the trip. Those are tests of
 * {@link bindWasmBackend} — the handle checks, the cascade, the copies,
 * the deferral — and running them against a real `.wasm` module would
 * measure the module as well, in a test that needs a build of it to fail
 * or pass.
 *
 * So this: a {@link WasmBackend} that keeps its samples in a `Map`.
 * Every rule Annex C states about the *binding* is measurable against
 * it, and a failure is the binding's, because there is no wire and no
 * module.
 *
 * What it is not: a DDS implementation. There is no discovery, no
 * reliability, no history depth, no QoS at all. Matching is by domain
 * and topic name, delivery is immediate and in order, and the sample
 * states it reports are the ones a fresh take produces. A test that
 * needs any of the rest belongs against the Rust core.
 *
 * It is deliberately synchronous — `writeSample` runs the listener
 * straight through, in the writer's own turn. That is precisely what
 * §C.2.4 forbids a *binding* from doing, and it is what makes the
 * deferral in {@link bindWasmBackend} measurable: if the wrapper ever
 * stopped queueing, this backend would deliver inside `writeSample` and
 * the C.5.2 test would see it.
 */

import { makeDdsGuid } from "./wasm.js";
import type { DdsGuid, Sample, WasmBackend } from "./wasm.js";

/** The twelve-octet prefix every writer of one backend instance shares. */
const GUID_PREFIX_LENGTH = 12;

/** One participant. */
interface Participant {
  readonly domainId: number;
}

/** One topic, and the participant it belongs to. */
interface Topic {
  readonly participant: number;
  readonly name: string;
  readonly typeName: string;
}

/** A publisher or a subscriber: a participant and nothing else. */
interface Group {
  readonly participant: number;
}

/** One writer: the publisher it was made on and the topic it writes. */
interface Writer {
  readonly publisher: number;
  readonly topic: number;
  /** The next sequence number this writer will stamp. */
  next: bigint;
}

/** One reader: the subscriber, the topic, and what has arrived. */
interface Reader {
  readonly subscriber: number;
  readonly topic: number;
  readonly queue: Sample[];
  listener: ((r: number) => void) | null;
}

/**
 * An in-memory {@link WasmBackend}.
 *
 * Handles start at 1 and are never reused, so a handle that has been
 * deleted names nothing else afterwards — which is what lets the
 * binding layer's `RangeError` say which handle it was.
 */
export function createTestBackend(): WasmBackend {
  const participants = new Map<number, Participant>();
  const topics = new Map<number, Topic>();
  const publishers = new Map<number, Group>();
  const subscribers = new Map<number, Group>();
  const writers = new Map<number, Writer>();
  const readers = new Map<number, Reader>();
  let nextHandle = 1;

  /** The next unused handle. */
  function fresh(): number {
    const handle = nextHandle;
    nextHandle += 1;
    return handle;
  }

  /** The domain a topic sits in. */
  function domainOf(topic: Topic): number {
    return participants.get(topic.participant)?.domainId ?? -1;
  }

  /** A GUID that names one writer and no other. */
  function guidOf(writer: number): DdsGuid {
    const prefix = new Uint8Array(GUID_PREFIX_LENGTH);
    // Octets 0..1 are the vendor id in DDSI-RTPS 2.5 §8.2.4.1; the
    // rest of the prefix is this backend's own numbering, which is all
    // an in-memory data path has to tell its writers apart with.
    prefix[0] = 0x01;
    prefix[1] = 0x20;
    prefix[8] = (writer >>> 24) & 0xff;
    prefix[9] = (writer >>> 16) & 0xff;
    prefix[10] = (writer >>> 8) & 0xff;
    prefix[11] = writer & 0xff;
    // The entity kind of a user-defined writer with key: DDSI-RTPS 2.5
    // §9.3.1.2, `0x02` in the low octet.
    return makeDdsGuid(prefix, ((writer << 8) >>> 0) | 0x02);
  }

  return {
    createParticipant(domainId: number): number {
      const handle = fresh();
      participants.set(handle, { domainId });
      return handle;
    },

    deleteParticipant(p: number): void {
      participants.delete(p);
    },

    createTopic(p: number, name: string, typeName: string): number {
      const handle = fresh();
      topics.set(handle, { participant: p, name, typeName });
      return handle;
    },

    deleteTopic(t: number): void {
      topics.delete(t);
    },

    createPublisher(p: number): number {
      const handle = fresh();
      publishers.set(handle, { participant: p });
      return handle;
    },

    createSubscriber(p: number): number {
      const handle = fresh();
      subscribers.set(handle, { participant: p });
      return handle;
    },

    deletePublisher(pub: number): void {
      publishers.delete(pub);
    },

    deleteSubscriber(sub: number): void {
      subscribers.delete(sub);
    },

    createDataWriter(pub: number, topic: number): number {
      const handle = fresh();
      writers.set(handle, { publisher: pub, topic, next: 1n });
      return handle;
    },

    createDataReader(sub: number, topic: number): number {
      const handle = fresh();
      readers.set(handle, { subscriber: sub, topic, queue: [], listener: null });
      return handle;
    },

    writeSample(w: number, xcdr2: Uint8Array): void {
      const writer = writers.get(w);
      const topic = writer === undefined ? undefined : topics.get(writer.topic);
      if (writer === undefined || topic === undefined) {
        throw new RangeError(`writer ${w} has no topic`);
      }
      const sequenceNumber = writer.next;
      writer.next += 1n;
      const info = {
        validData: true,
        sampleState: "not_read",
        viewState: "new",
        instanceState: "alive",
        sourceTimestampNs: BigInt(Date.now()) * 1_000_000n,
        sequenceNumber,
        publicationHandle: guidOf(w),
        instanceHandle: BigInt(w),
      } as const;
      for (const [handle, reader] of readers) {
        const other = topics.get(reader.topic);
        if (
          other === undefined ||
          other.name !== topic.name ||
          domainOf(other) !== domainOf(topic)
        ) {
          continue;
        }
        // A copy per reader: two readers that were handed one array
        // would see each other's writes into it.
        reader.queue.push({ bytes: xcdr2.slice(), info });
        // Straight through, in the writer's turn. See the file comment:
        // the deferral is the binding layer's and this is what proves
        // it happens.
        reader.listener?.(handle);
      }
    },

    takeSamples(r: number, max: number): ReadonlyArray<Sample> {
      const reader = readers.get(r);
      if (reader === undefined) {
        return [];
      }
      return reader.queue.splice(0, max);
    },

    deleteDataWriter(w: number): void {
      writers.delete(w);
    },

    deleteDataReader(r: number): void {
      readers.delete(r);
    },

    setDataAvailableListener(r: number, cb: ((r: number) => void) | null): void {
      const reader = readers.get(r);
      if (reader !== undefined) {
        reader.listener = cb;
      }
    },
  };
}
