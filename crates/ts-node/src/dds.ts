// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// dds.ts — DDS-PSM-Cxx 1.0 conform TS surface over koffi.

import * as N from "./native.js";
import { ZeroDdsError } from "./index.js";

/// Topic-traits interface — sample-type coding.
export interface TopicTraits<T> {
  readonly typeName: string;
  encode(value: T): Uint8Array;
  decode(bytes: Uint8Array): T;
}

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
  waitForMatched(min: number, timeoutMs: bigint): void {
    const rc = N.zerodds_dw_wait_for_matched(this.handle, min, timeoutMs) as number;
    if (rc !== 0) throw new ZeroDdsError(rc, "DataWriter::wait_for_matched");
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
  destroy(): void {
    if (this.handle && this.participant) {
      N.zerodds_dp_delete_subscriber(this.participant, this.handle);
      this.handle = null;
    }
  }
}

/// DataReader<T>.
export class DataReader<T> {
  constructor(
    private handle: unknown | null,
    private subscriber: unknown | null,
  ) {}
  static create<T>(sub: Subscriber, topic: Topic<T>): DataReader<T> {
    const dr = N.zerodds_sub_create_datareader(sub.raw, topic.raw, null);
    if (!dr) throw new ZeroDdsError(-1, "create_datareader");
    return new DataReader<T>(dr, sub.raw);
  }
  waitForMatched(min: number, timeoutMs: bigint): void {
    const rc = N.zerodds_dr_wait_for_matched(this.handle, min, timeoutMs) as number;
    if (rc !== 0) throw new ZeroDdsError(rc, "DataReader::wait_for_matched");
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
