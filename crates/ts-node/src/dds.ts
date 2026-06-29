// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// dds.ts — DDS-PSM-Cxx 1.0 conform TS surface over koffi.

import koffi from "koffi";
import * as N from "./native.js";
import { ZeroDdsError } from "./index.js";
import {
  type DataWriterQos, type DataReaderQos, type PublisherQos,
  type SubscriberQos, type TopicQos,
  QosScope,
  buildDataWriterQosBuffer, buildDataReaderQosBuffer,
  buildPublisherQosBuffer, buildSubscriberQosBuffer, buildTopicQosBuffer,
  defaultDataWriterQos as defaultWriterQos,
  defaultDataReaderQos as defaultReaderQos,
} from "./qos.js";

/// Topic-traits interface — sample-type coding. `keyHash` (optional) supplies
/// the 16-byte instance key used by the keyed-lifecycle ops (dispose /
/// (un)register); `isKeyed` marks whether the type carries a key at all.
export interface TopicTraits<T> {
  readonly typeName: string;
  encode(value: T): Uint8Array;
  /// `endian` carries the wire byte order from the encapsulation header
  /// ("le" default, "be" for a big-endian peer) and `representation` the XCDR
  /// version (1 = XCDR2, 0 = XCDR1 / classic CDR), so the generated decoder
  /// reads multi-byte fields with the correct order + framing.
  decode(bytes: Uint8Array, endian?: "le" | "be", representation?: number): T;
  readonly isKeyed?: boolean;
  keyHash?(value: T): Uint8Array;
}

/// A TypeSupport as emitted by `idlc ts` (the `<Type>TypeSupport` const). The
/// generated support exposes the DDS type name plus XCDR2 encode/decode (and,
/// for keyed types, `isKeyed` + `keyHash`), so it is structurally a superset of
/// {@link TopicTraits}; `createTypedTopic` accepts either form.
export interface TypeSupport<T> {
  readonly typeName: string;
  encode(sample: T, ...rest: unknown[]): Uint8Array;
  decode(bytes: Uint8Array, ...rest: unknown[]): T;
  readonly isKeyed?: boolean;
  keyHash?(sample: T): Uint8Array;
}

/// Adapts a {@link TypeSupport} (codegen output) to the internal
/// {@link TopicTraits} contract used by the factory layer. Preserves the
/// keyed-lifecycle hooks (`isKeyed` / `keyHash`) when the support provides them.
function traitsFromTypeSupport<T>(ts: TypeSupport<T>): TopicTraits<T> {
  return {
    typeName: ts.typeName,
    encode: (v) => ts.encode(v),
    // The generated decode is `decode(bytes, offset, length, endian,
    // representation)` — pass the full arg list so endian/representation land in
    // the right slots (not in `offset`).
    decode: (b, endian, representation) =>
      ts.decode(b, 0, b.length, endian ?? "le", representation ?? 1),
    isKeyed: ts.isKeyed,
    keyHash: ts.keyHash ? (v: T) => ts.keyHash!(v) : undefined,
  };
}

// Re-export the QoS surface so `@zerodds/node` consumers can construct policies.
export {
  type DataWriterQos, type DataReaderQos, type PublisherQos,
  type SubscriberQos, type TopicQos,
  type ReliabilityPolicy, type DurabilityPolicy, type HistoryPolicy,
  type DeadlinePolicy, type LivelinessPolicy, type OwnershipPolicy,
  type OwnershipStrengthPolicy, type PartitionPolicy, type Duration,
  ReliabilityKind, DurabilityKind, HistoryKind, OwnershipKind,
  LivelinessKind, DestinationOrderKind,
  DURATION_ZERO, DURATION_INFINITE,
  defaultDataWriterQos, defaultDataReaderQos, defaultPublisherQos,
  defaultSubscriberQos, defaultTopicQos,
} from "./qos.js";

/// A taken sample together with its DDS SampleInfo (Spec §2.2.2.5.4). Returned
/// by the lifecycle-aware `takeWithInfo()`; lifecycle markers (dispose /
/// unregister) appear here with `validData === false` and the corresponding
/// `instanceState` so the consumer can observe NOT_ALIVE_DISPOSED /
/// NOT_ALIVE_NO_WRITERS transitions.
export interface SampleWithInfo<T> {
  /** Decoded payload; `null` for a pure lifecycle marker (no valid data). */
  readonly data: T | null;
  /** Instance lifecycle state: 1=ALIVE, 2=NOT_ALIVE_DISPOSED, 4=NOT_ALIVE_NO_WRITERS. */
  readonly instanceState: number;
  /** Per-instance handle (from the wire key_hash). */
  readonly instanceHandle: bigint;
  /** True if `data` is a real sample, false for a lifecycle-only marker. */
  readonly validData: boolean;
}

