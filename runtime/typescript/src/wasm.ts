// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

/**
 * The WASM-Bindings Profile: handles, samples, and the layer that guards
 * them.
 *
 * DDS-TS 1.0 Annex C. A TypeScript application talks to a DDS
 * implementation compiled to WebAssembly through the operations of
 * §C.2 — create a participant, a topic, a writer, a reader; write XCDR2
 * octets; take them back. This file declares those operations, the
 * handle and sample types they use, and {@link bindWasmBackend}, which
 * wraps a raw module export table in the run-time invariants Annex C
 * requires and the type system cannot state.
 *
 * # What crosses the boundary
 *
 * `Uint8Array`, always. §C.3.1 makes the XCDR2 octet sequence the
 * canonical wire form and forbids exposing a JavaScript object graph as
 * one: a binding that passed `{ x: 1 }` across and serialised on the far
 * side would have two serialisers, and the one on the JavaScript side
 * would be the one no other vendor implements. The generated codec turns
 * a sample into octets before it gets here.
 *
 * A boxed `any` is no exception (§C.3.3). `boxAny` on the producing
 * side, the generated serialiser, the octets, the generated
 * deserialiser, `unboxAny` on the consuming side — the box is a
 * TypeScript-side narrowing device and never a wire object of its own.
 *
 * # Why the brands are string literals
 *
 * The same reason as `./branded.js` gives for `Char`: a
 * `unique symbol` brand is nominal across module boundaries, so a
 * handle from vendor A's binding would not satisfy vendor B's parameter
 * even though both are indices into the same resource table. The tags
 * below — `"participant"`, `"topic"`, `"publisher"`, `"subscriber"`,
 * `"writer"`, `"reader"` — and the property name `__dds_brand` are
 * normative (§C.1.1).
 *
 * The brand is compile-time only. `0 as DataReaderHandle` type-checks,
 * and 0 is the invalid-handle sentinel, so the check that matters is the
 * run-time one in {@link bindWasmBackend}.
 */

/** A domain participant (§C.1.1). */
export type ParticipantHandle = number & { readonly __dds_brand: "participant" };
/** A topic (§C.1.1). */
export type TopicHandle = number & { readonly __dds_brand: "topic" };
/** A publisher (§C.1.1). */
export type PublisherHandle = number & { readonly __dds_brand: "publisher" };
/** A subscriber (§C.1.1). */
export type SubscriberHandle = number & { readonly __dds_brand: "subscriber" };
/** A data writer (§C.1.1). */
export type DataWriterHandle = number & { readonly __dds_brand: "writer" };
/** A data reader (§C.1.1). */
export type DataReaderHandle = number & { readonly __dds_brand: "reader" };

/**
 * An OMG DDSI-RTPS GUID: a 12-octet `GuidPrefix` and a 32-bit
 * `EntityId`.
 *
 * DDS-TS 1.0 §C.1.2. The whole sixteen octets and not a 64-bit subset:
 * the prefix carries the vendor id, the host id, the application id and
 * the instance id (DDSI-RTPS 2.5 §8.2.4), and truncating it loses the
 * vendor and host components — which is exactly what tells two writers
 * apart in a multi-vendor or multi-host deployment.
 */
export interface DdsGuid {
  /** Twelve octets. */
  readonly prefix: Uint8Array;
  /** The entity id, as a 32-bit unsigned value. */
  readonly entityId: number;
}

/**
 * A GUID from its two parts, or a `RangeError`.
 *
 * DDS-TS 1.0 §C.1.2.
 */
export function makeDdsGuid(prefix: Uint8Array, entityId: number): DdsGuid {
  if (prefix.length !== 12) {
    throw new RangeError("DdsGuid.prefix requires 12 bytes");
  }
  if (!Number.isInteger(entityId) || entityId < 0 || entityId > 0xffffffff) {
    throw new RangeError("DdsGuid.entityId requires uint32");
  }
  return { prefix, entityId };
}

