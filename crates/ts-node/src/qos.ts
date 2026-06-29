// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// qos.ts — public DDS QoS policy surface (OMG DDS-DCPS 1.4 §2.2.3) over the
// zerodds-c-api QoS structs. Each `*Qos` object is a plain, mutable
// JS record with spec-default policy values; `build*QosBuffer` marshals it into
// a koffi-encoded buffer whose layout is byte-identical to the corresponding
// `zerodds_ZeroDds*Qos` C struct (see native.ts). Pointer-bearing fields
// (PARTITION `const char *const *`, *_data `const uint8_t*`) are pinned in a
// {@link QosScope} that MUST stay alive across the native create call.

import koffi from "koffi";
import * as N from "./native.js";

/// Duration (Spec §2.2.3.5). The INFINITE sentinel is `{sec: 0x7fffffff,
/// nanosec: 0xffffffff}`; the zero duration (default for deadline/lifespan/...)
/// is treated as "infinite/disabled" by the core, matching the spec defaults.
export interface Duration {
  sec: number;
  nanosec: number;
}
export const DURATION_ZERO: Duration = { sec: 0, nanosec: 0 };
export const DURATION_INFINITE: Duration = { sec: 0x7fffffff, nanosec: 0xffffffff };

/// ReliabilityQosPolicy.Kind (Spec §2.2.3.14). 1 = BEST_EFFORT, 2 = RELIABLE.
export enum ReliabilityKind {
  BestEffort = 1,
  Reliable = 2,
}
/// DurabilityQosPolicy.Kind (Spec §2.2.3.4).
export enum DurabilityKind {
  Volatile = 0,
  TransientLocal = 1,
  Transient = 2,
  Persistent = 3,
}
/// HistoryQosPolicy.Kind (Spec §2.2.3.18).
export enum HistoryKind {
  KeepLast = 0,
  KeepAll = 1,
}
/// OwnershipQosPolicy.Kind (Spec §2.2.3.23).
export enum OwnershipKind {
  Shared = 0,
  Exclusive = 1,
}
/// LivelinessQosPolicy.Kind (Spec §2.2.3.11).
export enum LivelinessKind {
  Automatic = 0,
  ManualByParticipant = 1,
  ManualByTopic = 2,
}
/// DestinationOrderQosPolicy.Kind (Spec §2.2.3.17).
export enum DestinationOrderKind {
  ByReception = 0,
  BySource = 1,
}

// ---- Policy records (with spec defaults) ----

export interface ReliabilityPolicy {
  kind: ReliabilityKind;
  maxBlockingTime: Duration;
}
export interface DurabilityPolicy {
  kind: DurabilityKind;
}
export interface HistoryPolicy {
  kind: HistoryKind;
  depth: number;
}
export interface DeadlinePolicy {
  period: Duration;
}
export interface LivelinessPolicy {
  kind: LivelinessKind;
  leaseDuration: Duration;
}
export interface OwnershipPolicy {
  kind: OwnershipKind;
}
export interface OwnershipStrengthPolicy {
  value: number;
}
export interface PartitionPolicy {
  names: string[];
}

/// DataWriterQos (Spec §2.2.3.x, RxO offered side). Only the policies the
/// binding can act on are surfaced as named fields; the rest are filled with
/// spec defaults by the marshaller.
export interface DataWriterQos {
  reliability: ReliabilityPolicy;
  durability: DurabilityPolicy;
  history: HistoryPolicy;
  deadline: DeadlinePolicy;
  liveliness: LivelinessPolicy;
  ownership: OwnershipPolicy;
  ownershipStrength: OwnershipStrengthPolicy;
}
/// DataReaderQos (Spec §2.2.3.x, RxO requested side).
export interface DataReaderQos {
  reliability: ReliabilityPolicy;
  durability: DurabilityPolicy;
  history: HistoryPolicy;
  deadline: DeadlinePolicy;
  liveliness: LivelinessPolicy;
  ownership: OwnershipPolicy;
}
/// Publisher/Subscriber QoS carry the PARTITION policy (Spec §2.2.3.13).
export interface PublisherQos {
  partition: PartitionPolicy;
}
export type SubscriberQos = PublisherQos;
/// TopicQos carries the topic-level policies (RELIABILITY/DURABILITY/HISTORY/...).
export interface TopicQos {
  reliability: ReliabilityPolicy;
  durability: DurabilityPolicy;
  history: HistoryPolicy;
  deadline: DeadlinePolicy;
  liveliness: LivelinessPolicy;
  ownership: OwnershipPolicy;
}

// ---- Spec-default factories ----