/// A taken sample. The payload mirrors the topic's decoded form; lifecycle
/// markers (no `valid_data`) are not surfaced by the convenience `take()`.
export type Sample<T> = T;

/// Default polling granularity for the async wait helpers, in milliseconds.
/// The waits are event-shaped (return as soon as the predicate holds) but the
/// synchronous koffi FFI has no native readiness future, so readiness is sampled
/// on this interval. Kept small so a satisfied predicate returns promptly.
const POLL_MS = 10;

const sleep = (ms: number): Promise<void> =>
  new Promise((r) => setTimeout(r, ms));

/// Default traits for raw bytes.
export const ByteSeqTraits: TopicTraits<Uint8Array> = {
  typeName: "DDS::Bytes",
  encode: (v) => v,
  decode: (b) => b,
};

/// Default traits for UTF-8 strings.
export const StringTraits: TopicTraits<string> = {
  typeName: "DDS::String",
  encode: (v) => new TextEncoder().encode(v),
  decode: (b) => new TextDecoder().decode(b),
};

/// DomainParticipantFactory.
///
/// The OMG DDS factory is a process singleton. This binding exposes it two
/// ways: the static `createParticipant()` shortcut (DDS-PSM-Cxx style) and the
/// `instance()` accessor that returns a fluent {@link DomainParticipantFactoryHandle}
/// whose `createParticipant()` is an instance method — the form used by the
/// `@zerodds/node` quickstart.
export class DomainParticipantFactory {
  static getInstance(): unknown {
    return N.zerodds_dpf_get_instance();
  }
  static createParticipant(domainId: number): DomainParticipant {
    const f = DomainParticipantFactory.getInstance();
    const p = N.zerodds_dpf_create_participant(f, domainId, null);
    if (!p) throw new ZeroDdsError(-1, "create_participant");
    return new DomainParticipant(p, f);
  }
  /// Returns the process-wide factory as a fluent instance handle.
  static instance(): DomainParticipantFactoryHandle {
    return new DomainParticipantFactoryHandle(DomainParticipantFactory.getInstance());
  }
}

/// Fluent instance-method facade over the singleton factory.
export class DomainParticipantFactoryHandle {
  constructor(private readonly factory: unknown) {}
  /// Creates a participant on `domainId`.
  createParticipant(domainId: number): DomainParticipant {
    const p = N.zerodds_dpf_create_participant(this.factory, domainId, null);
    if (!p) throw new ZeroDdsError(-1, "create_participant");
    return new DomainParticipant(p, this.factory);
  }
}

/// DomainParticipant.
export class DomainParticipant {
  constructor(
    private handle: unknown | null,
    private factory: unknown | null,
  ) {}
  get raw(): unknown {
    if (!this.handle) throw new Error("DomainParticipant disposed");
    return this.handle;
  }
  domainId(): number {
    return N.zerodds_dp_get_domain_id(this.raw) as number;
  }

  // ---- Fluent entity factories (instance-method DCPS facade) ----

  /// Creates a `Topic<Uint8Array>` carrying raw bytes (no codec).
  createBytesTopic(name: string, qos?: TopicQos): Topic<Uint8Array> {
    return Topic.create(this, name, ByteSeqTraits, qos);
  }
  /// Creates a `Topic<string>` carrying UTF-8 strings.
  createStringTopic(name: string, qos?: TopicQos): Topic<string> {
    return Topic.create(this, name, StringTraits, qos);
  }
  /// Creates a typed topic bound to a codegen `TypeSupport`.
  createTypedTopic<T>(name: string, typeSupport: TypeSupport<T>, qos?: TopicQos): Topic<T> {
    return Topic.create(this, name, traitsFromTypeSupport(typeSupport), qos);
  }
  /// Creates a `ContentFilteredTopic` over `related` (Spec §2.2.2.3.3). The
  /// filter expression is the DDS SQL `WHERE` clause; `parameters` are the
  /// positional `%0..%n` arguments. For the untyped C-FFI filter to evaluate
  /// the payload, supply `schema` (the on-wire field names + CDR kinds in
  /// declaration order); without it the related-topic's traits are used to
  /// derive nothing and the filter cannot read fields.
  createContentFilteredTopic<T>(
    name: string,
    related: Topic<T>,
    filterExpression: string,
    parameters: string[] = [],
    schema?: { name: string; kind: CftFieldKind }[],
  ): ContentFilteredTopic<T> {
    return ContentFilteredTopic.create(this, name, related, filterExpression, parameters, schema);
  }
  /// Creates a `Publisher` in this participant. Pass `qos.partition` to place
  /// the publisher in named PARTITIONs (Spec §2.2.3.13).
  createPublisher(qos?: PublisherQos): Publisher {
    return Publisher.create(this, qos);
  }
  /// Creates a `Subscriber` in this participant. Pass `qos.partition` to place
  /// the subscriber in named PARTITIONs (Spec §2.2.3.13).
  createSubscriber(qos?: SubscriberQos): Subscriber {
    return Subscriber.create(this, qos);
  }