/**
 * What DDS knows about a sample besides its octets.
 *
 * DDS-TS 1.0 §C.1.2. A strict subset of the OMG DDS 1.4 `SampleInfo`
 * (§2.2.2.5.4): the fields this Profile exposes and no others. A
 * `validData` of false is a sample that carries only an instance state
 * change — a dispose, a writer leaving — and whose `bytes` say nothing.
 */
export interface SampleInfo {
  readonly validData: boolean;
  readonly sampleState: "read" | "not_read";
  readonly viewState: "new" | "not_new";
  readonly instanceState: "alive" | "not_alive_disposed" | "not_alive_no_writers";
  readonly sourceTimestampNs: bigint;
  readonly sequenceNumber: bigint;
  /** The full RTPS GUID of the writer that published it. */
  readonly publicationHandle: DdsGuid;
  /** Opaque, and 64 bits wide. */
  readonly instanceHandle: bigint;
}

/**
 * One sample: the XCDR2 octets exactly as they crossed the wire, and
 * what DDS knows about them.
 *
 * DDS-TS 1.0 §C.1.2.
 */
export interface Sample {
  readonly bytes: Uint8Array;
  readonly info: SampleInfo;
}

/** Called when a reader has data (§C.1.3). */
export type DataAvailableCallback = (r: DataReaderHandle) => void;

/**
 * The raw module surface — a `.wasm` export table, an N-API `.node`
 * module, or a reference implementation in TypeScript.
 *
 * Handles are plain numbers here: branding is the binding layer's, and
 * a WebAssembly export cannot carry a TypeScript type anyway. Everything
 * that Annex C requires *around* these calls — the invalid-handle
 * sentinel, the cascade on delete, the double-delete, the deferral of
 * listener delivery — is done by {@link bindWasmBackend} rather than
 * asked of each module, so that a browser build and a Node build behave
 * the same way without implementing the same rules twice (§C.4.2).
 */
export interface WasmBackend {
  createParticipant(domainId: number): number;
  deleteParticipant(p: number): void;
  createTopic(p: number, name: string, typeName: string): number;
  deleteTopic(t: number): void;
  createPublisher(p: number): number;
  createSubscriber(p: number): number;
  deletePublisher(pub: number): void;
  deleteSubscriber(sub: number): void;
  createDataWriter(pub: number, topic: number): number;
  createDataReader(sub: number, topic: number): number;
  writeSample(w: number, xcdr2: Uint8Array): void;
  takeSamples(r: number, max: number): ReadonlyArray<Sample>;
  deleteDataWriter(w: number): void;
  deleteDataReader(r: number): void;
  setDataAvailableListener(r: number, cb: ((r: number) => void) | null): void;
}

/**
 * The operations of §C.2, with branded handles and the Annex C
 * invariants enforced.
 *
 * The signatures are normative; the WebAssembly export names behind them
 * are not. A Node.js binding built on N-API exposes this same interface
 * so that an application retargets between browser and Node by changing
 * an import and nothing else (§C.4.2).
 */
export interface DdsBindings {
  createParticipant(domainId: number): ParticipantHandle;
  deleteParticipant(p: ParticipantHandle): void;
  createTopic(p: ParticipantHandle, name: string, typeName: string): TopicHandle;
  deleteTopic(t: TopicHandle): void;
  createPublisher(p: ParticipantHandle): PublisherHandle;
  createSubscriber(p: ParticipantHandle): SubscriberHandle;
  deletePublisher(pub: PublisherHandle): void;
  deleteSubscriber(sub: SubscriberHandle): void;
  createDataWriter(pub: PublisherHandle, topic: TopicHandle): DataWriterHandle;
  createDataReader(sub: SubscriberHandle, topic: TopicHandle): DataReaderHandle;
  writeSample(w: DataWriterHandle, xcdr2: Uint8Array): void;
  takeSamples(r: DataReaderHandle, max: number): ReadonlyArray<Sample>;
  deleteDataWriter(w: DataWriterHandle): void;
  deleteDataReader(r: DataReaderHandle): void;
  setDataAvailableListener(r: DataReaderHandle, cb: DataAvailableCallback | null): void;
}