/// RELIABLE is the writer/reader default in this binding (matches the implicit
/// reliable-loopback the C-FFI gives for a NULL qos, and the rust/c# bindings).
function defaultReliability(): ReliabilityPolicy {
  return { kind: ReliabilityKind.Reliable, maxBlockingTime: { sec: 0, nanosec: 100_000_000 } };
}
function defaultHistory(): HistoryPolicy {
  return { kind: HistoryKind.KeepLast, depth: 1 };
}

/// Returns a fresh {@link DataWriterQos} with all-default policies.
export function defaultDataWriterQos(): DataWriterQos {
  return {
    reliability: defaultReliability(),
    durability: { kind: DurabilityKind.Volatile },
    history: defaultHistory(),
    deadline: { period: { ...DURATION_INFINITE } },
    liveliness: { kind: LivelinessKind.Automatic, leaseDuration: { ...DURATION_INFINITE } },
    ownership: { kind: OwnershipKind.Shared },
    ownershipStrength: { value: 0 },
  };
}
/// Returns a fresh {@link DataReaderQos} with all-default policies.
export function defaultDataReaderQos(): DataReaderQos {
  return {
    reliability: defaultReliability(),
    durability: { kind: DurabilityKind.Volatile },
    history: defaultHistory(),
    deadline: { period: { ...DURATION_INFINITE } },
    liveliness: { kind: LivelinessKind.Automatic, leaseDuration: { ...DURATION_INFINITE } },
    ownership: { kind: OwnershipKind.Shared },
  };
}
/// Returns a fresh {@link PublisherQos} (empty partition = the default
/// partition, Spec §2.2.3.13).
export function defaultPublisherQos(): PublisherQos {
  return { partition: { names: [] } };
}
/// Returns a fresh {@link SubscriberQos}.
export function defaultSubscriberQos(): SubscriberQos {
  return { partition: { names: [] } };
}
/// Returns a fresh {@link TopicQos}.
export function defaultTopicQos(): TopicQos {
  return {
    reliability: defaultReliability(),
    durability: { kind: DurabilityKind.Volatile },
    history: defaultHistory(),
    deadline: { period: { ...DURATION_INFINITE } },
    liveliness: { kind: LivelinessKind.Automatic, leaseDuration: { ...DURATION_INFINITE } },
    ownership: { kind: OwnershipKind.Shared },
  };
}

// ---- koffi marshalling ----

/// Owns the off-heap allocations a QoS buffer points into (the PARTITION
/// `char**` array + its strings). koffi `alloc`/Buffer values are GC-rooted by
/// keeping them referenced here; the buffer + scope MUST outlive the native
/// create call.
export class QosScope {
  /** Kept alive so koffi-allocated externals are not collected mid-call. */
  readonly roots: unknown[] = [];
  /** Builds a `const char *const *` array; empty list -> NULL ptr. */
  partition(names: string[]): { names: unknown; names_len: number } {
    if (!names || names.length === 0) return { names: null, names_len: 0 };
    const bufs = names.map((s) => {
      const b = Buffer.from(s + "\0", "utf8");
      this.roots.push(b);
      return b;
    });
    const arr = koffi.alloc("char *", names.length);
    koffi.encode(arr, koffi.array("char *", names.length), bufs);
    this.roots.push(arr);
    return { names: arr, names_len: names.length };
  }
}

const EMPTY_BYTES = { value: null, value_len: 0 };
const DEFAULT_LATENCY = { duration: DURATION_ZERO };
const DEFAULT_PRESENTATION = { access_scope: 0, coherent_access: false, ordered_access: false };
const DEFAULT_RESOURCE_LIMITS = {
  max_samples: -1,
  max_instances: -1,
  max_samples_per_instance: -1,
};
const DEFAULT_DURABILITY_SERVICE = {
  service_cleanup_delay: DURATION_ZERO,
  history_kind: HistoryKind.KeepLast,
  history_depth: 1,
  max_samples: -1,
  max_instances: -1,
  max_samples_per_instance: -1,
};

function rel(p: ReliabilityPolicy) {
  return { kind: p.kind >>> 0, max_blocking_time: p.maxBlockingTime };
}
function live(p: LivelinessPolicy) {
  return { kind: p.kind >>> 0, lease_duration: p.leaseDuration };
}