  destroy(): void {
    if (this.handle) {
      N.zerodds_dp_delete_contained_entities(this.handle);
      N.zerodds_dpf_delete_participant(this.factory, this.handle);
      this.handle = null;
    }
  }
}

/// TopicDescription / Topic.
export class Topic<T> {
  constructor(
    private handle: unknown | null,
    private participant: unknown | null,
    public readonly traits: TopicTraits<T>,
  ) {}
  static create<T>(dp: DomainParticipant, name: string, traits: TopicTraits<T>, qos?: TopicQos): Topic<T> {
    let t: unknown;
    if (qos) {
      const scope = new QosScope();
      const buf = buildTopicQosBuffer(qos, scope);
      t = N.zerodds_dp_create_topic_qos(dp.raw, name, traits.typeName, buf);
      void scope; // kept alive until here
    } else {
      t = N.zerodds_dp_create_topic(dp.raw, name, traits.typeName, null);
    }
    if (!t) throw new ZeroDdsError(-1, "create_topic");
    return new Topic<T>(t, dp.raw, traits);
  }
  get raw(): unknown {
    if (!this.handle) throw new Error("Topic disposed");
    return this.handle;
  }
  name(): string {
    const ptr = N.zerodds_topic_get_name(this.raw) as unknown;
    if (!ptr) return "";
    // koffi auto-converts char* to string; freeing handled by cleanup of returned char*.
    const s = (ptr as unknown as { toString(): string }).toString();
    // koffi C-string freed automatically.
    return s;
  }
  destroy(): void {
    if (this.handle && this.participant) {
      N.zerodds_dp_delete_topic(this.participant, this.handle);
      this.handle = null;
    }
  }
}

/// CDR field kind for a {@link ContentFilteredTopic} schema field, matching the
/// untyped C-FFI filter (`zerodds_cft_set_schema`): the value the filter reads
/// at that on-wire position.
export enum CftFieldKind {
  Bool = 0,
  Int32 = 1,
  Int64 = 2,
  Float32 = 3,
  Float64 = 4,
  String = 5,
}

/// ContentFilteredTopic (Spec §2.2.2.3.3). Wraps the related topic with an SQL
/// filter expression; a `DataReader` created on it only receives samples that
/// satisfy the filter. Use {@link Subscriber.createFilteredReader}.
export class ContentFilteredTopic<T> {
  constructor(
    private handle: unknown | null,
    private participant: unknown | null,
    public readonly related: Topic<T>,
  ) {}
  static create<T>(
    dp: DomainParticipant,
    name: string,
    related: Topic<T>,
    filterExpression: string,
    parameters: string[],
    schema?: { name: string; kind: CftFieldKind }[],
  ): ContentFilteredTopic<T> {
    // Marshal the positional parameter list into a `const char *const *`.
    const paramBufs = parameters.map((p) => Buffer.from(p + "\0", "utf8"));
    const paramArr =
      parameters.length > 0 ? koffi.alloc("char *", parameters.length) : null;
    if (paramArr) koffi.encode(paramArr, koffi.array("char *", parameters.length), paramBufs);
    const cft = N.zerodds_dp_create_contentfilteredtopic(
      dp.raw, name, related.raw, filterExpression, paramArr, parameters.length,
    );
    if (!cft) throw new ZeroDdsError(-1, "create_contentfilteredtopic");
    // Install the positional CDR schema so the untyped filter can read fields.
    if (schema && schema.length > 0) {
      const nameBufs = schema.map((f) => Buffer.from(f.name + "\0", "utf8"));
      const nameArr = koffi.alloc("char *", schema.length);
      koffi.encode(nameArr, koffi.array("char *", schema.length), nameBufs);
      const kinds = new Uint32Array(schema.map((f) => f.kind >>> 0));
      const rc = N.zerodds_cft_set_schema(cft, nameArr, kinds, schema.length) as number;
      if (rc !== 0) throw new ZeroDdsError(rc, "cft_set_schema");
    }
    return new ContentFilteredTopic<T>(cft, dp.raw, related);
  }
  get raw(): unknown {
    if (!this.handle) throw new Error("ContentFilteredTopic disposed");
    return this.handle;
  }
  get traits(): TopicTraits<T> {
    return this.related.traits;
  }
  destroy(): void {
    if (this.handle && this.participant) {
      N.zerodds_dp_delete_contentfilteredtopic(this.participant, this.handle);
      this.handle = null;
    }
  }
}