/** The six resource kinds, under the brand tags §C.1.1 gives them. */
type HandleKind = "participant" | "topic" | "publisher" | "subscriber" | "writer" | "reader";

/** One live resource, and the resource it was created from. */
interface Resource {
  readonly kind: HandleKind;
  readonly value: number;
  /** What a delete of the parent takes with it. */
  readonly parents: ReadonlyArray<string>;
}

/** The table key. Two kinds may hold the same number, so both are in it. */
function slot(kind: HandleKind, value: number): string {
  return `${kind}:${value}`;
}

/**
 * Wrap a raw module in the invariants Annex C states.
 *
 * Five of them, and each one is a rule the type system cannot carry:
 *
 * 1. **The zero sentinel** (§C.1.1). `0 as DataReaderHandle` compiles,
 *    so every operation checks the handle it receives and rejects 0 —
 *    and anything that is not a non-negative 32-bit integer — with a
 *    `RangeError`.
 * 2. **Liveness** (§C.5.3). A handle that has been deleted throws when
 *    it is used again, naming the handle. A double delete throws for
 *    the same reason: silently succeeding would let a caller believe it
 *    had released something twice as often as it had.
 * 3. **The cascade** (§C.5.3). Deleting a participant releases
 *    everything created from it — topics, publishers, subscribers, and
 *    the writers and readers under those — and a call naming any of
 *    them afterwards throws rather than reaching a module whose
 *    resource is gone.
 * 4. **Borrowed buffers** (§C.3.2). The octets handed to `writeSample`
 *    are copied before they cross, so a module that retained a
 *    reference past the call would be holding a buffer the caller
 *    cannot observe; and samples coming back are copied again, so what
 *    the caller holds is owned by JavaScript and not a view into a
 *    WebAssembly memory that may be resized under it.
 * 5. **Deferred delivery** (§C.2.4). A `DataAvailable` listener runs on
 *    the host event loop, never inside the synchronous `writeSample`
 *    that produced the sample. The wrapper queues a microtask, so the
 *    callback runs after the caller's current turn finishes.
 */