/// Encodes a {@link DataWriterQos} into a `ZeroDdsDataWriterQos` buffer.
/// `partition` (the owning Publisher's PARTITION names) is copied onto the
/// writer QoS partition slot because the C-FFI matcher reads PARTITION off the
/// endpoint QoS, not the Publisher (DDS 1.4 §2.2.3.13 propagation).
export function buildDataWriterQosBuffer(
  q: DataWriterQos,
  scope: QosScope,
  partition: string[] = [],
): Buffer {
  const buf = Buffer.alloc(koffi.sizeof(N.DataWriterQosStruct));
  koffi.encode(buf, N.DataWriterQosStruct, {
    reliability: rel(q.reliability),
    durability: { kind: q.durability.kind >>> 0 },
    durability_service: DEFAULT_DURABILITY_SERVICE,
    deadline: { period: q.deadline.period },
    latency_budget: DEFAULT_LATENCY,
    liveliness: live(q.liveliness),
    destination_order: { kind: DestinationOrderKind.ByReception },
    lifespan: { duration: DURATION_INFINITE },
    ownership: { kind: q.ownership.kind >>> 0 },
    ownership_strength: { value: q.ownershipStrength.value | 0 },
    partition: scope.partition(partition),
    presentation: DEFAULT_PRESENTATION,
    history: { kind: q.history.kind >>> 0, depth: q.history.depth | 0 },
    resource_limits: DEFAULT_RESOURCE_LIMITS,
    transport_priority: { value: 0 },
    writer_data_lifecycle: { autodispose_unregistered_instances: true },
    user_data: EMPTY_BYTES,
    topic_data: EMPTY_BYTES,
    group_data: EMPTY_BYTES,
  });
  return buf;
}

/// Encodes a {@link DataReaderQos} into a `ZeroDdsDataReaderQos` buffer.
/// `partition` (the owning Subscriber's PARTITION names) is copied onto the
/// reader QoS partition slot (DDS 1.4 §2.2.3.13 propagation; the C-FFI matcher
/// reads PARTITION off the endpoint QoS).
export function buildDataReaderQosBuffer(
  q: DataReaderQos,
  scope: QosScope,
  partition: string[] = [],
): Buffer {
  const buf = Buffer.alloc(koffi.sizeof(N.DataReaderQosStruct));
  koffi.encode(buf, N.DataReaderQosStruct, {
    reliability: rel(q.reliability),
    durability: { kind: q.durability.kind >>> 0 },
    deadline: { period: q.deadline.period },
    latency_budget: DEFAULT_LATENCY,
    liveliness: live(q.liveliness),
    destination_order: { kind: DestinationOrderKind.ByReception },
    ownership: { kind: q.ownership.kind >>> 0 },
    partition: scope.partition(partition),
    presentation: DEFAULT_PRESENTATION,
    history: { kind: q.history.kind >>> 0, depth: q.history.depth | 0 },
    resource_limits: DEFAULT_RESOURCE_LIMITS,
    time_based_filter: { minimum_separation: DURATION_ZERO },
    reader_data_lifecycle: {
      autopurge_nowriter_samples_delay: DURATION_INFINITE,
      autopurge_disposed_samples_delay: DURATION_INFINITE,
    },
    user_data: EMPTY_BYTES,
    topic_data: EMPTY_BYTES,
    group_data: EMPTY_BYTES,
  });
  return buf;
}

/// Encodes a {@link PublisherQos} (carries PARTITION) into a buffer.
export function buildPublisherQosBuffer(q: PublisherQos, scope: QosScope): Buffer {
  const buf = Buffer.alloc(koffi.sizeof(N.PublisherQosStruct));
  koffi.encode(buf, N.PublisherQosStruct, {
    presentation: DEFAULT_PRESENTATION,
    partition: scope.partition(q.partition.names),
    group_data: EMPTY_BYTES,
    entity_factory: { autoenable_created_entities: true },
  });
  return buf;
}
/// Encodes a {@link SubscriberQos} (identical layout to PublisherQos).
export const buildSubscriberQosBuffer = buildPublisherQosBuffer;

/// Encodes a {@link TopicQos} into a `ZeroDdsTopicQos` buffer.
export function buildTopicQosBuffer(q: TopicQos, scope: QosScope): Buffer {
  const buf = Buffer.alloc(koffi.sizeof(N.TopicQosStruct));
  koffi.encode(buf, N.TopicQosStruct, {
    durability: { kind: q.durability.kind >>> 0 },
    durability_service: DEFAULT_DURABILITY_SERVICE,
    deadline: { period: q.deadline.period },
    latency_budget: DEFAULT_LATENCY,
    liveliness: live(q.liveliness),
    reliability: rel(q.reliability),
    destination_order: { kind: DestinationOrderKind.ByReception },
    history: { kind: q.history.kind >>> 0, depth: q.history.depth | 0 },
    resource_limits: DEFAULT_RESOURCE_LIMITS,
    transport_priority: { value: 0 },
    lifespan: { duration: DURATION_INFINITE },
    ownership: { kind: q.ownership.kind >>> 0 },
    topic_data: EMPTY_BYTES,
  });
  return buf;
}