/// Publisher.
export class Publisher {
  constructor(
    private handle: unknown | null,
    private participant: unknown | null,
    /// PARTITION names this publisher was created in (Spec §2.2.3.13). These
    /// are propagated onto each created writer's QoS so the C-FFI matcher
    /// (`partitions_overlap`) gates matching.
    readonly partition: string[] = [],
  ) {}
  static create(dp: DomainParticipant, qos?: PublisherQos): Publisher {
    let p: unknown;
    if (qos) {
      const scope = new QosScope();
      const buf = buildPublisherQosBuffer(qos, scope);
      p = N.zerodds_dp_create_publisher_qos(dp.raw, buf);
      void scope;
    } else {
      p = N.zerodds_dp_create_publisher(dp.raw, null);
    }
    if (!p) throw new ZeroDdsError(-1, "create_publisher");
    return new Publisher(p, dp.raw, qos?.partition.names ?? []);
  }
  get raw(): unknown {
    if (!this.handle) throw new Error("Publisher disposed");
    return this.handle;
  }

  // ---- Fluent writer factories ----

  /// Creates a `DataWriter<Uint8Array>` for a bytes topic.
  createBytesWriter(topic: Topic<Uint8Array>, qos?: DataWriterQos): DataWriter<Uint8Array> {
    return DataWriter.create(this, topic, qos);
  }
  /// Creates a typed `DataWriter<T>` for a typed topic. Pass `qos` to select
  /// RELIABILITY / DURABILITY / HISTORY / OWNERSHIP etc. (Spec §2.2.3).
  createTypedWriter<T>(topic: Topic<T>, qos?: DataWriterQos): DataWriter<T> {
    return DataWriter.create(this, topic, qos);
  }

  destroy(): void {
    if (this.handle && this.participant) {
      N.zerodds_dp_delete_publisher(this.participant, this.handle);
      this.handle = null;
    }
  }
}

/// DataWriter<T>.
export class DataWriter<T> {
  constructor(
    private handle: unknown | null,
    private publisher: unknown | null,
    private traits: TopicTraits<T>,
  ) {}
  static create<T>(pub: Publisher, topic: Topic<T>, qos?: DataWriterQos): DataWriter<T> {
    let dw: unknown;
    // A QoS buffer is built whenever an explicit QoS is given OR the publisher
    // carries a PARTITION (which must be copied onto the writer QoS so the
    // C-FFI matcher gates on it; Spec §2.2.3.13).
    if (qos || pub.partition.length > 0) {
      const effective = qos ?? defaultWriterQos();
      const scope = new QosScope();
      const buf = buildDataWriterQosBuffer(effective, scope, pub.partition);
      dw = N.zerodds_pub_create_datawriter_qos(pub.raw, topic.raw, buf);
      void scope;
    } else {
      dw = N.zerodds_pub_create_datawriter(pub.raw, topic.raw, null);
    }
    if (!dw) throw new ZeroDdsError(-1, "create_datawriter");
    return new DataWriter<T>(dw, pub.raw, topic.traits);
  }
  write(sample: T): void {
    const bytes = this.traits.encode(sample);
    const rc = N.zerodds_dw_write(this.handle, bytes, bytes.length, 0n) as number;
    if (rc !== 0) throw new ZeroDdsError(rc, "DataWriter::write");
  }

  /// Computes the 16-byte instance key hash for `sample` via the topic traits,
  /// or throws if the type is not keyed. Used by the lifecycle ops.
  private keyOf(sample: T): Uint8Array {
    if (!this.traits.keyHash) {
      throw new Error("DataWriter: type is not keyed (no keyHash); dispose/register need a key");
    }
    return this.traits.keyHash(sample);
  }

