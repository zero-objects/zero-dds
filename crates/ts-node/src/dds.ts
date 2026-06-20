// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// dds.ts — DDS-PSM-Cxx 1.0 conform TS surface over koffi.

import koffi from "koffi";
import * as N from "./native.js";
import { ZeroDdsError } from "./index.js";

/// Topic-traits interface — sample-type coding.
export interface TopicTraits<T> {
  readonly typeName: string;
  encode(value: T): Uint8Array;
  decode(bytes: Uint8Array): T;
}

/// A TypeSupport as emitted by `idlc ts` (the `<Type>TypeSupport` const). The
/// generated support exposes the DDS type name plus XCDR2 encode/decode, so it
/// is structurally a superset of {@link TopicTraits}; `createTypedTopic`
/// accepts either form.
export interface TypeSupport<T> {
  readonly typeName: string;
  encode(sample: T, ...rest: unknown[]): Uint8Array;
  decode(bytes: Uint8Array, ...rest: unknown[]): T;
}

/// Adapts a {@link TypeSupport} (codegen output) to the internal
/// {@link TopicTraits} contract used by the factory layer.
function traitsFromTypeSupport<T>(ts: TypeSupport<T>): TopicTraits<T> {
  return {
    typeName: ts.typeName,
    encode: (v) => ts.encode(v),
    decode: (b) => ts.decode(b),
  };
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
  createBytesTopic(name: string): Topic<Uint8Array> {
    return Topic.create(this, name, ByteSeqTraits);
  }
  /// Creates a `Topic<string>` carrying UTF-8 strings.
  createStringTopic(name: string): Topic<string> {
    return Topic.create(this, name, StringTraits);
  }
  /// Creates a typed topic bound to a codegen `TypeSupport`.
  createTypedTopic<T>(name: string, typeSupport: TypeSupport<T>): Topic<T> {
    return Topic.create(this, name, traitsFromTypeSupport(typeSupport));
  }
  /// Creates a `Publisher` in this participant.
  createPublisher(): Publisher {
    return Publisher.create(this);
  }
  /// Creates a `Subscriber` in this participant.
  createSubscriber(): Subscriber {
    return Subscriber.create(this);
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
  static create<T>(dp: DomainParticipant, name: string, traits: TopicTraits<T>): Topic<T> {
    const t = N.zerodds_dp_create_topic(dp.raw, name, traits.typeName, null);
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

/// Publisher.
export class Publisher {
  constructor(
    private handle: unknown | null,
    private participant: unknown | null,
  ) {}
  static create(dp: DomainParticipant): Publisher {
    const p = N.zerodds_dp_create_publisher(dp.raw, null);
    if (!p) throw new ZeroDdsError(-1, "create_publisher");
    return new Publisher(p, dp.raw);
  }
  get raw(): unknown {
    if (!this.handle) throw new Error("Publisher disposed");
    return this.handle;
  }

  // ---- Fluent writer factories ----

  /// Creates a `DataWriter<Uint8Array>` for a bytes topic.
  createBytesWriter(topic: Topic<Uint8Array>): DataWriter<Uint8Array> {
    return DataWriter.create(this, topic);
  }
  /// Creates a typed `DataWriter<T>` for a typed topic.
  createTypedWriter<T>(topic: Topic<T>): DataWriter<T> {
    return DataWriter.create(this, topic);
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
  static create<T>(pub: Publisher, topic: Topic<T>): DataWriter<T> {
    const dw = N.zerodds_pub_create_datawriter(pub.raw, topic.raw, null);
    if (!dw) throw new ZeroDdsError(-1, "create_datawriter");
    return new DataWriter<T>(dw, pub.raw, topic.traits);
  }
  write(sample: T): void {
    const bytes = this.traits.encode(sample);
    const rc = N.zerodds_dw_write(this.handle, bytes, bytes.length, 0n) as number;
    if (rc !== 0) throw new ZeroDdsError(rc, "DataWriter::write");
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
  ) {}
  static create(dp: DomainParticipant): Subscriber {
    const s = N.zerodds_dp_create_subscriber(dp.raw, null);
    if (!s) throw new ZeroDdsError(-1, "create_subscriber");
    return new Subscriber(s, dp.raw);
  }
  get raw(): unknown {
    if (!this.handle) throw new Error("Subscriber disposed");
    return this.handle;
  }

  // ---- Fluent reader factories ----

  /// Creates a `DataReader<Uint8Array>` for a bytes topic.
  createBytesReader(topic: Topic<Uint8Array>): DataReader<Uint8Array> {
    return DataReader.create(this, topic);
  }
  /// Creates a typed `DataReader<T>` for a typed topic.
  createTypedReader<T>(topic: Topic<T>): DataReader<T> {
    return DataReader.create(this, topic);
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
  static create<T>(sub: Subscriber, topic: Topic<T>): DataReader<T> {
    const dr = N.zerodds_sub_create_datareader(sub.raw, topic.raw, null);
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
    return this.traits.decode(copy);
  }

  /// Drains every currently-available sample into an array.
  private drainAll(): T[] {
    const out: T[] = [];
    for (;;) {
      const s = this.drainOne();
      if (s === null) break;
      out.push(s);
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
