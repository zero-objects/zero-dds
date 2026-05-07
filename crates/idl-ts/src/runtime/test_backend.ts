// SPDX-License-Identifier: Apache-2.0
// DDS-TS 1.0 — In-memory test backend for the WASM-Bindings
// Profile (§C.5).
//
// This is a pure-TypeScript reference backend that satisfies the
// `WasmBackend`-trait without any actual DDS / RTPS stack. It
// loops written samples directly to the matching readers in the
// same process. Use only for §C.5 round-trip conformance tests
// and developer prototyping; production deployments bind a real
// WASM module compiled from a DDS implementation.

import type {
  WasmBackend,
  ParticipantHandle, TopicHandle,
  PublisherHandle, SubscriberHandle,
  DataWriterHandle, DataReaderHandle,
  Sample, DataAvailableCallback,
} from "./wasm.js";
import { makeDdsGuid } from "./wasm.js";

interface TopicEntry {
  readonly participant: ParticipantHandle;
  readonly name: string;
  readonly typeName: string;
}

interface ReaderEntry {
  readonly subscriber: SubscriberHandle;
  readonly topic: TopicHandle;
  readonly queue: Sample[];
  listener: DataAvailableCallback | null;
}

interface WriterEntry {
  readonly publisher: PublisherHandle;
  readonly topic: TopicHandle;
  sequence: bigint;
}

class InMemoryBackend implements WasmBackend {
  private nextHandle = 1;
  private participants = new Map<ParticipantHandle, number>();
  private topics       = new Map<TopicHandle, TopicEntry>();
  private publishers   = new Map<PublisherHandle, ParticipantHandle>();
  private subscribers  = new Map<SubscriberHandle, ParticipantHandle>();
  private writers      = new Map<DataWriterHandle, WriterEntry>();
  private readers      = new Map<DataReaderHandle, ReaderEntry>();

  private alloc<T extends number>(): T {
    const v = this.nextHandle;
    this.nextHandle += 1;
    return v as T;
  }

  createParticipant(domainId: number): ParticipantHandle {
    const h = this.alloc<ParticipantHandle>();
    this.participants.set(h, domainId);
    return h;
  }

  deleteParticipant(p: ParticipantHandle): void {
    if (!this.participants.delete(p)) {
      throw new RangeError(`unknown participant handle ${p}`);
    }
  }

  createTopic(p: ParticipantHandle, name: string, typeName: string): TopicHandle {
    if (!this.participants.has(p)) {
      throw new RangeError(`unknown participant handle ${p}`);
    }
    const h = this.alloc<TopicHandle>();
    this.topics.set(h, { participant: p, name, typeName });
    return h;
  }

  deleteTopic(t: TopicHandle): void {
    if (!this.topics.delete(t)) {
      throw new RangeError(`unknown topic handle ${t}`);
    }
  }

  createPublisher(p: ParticipantHandle): PublisherHandle {
    if (!this.participants.has(p)) {
      throw new RangeError(`unknown participant handle ${p}`);
    }
    const h = this.alloc<PublisherHandle>();
    this.publishers.set(h, p);
    return h;
  }

  createSubscriber(p: ParticipantHandle): SubscriberHandle {
    if (!this.participants.has(p)) {
      throw new RangeError(`unknown participant handle ${p}`);
    }
    const h = this.alloc<SubscriberHandle>();
    this.subscribers.set(h, p);
    return h;
  }

  deletePublisher(pub: PublisherHandle): void {
    if (!this.publishers.delete(pub)) {
      throw new RangeError(`unknown publisher handle ${pub}`);
    }
  }

  deleteSubscriber(sub: SubscriberHandle): void {
    if (!this.subscribers.delete(sub)) {
      throw new RangeError(`unknown subscriber handle ${sub}`);
    }
  }

  createDataWriter(pub: PublisherHandle, topic: TopicHandle): DataWriterHandle {
    if (!this.publishers.has(pub)) {
      throw new RangeError(`unknown publisher handle ${pub}`);
    }
    if (!this.topics.has(topic)) {
      throw new RangeError(`unknown topic handle ${topic}`);
    }
    const h = this.alloc<DataWriterHandle>();
    this.writers.set(h, { publisher: pub, topic, sequence: 0n });
    return h;
  }

  createDataReader(sub: SubscriberHandle, topic: TopicHandle): DataReaderHandle {
    if (!this.subscribers.has(sub)) {
      throw new RangeError(`unknown subscriber handle ${sub}`);
    }
    if (!this.topics.has(topic)) {
      throw new RangeError(`unknown topic handle ${topic}`);
    }
    const h = this.alloc<DataReaderHandle>();
    this.readers.set(h, { subscriber: sub, topic, queue: [], listener: null });
    return h;
  }

  writeSample(w: DataWriterHandle, xcdr2: Uint8Array): void {
    const writer = this.writers.get(w);
    if (writer === undefined) {
      throw new RangeError(`unknown writer handle ${w}`);
    }
    writer.sequence += 1n;
    const guid = makeDdsGuid(new Uint8Array(12), 0);
    const info = {
      validData:         true,
      sampleState:       "not_read" as const,
      viewState:         "new" as const,
      instanceState:     "alive" as const,
      sourceTimestampNs: BigInt(Date.now()) * 1_000_000n,
      sequenceNumber:    writer.sequence,
      publicationHandle: guid,
      instanceHandle:    0n,
    };
    const sample: Sample = { bytes: xcdr2.slice(), info };
    // Loop to every reader on the same topic.
    for (const reader of this.readers.values()) {
      if (reader.topic === writer.topic) {
        reader.queue.push(sample);
        if (reader.listener !== null) {
          // Schedule listener callback asynchronously to match
          // the §C.2.4 contract.
          const cb = reader.listener;
          const handle = [...this.readers.entries()]
            .find(([, e]) => e === reader)?.[0];
          if (handle !== undefined) {
            queueMicrotask(() => cb(handle));
          }
        }
      }
    }
  }

  takeSamples(r: DataReaderHandle, max: number): ReadonlyArray<Sample> {
    const reader = this.readers.get(r);
    if (reader === undefined) {
      throw new RangeError(`unknown reader handle ${r}`);
    }
    const taken = reader.queue.splice(0, max);
    return taken;
  }

  deleteDataWriter(w: DataWriterHandle): void {
    if (!this.writers.delete(w)) {
      throw new RangeError(`unknown writer handle ${w}`);
    }
  }

  deleteDataReader(r: DataReaderHandle): void {
    if (!this.readers.delete(r)) {
      throw new RangeError(`unknown reader handle ${r}`);
    }
  }

  setDataAvailableListener(
    r: DataReaderHandle,
    cb: DataAvailableCallback | null,
  ): void {
    const reader = this.readers.get(r);
    if (reader === undefined) {
      throw new RangeError(`unknown reader handle ${r}`);
    }
    reader.listener = cb;
  }
}

/** Returns a fresh in-memory backend instance. */
export function createInMemoryBackend(): WasmBackend {
  return new InMemoryBackend();
}