  /// `register_instance` (Spec §2.2.2.4.2.5). Returns the instance handle the
  /// core assigns to `sample`'s key, for use with {@link unregisterInstance} /
  /// {@link disposeInstance}.
  registerInstance(sample: T): bigint {
    const key = this.keyOf(sample);
    const out: [bigint] = [0n];
    const rc = N.zerodds_dw_register_instance(this.handle, key, key.length, out) as number;
    if (rc !== 0) throw new ZeroDdsError(rc, "DataWriter::register_instance");
    return out[0];
  }
  /// `lookup_instance` (Spec §2.2.2.4.2.10) — handle for `sample`'s key.
  lookupInstance(sample: T): bigint {
    const key = this.keyOf(sample);
    const out: [bigint] = [0n];
    const rc = N.zerodds_dw_lookup_instance(this.handle, key, key.length, out) as number;
    if (rc !== 0) throw new ZeroDdsError(rc, "DataWriter::lookup_instance");
    return out[0];
  }
  /// `unregister_instance` (Spec §2.2.2.4.2.7) — emits the UNREGISTERED
  /// lifecycle; a reader observes the instance go to NOT_ALIVE_NO_WRITERS.
  unregisterInstance(handle: bigint): void {
    const rc = N.zerodds_dw_unregister_instance(this.handle, handle) as number;
    if (rc !== 0) throw new ZeroDdsError(rc, "DataWriter::unregister_instance");
  }
  /// Convenience: looks up the handle for `sample` then unregisters it.
  unregister(sample: T): void {
    this.unregisterInstance(this.lookupInstance(sample));
  }
  /// `dispose` (Spec §2.2.2.4.2.13) — emits the DISPOSED lifecycle for
  /// `sample`'s instance; a reader observes NOT_ALIVE_DISPOSED. `handle` is
  /// informational (the wire path keys on the key hash).
  dispose(sample: T, handle: bigint = 0n): void {
    const key = this.keyOf(sample);
    const rc = N.zerodds_dw_dispose(this.handle, key, handle) as number;
    if (rc !== 0) throw new ZeroDdsError(rc, "DataWriter::dispose");
  }
  /// `assert_liveliness` (Spec §2.2.2.4.2.22) — manually asserts this writer is
  /// alive (for MANUAL_BY_* liveliness).
  assertLiveliness(): void {
    const rc = N.zerodds_dw_assert_liveliness(this.handle) as number;
    if (rc !== 0) throw new ZeroDdsError(rc, "DataWriter::assert_liveliness");
  }
  /// Promise-returning `write`: encodes and publishes off the synchronous call
  /// path so a tight publish loop yields to the event loop between samples.
  async writeAsync(sample: T): Promise<void> {
    await Promise.resolve();
    this.write(sample);
  }
  waitForMatched(min: number, timeoutMs: bigint): void {
    const rc = N.zerodds_dw_wait_for_matched(this.handle, min, timeoutMs) as number;
    if (rc !== 0) throw new ZeroDdsError(rc, "DataWriter::wait_for_matched");
  }
  /// Resolves once at least `min` matched subscriptions exist, or rejects on a
  /// `timeoutMs` deadline. Non-blocking: readiness is polled cooperatively so
  /// the Node event loop is never stalled inside the FFI.
  async waitForMatchedSubscription(min: number, timeoutMs: number): Promise<void> {
    const deadline = Date.now() + timeoutMs;
    for (;;) {
      const rc = N.zerodds_dw_wait_for_matched(this.handle, min, 0n) as number;
      if (rc === 0) return;
      if (Date.now() >= deadline) {
        throw new ZeroDdsError(rc, "DataWriter::wait_for_matched_subscription (timeout)");
      }
      await sleep(POLL_MS);
    }
  }
  destroy(): void {
    if (this.handle && this.publisher) {
      N.zerodds_pub_delete_datawriter(this.publisher, this.handle);
      this.handle = null;
    }
  }
}

/// Subscriber.
export class Subscriber {
  constructor(
    private handle: unknown | null,
    private participant: unknown | null,
    /// PARTITION names this subscriber was created in (Spec §2.2.3.13),
    /// propagated onto each created reader's QoS.
    readonly partition: string[] = [],
  ) {}
  static create(dp: DomainParticipant, qos?: SubscriberQos): Subscriber {
    let s: unknown;
    if (qos) {
      const scope = new QosScope();
      const buf = buildSubscriberQosBuffer(qos, scope);
      s = N.zerodds_dp_create_subscriber_qos(dp.raw, buf);
      void scope;
    } else {
      s = N.zerodds_dp_create_subscriber(dp.raw, null);
    }
    if (!s) throw new ZeroDdsError(-1, "create_subscriber");
    return new Subscriber(s, dp.raw, qos?.partition.names ?? []);
  }
  get raw(): unknown {
    if (!this.handle) throw new Error("Subscriber disposed");
    return this.handle;
  }

  // ---- Fluent reader factories ----

  /// Creates a `DataReader<Uint8Array>` for a bytes topic.
  createBytesReader(topic: Topic<Uint8Array>, qos?: DataReaderQos): DataReader<Uint8Array> {
    return DataReader.create(this, topic, qos);
  }
  /// Creates a typed `DataReader<T>` for a typed topic. Pass `qos` to select
  /// RELIABILITY / DURABILITY / HISTORY / OWNERSHIP etc. (Spec §2.2.3).
  createTypedReader<T>(topic: Topic<T>, qos?: DataReaderQos): DataReader<T> {
    return DataReader.create(this, topic, qos);
  }
  /// Creates a `DataReader<T>` on a {@link ContentFilteredTopic}; only samples
  /// satisfying the filter expression are delivered (Spec §2.2.2.3.3).
  createFilteredReader<T>(cft: ContentFilteredTopic<T>): DataReader<T> {
    const dr = N.zerodds_sub_create_datareader_with_cft(this.raw, cft.raw, null);
    if (!dr) throw new ZeroDdsError(-1, "create_datareader_with_cft");
    return new DataReader<T>(dr, this.raw, cft.traits);
  }