export function bindWasmBackend(backend: WasmBackend): DdsBindings {
  const live = new Map<string, Resource>();

  /** Record a handle the module just produced. */
  function born(kind: HandleKind, value: number, parents: ReadonlyArray<string>): number {
    if (!Number.isInteger(value) || value <= 0 || value > 0xffffffff) {
      throw new RangeError(
        `${kind}: the backend answered with ${value}, which is not a handle`,
      );
    }
    live.set(slot(kind, value), { kind, value, parents });
    return value;
  }

  /** Check a handle a caller passed in, and answer with its number. */
  function check(kind: HandleKind, handle: number): number {
    if (!Number.isInteger(handle) || handle < 0 || handle > 0xffffffff) {
      throw new RangeError(`${kind} handle ${String(handle)} is not a handle`);
    }
    if (handle === 0) {
      throw new RangeError(`${kind} handle 0 is the invalid-handle sentinel`);
    }
    if (!live.has(slot(kind, handle))) {
      throw new RangeError(`${kind} handle ${handle} is not live`);
    }
    return handle;
  }

  /** Forget a handle and everything created from it. */
  function bury(kind: HandleKind, handle: number): void {
    const key = slot(kind, handle);
    live.delete(key);
    for (const [child, resource] of [...live]) {
      if (resource.parents.includes(key)) {
        bury(resource.kind, resource.value);
      }
    }
  }

  /**
   * A copy of the caller's octets.
   *
   * `slice` and not the array itself: §C.3.2 makes the buffer borrowed
   * for the duration of the call, and the copy is what makes that true
   * of a module that does not read the clause.
   */
  function borrowed(octets: Uint8Array): Uint8Array {
    return octets.slice();
  }

  return {
    createParticipant(domainId: number): ParticipantHandle {
      if (!Number.isInteger(domainId) || domainId < 0) {
        throw new RangeError(`domain ${String(domainId)} is not a domain id`);
      }
      return born("participant", backend.createParticipant(domainId), []) as ParticipantHandle;
    },

    deleteParticipant(p: ParticipantHandle): void {
      const handle = check("participant", p);
      backend.deleteParticipant(handle);
      bury("participant", handle);
    },

    createTopic(p: ParticipantHandle, name: string, typeName: string): TopicHandle {
      const participant = check("participant", p);
      return born("topic", backend.createTopic(participant, name, typeName), [
        slot("participant", participant),
      ]) as TopicHandle;
    },

    deleteTopic(t: TopicHandle): void {
      const handle = check("topic", t);
      backend.deleteTopic(handle);
      bury("topic", handle);
    },

    createPublisher(p: ParticipantHandle): PublisherHandle {
      const participant = check("participant", p);
      return born("publisher", backend.createPublisher(participant), [
        slot("participant", participant),
      ]) as PublisherHandle;
    },

    createSubscriber(p: ParticipantHandle): SubscriberHandle {
      const participant = check("participant", p);
      return born("subscriber", backend.createSubscriber(participant), [
        slot("participant", participant),
      ]) as SubscriberHandle;
    },

    deletePublisher(pub: PublisherHandle): void {
      const handle = check("publisher", pub);
      backend.deletePublisher(handle);
      bury("publisher", handle);
    },

    deleteSubscriber(sub: SubscriberHandle): void {
      const handle = check("subscriber", sub);
      backend.deleteSubscriber(handle);
      bury("subscriber", handle);
    },

    createDataWriter(pub: PublisherHandle, topic: TopicHandle): DataWriterHandle {
      const publisher = check("publisher", pub);
      const t = check("topic", topic);
      return born("writer", backend.createDataWriter(publisher, t), [
        slot("publisher", publisher),
        slot("topic", t),
      ]) as DataWriterHandle;
    },

    createDataReader(sub: SubscriberHandle, topic: TopicHandle): DataReaderHandle {
      const subscriber = check("subscriber", sub);
      const t = check("topic", topic);
      return born("reader", backend.createDataReader(subscriber, t), [
        slot("subscriber", subscriber),
        slot("topic", t),
      ]) as DataReaderHandle;
    },

    writeSample(w: DataWriterHandle, xcdr2: Uint8Array): void {
      backend.writeSample(check("writer", w), borrowed(xcdr2));
    },

    takeSamples(r: DataReaderHandle, max: number): ReadonlyArray<Sample> {
      if (!Number.isInteger(max) || max < 0) {
        throw new RangeError(`take of ${String(max)} samples is not a count`);
      }
      return backend.takeSamples(check("reader", r), max).map((sample) => ({
        bytes: sample.bytes.slice(),
        info: sample.info,
      }));
    },

    deleteDataWriter(w: DataWriterHandle): void {
      const handle = check("writer", w);
      backend.deleteDataWriter(handle);
      bury("writer", handle);
    },

    deleteDataReader(r: DataReaderHandle): void {
      const handle = check("reader", r);
      backend.deleteDataReader(handle);
      bury("reader", handle);
    },

    setDataAvailableListener(r: DataReaderHandle, cb: DataAvailableCallback | null): void {
      const handle = check("reader", r);
      if (cb === null) {
        backend.setDataAvailableListener(handle, null);
        return;
      }
      backend.setDataAvailableListener(handle, (reader: number) => {
        // The host event loop, not this turn. §C.2.4 forbids delivery
        // inside the synchronous `writeSample` that produced the
        // sample, and a backend that calls straight through would
        // otherwise re-enter the application from the middle of its
        // own write.
        queueMicrotask(() => {
          cb(reader as DataReaderHandle);
        });
      });
    },
  };
}