  destroy(): void {
    if (this.handle && this.participant) {
      N.zerodds_dp_delete_subscriber(this.participant, this.handle);
      this.handle = null;
    }
  }
}

/// DataReader<T>.
export class DataReader<T> {
  /// Samples drained by `waitForData()` but not yet returned from `take()`.
  private pending: T[] = [];

  constructor(
    private handle: unknown | null,
    private subscriber: unknown | null,
    private traits: TopicTraits<T> = ByteSeqTraits as unknown as TopicTraits<T>,
  ) {}

  /// Drains all currently-available samples via the BATCH `zerodds_dr_take`
  /// path (Spec §2.2.2.5.3), which — unlike the single-sample
  /// `take_next_sample` — applies the ContentFilteredTopic filter
  /// (§2.2.2.3.3), EXCLUSIVE-ownership arbitration (§2.2.3.23) and resolves the
  /// per-instance InstanceHandle. Returns each sample with its SampleInfo
  /// (lifecycle markers included). The native loan is returned before this
  /// method completes, so the decoded copies are owned by JS.
  private drainBatch(maxSamples = 256): SampleWithInfo<T>[] {
    if (!this.handle) throw new Error("DataReader disposed");
    const out: Record<string, unknown> = {};
    const rc = N.zerodds_dr_take(
      this.handle, out, maxSamples, N.STATE_ANY, N.STATE_ANY, N.STATE_ANY,
    ) as number;
    if (rc === N.ZeroDdsStatus.NoData) return [];
    if (rc !== 0) throw new ZeroDdsError(rc, "DataReader::take(batch)");
    const count = Number(out["count"] ?? 0);
    if (count === 0) {
      // Always return the loan even on an empty take so the token is freed.
      N.zerodds_dr_return_loan(this.handle, out);
      return [];
    }
    const buffersPtr = out["buffers"];
    const lengthsPtr = out["lengths"];
    const infosPtr = out["infos"];
    // Decode the parallel arrays. `buffers` is uint8_t**, `lengths` is size_t*,
    // `infos` is an array of `count` SampleInfo structs.
    const bufPtrs = koffi.decode(buffersPtr, "uint8_t *", count) as unknown[];
    const lengths = koffi.decode(lengthsPtr, "size_t", count) as number[];
    const infos = koffi.decode(infosPtr, N.SampleInfo, count) as Record<string, unknown>[];
    const result: SampleWithInfo<T>[] = [];
    for (let i = 0; i < count; i++) {
      const info = infos[i];
      const len = Number(lengths[i] ?? 0);
      const validData = info["valid_data"] !== false;
      const instanceState = Number(info["instance_state"] ?? N.InstanceState.Alive);
      const instanceHandle = BigInt((info["instance_handle"] as bigint | number | undefined) ?? 0n);
      let data: T | null = null;
      if (validData && len > 0 && bufPtrs[i]) {
        const copy = new Uint8Array(len);
        const view = koffi.decode(bufPtrs[i], "uint8_t", len) as Uint8Array;
        copy.set(view);
        // Dispatch on the wire byte order + XCDR representation from the
        // encapsulation header so a big-endian and/or XCDR1 peer's sample
        // decodes correctly.
        const endian = info["big_endian"] ? "be" : "le";
        const representation = Number(info["representation"] ?? 1);
        data = this.traits.decode(copy, endian, representation);
      }
      result.push({ data, instanceState, instanceHandle, validData });
    }
    // Return the loan (frees the native Vec + buffers) now that the bytes are
    // copied into JS-owned Uint8Arrays.
    N.zerodds_dr_return_loan(this.handle, out);
    return result;
  }
  static create<T>(sub: Subscriber, topic: Topic<T>, qos?: DataReaderQos): DataReader<T> {
    let dr: unknown;
    // Build a QoS buffer when an explicit QoS is given OR the subscriber carries
    // a PARTITION (propagated onto the reader QoS; Spec §2.2.3.13).
    if (qos || sub.partition.length > 0) {
      const effective = qos ?? defaultReaderQos();
      const scope = new QosScope();
      const buf = buildDataReaderQosBuffer(effective, scope, sub.partition);
      dr = N.zerodds_sub_create_datareader_qos(sub.raw, topic.raw, buf);
      void scope;
    } else {
      dr = N.zerodds_sub_create_datareader(sub.raw, topic.raw, null);
    }
    if (!dr) throw new ZeroDdsError(-1, "create_datareader");
    return new DataReader<T>(dr, sub.raw, topic.traits);
  }
  waitForMatched(min: number, timeoutMs: bigint): void {
    const rc = N.zerodds_dr_wait_for_matched(this.handle, min, timeoutMs) as number;
    if (rc !== 0) throw new ZeroDdsError(rc, "DataReader::wait_for_matched");
  }
  /// Resolves once at least `min` matched publications exist, or rejects on a
  /// `timeoutMs` deadline. Non-blocking (cooperative poll, see
  /// {@link DataWriter.waitForMatchedSubscription}).
  async waitForMatchedPublication(min: number, timeoutMs: number): Promise<void> {
    const deadline = Date.now() + timeoutMs;
    for (;;) {
      const rc = N.zerodds_dr_wait_for_matched(this.handle, min, 0n) as number;
      if (rc === 0) return;
      if (Date.now() >= deadline) {
        throw new ZeroDdsError(rc, "DataReader::wait_for_matched_publication (timeout)");
      }
      await sleep(POLL_MS);
    }
  }

  /// Drains a single sample from the reader cache (destructive). Returns the
  /// decoded payload, or `null` if no data is currently available.
  ///
  /// The single-sample loan path (`zerodds_dr_take_next_sample`) is documented
  /// in the C-API as leak-tolerant in this release: the loaned buffer points
  /// into a leaked `LoanMemory` owned by the core, NOT an independent boxed
  /// slice. We therefore copy the bytes out and MUST NOT call
  /// `zerodds_buffer_free` on it (that frees a slice that was never separately
  /// boxed — a mismatched free / heap corruption). `rc == NoData (-7)` means
  /// the cache is currently empty.
  private drainOne(): T | null {
    if (!this.handle) throw new Error("DataReader disposed");
    const outBuf: [unknown] = [null];
    const outLen: [number] = [0];
    const info: Record<string, unknown> = {};
    const rc = N.zerodds_dr_take_next_sample(this.handle, outBuf, outLen, info) as number;
    if (rc === N.ZeroDdsStatus.NoData) return null;
    if (rc !== 0) throw new ZeroDdsError(rc, "DataReader::take_next_sample");
    const buf = outBuf[0];
    const len = outLen[0];
    // Lifecycle marker (no payload): not surfaced by the convenience take().
    if (!buf || len === 0 || info["valid_data"] === false) return null;
    const copy = new Uint8Array(len);
    const view = koffi.decode(buf, "uint8_t", len) as Uint8Array;
    copy.set(view);
    // Dispatch on the encapsulation byte order + XCDR representation.
    const endian = info["big_endian"] ? "be" : "le";
    const representation = Number(info["representation"] ?? 1);
    return this.traits.decode(copy, endian, representation);
  }

  /// Drains every currently-available sample into an array (payloads only;
  /// lifecycle-only markers are dropped). Backed by the batch take path so
  /// CFT + EXCLUSIVE-ownership filtering is applied.
  private drainAll(): T[] {
    const out: T[] = [];
    for (const s of this.drainBatch()) {
      if (s.validData && s.data !== null) out.push(s.data);
    }
    return out;
  }

  /// Takes all currently-available samples. Returns an iterable (array) so a
  /// caller can `for (const s of reader.take())`. Includes any samples buffered
  /// by a preceding `waitForData()`.
  take(): Sample<T>[] {
    const buffered = this.pending;
    this.pending = [];
    return buffered.concat(this.drainAll());
  }

  /// Takes all currently-available samples WITH their SampleInfo, surfacing the
  /// per-instance lifecycle state (ALIVE / NOT_ALIVE_DISPOSED /
  /// NOT_ALIVE_NO_WRITERS) so a consumer can observe dispose/unregister
  /// transitions (Spec §2.2.2.5.4). Unlike {@link take}, lifecycle-only markers
  /// are NOT dropped. Backed by the batch take path (CFT + EXCLUSIVE-ownership
  /// filtering + per-instance handle resolution applied).
  takeWithInfo(): SampleWithInfo<T>[] {
    return this.drainBatch();
  }

  /// `lookup_instance` (Spec §2.2.2.5.x) — handle for `sample`'s key.
  lookupInstance(sample: T): bigint {
    if (!this.traits.keyHash) {
      throw new Error("DataReader: type is not keyed (no keyHash)");
    }
    const key = this.traits.keyHash(sample);
    const out: [bigint] = [0n];
    const rc = N.zerodds_dr_lookup_instance(this.handle, key, key.length, out) as number;
    if (rc !== 0) throw new ZeroDdsError(rc, "DataReader::lookup_instance");
    return out[0];
  }

  /// `get_requested_deadline_missed_status` (Spec §2.2.4.1). Returns the
  /// cumulative + delta missed-deadline counts and the last offending instance.
  getRequestedDeadlineMissedStatus(): {
    totalCount: number; totalCountChange: number; lastInstanceHandle: bigint;
  } {
    const out: Record<string, unknown> = {};
    const rc = N.zerodds_dr_get_requested_deadline_missed_status(this.handle, out) as number;
    if (rc !== 0) throw new ZeroDdsError(rc, "DataReader::get_requested_deadline_missed_status");
    return {
      totalCount: Number(out["total_count"] ?? 0),
      totalCountChange: Number(out["total_count_change"] ?? 0),
      lastInstanceHandle: BigInt((out["last_instance_handle"] as bigint | number | undefined) ?? 0n),
    };
  }
  /// `get_liveliness_changed_status` (Spec §2.2.4.1) — alive/not-alive writer
  /// counts and their deltas since the last read.
  getLivelinessChangedStatus(): {
    aliveCount: number; notAliveCount: number;
    aliveCountChange: number; notAliveCountChange: number;
    lastPublicationHandle: bigint;
  } {
    const out: Record<string, unknown> = {};
    const rc = N.zerodds_dr_get_liveliness_changed_status(this.handle, out) as number;
    if (rc !== 0) throw new ZeroDdsError(rc, "DataReader::get_liveliness_changed_status");
    return {
      aliveCount: Number(out["alive_count"] ?? 0),
      notAliveCount: Number(out["not_alive_count"] ?? 0),
      aliveCountChange: Number(out["alive_count_change"] ?? 0),
      notAliveCountChange: Number(out["not_alive_count_change"] ?? 0),
      lastPublicationHandle: BigInt((out["last_publication_handle"] as bigint | number | undefined) ?? 0n),
    };
  }

  /// Resolves once at least one sample is available, or after `timeoutMs`.
  /// Any samples seen while waiting are buffered for the next `take()` /
  /// `takeAsync()` so no data is lost. Non-blocking cooperative poll.
  async waitForData(timeoutMs: number): Promise<boolean> {
    if (this.pending.length > 0) return true;
    const deadline = Date.now() + timeoutMs;
    for (;;) {
      const batch = this.drainAll();
      if (batch.length > 0) {
        this.pending.push(...batch);
        return true;
      }
      if (Date.now() >= deadline) return false;
      await sleep(POLL_MS);
    }
  }

  /// Promise-returning `take`: yields to the event loop, then drains.
  async takeAsync(): Promise<Sample<T>[]> {
    await Promise.resolve();
    return this.take();
  }

  /// Async iterator over samples. Yields each sample as it arrives; ends when
  /// the reader is disposed. Backed by the same cooperative poll as
  /// `waitForData`, so it never blocks the event loop.
  async *streamSamples(): AsyncIterableIterator<Sample<T>> {
    for (;;) {
      if (!this.handle) return;
      const batch = this.take();
      if (batch.length > 0) {
        yield* batch;
        continue;
      }
      await sleep(POLL_MS);
    }
  }

  destroy(): void {
    if (this.handle && this.subscriber) {
      N.zerodds_sub_delete_datareader(this.subscriber, this.handle);
      this.handle = null;
    }
  }
}

/// GuardCondition.
export class GuardCondition {
  private handle: unknown | null;
  constructor() {
    this.handle = N.zerodds_guardcondition_create();
    if (!this.handle) throw new ZeroDdsError(-1, "GuardCondition::create");
  }
  get raw(): unknown {
    if (!this.handle) throw new Error("GuardCondition disposed");
    return this.handle;
  }
  setTriggerValue(v: boolean): void {
    const rc = N.zerodds_guardcondition_set_trigger_value(this.handle, v) as number;
    if (rc !== 0) throw new ZeroDdsError(rc, "GuardCondition::set_trigger_value");
  }
  getTriggerValue(): boolean {
    return N.zerodds_condition_get_trigger_value(this.handle) as boolean;
  }
  destroy(): void {
    if (this.handle) {
      N.zerodds_guardcondition_destroy(this.handle);
      this.handle = null;
    }
  }
}

/// WaitSet.
export class WaitSet {
  private handle: unknown | null;
  constructor() {
    this.handle = N.zerodds_waitset_create();
    if (!this.handle) throw new ZeroDdsError(-1, "WaitSet::create");
  }
  attach(c: GuardCondition): void {
    const rc = N.zerodds_waitset_attach_condition(this.handle, c.raw) as number;
    if (rc !== 0) throw new ZeroDdsError(rc, "WaitSet::attach");
  }
  destroy(): void {
    if (this.handle) {
      N.zerodds_waitset_destroy(this.handle);
      this.handle = null;
    }
  }
}
