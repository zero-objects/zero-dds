# Changelog

Format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), versioning follows [Semantic Versioning](https://semver.org/).

## [1.0.0-rc.1]

Initial release materialization of the `zerodds-qos` crate.

### Spec references

- **OMG DDS 1.4** §2.2.3 — all 22 standard QoS policies (Durability, Durability-Service, Deadline, Latency-Budget, Liveliness, Reliability, Destination-Order, History, Resource-Limits, Transport-Priority, Lifespan, User-Data, Topic-Data, Group-Data, Ownership, Ownership-Strength, Presentation, Partition, Time-Based-Filter, Reader/Writer-Data-Lifecycle, Entity-Factory).
- **OMG DDS 1.4** §2.2.3 table "QoS compatibility" — request/offered compatibility matrix.
- **OMG DDS 1.4** §2.2.3.23 / §2.2.2.5.5 — exclusive-ownership resolver logic.
- **OMG DDSI-RTPS 2.5** §9.6.3.2 — wire PIDs for ParameterList encoding.

### Public API

**Top-level:**

- `Duration` — DDS duration `(seconds: i32, nanoseconds: u32)` with zero/infinite constructors.
- `Pid` — newtype for DDSI-RTPS §9.6.3.2 PID constants of the QoS policy slice.
- `CompatibilityResult::{Compatible, Incompatible(reasons)}` + `IncompatibleReason` — request/offered matrix output.
- `check_compatibility(&WriterQos, &ReaderQos) -> CompatibilityResult`.

**22 standard policies** (`policies` module + top-level re-export):

- `DurabilityQosPolicy`, `DurabilityServiceQosPolicy`, `DeadlineQosPolicy`, `LatencyBudgetQosPolicy`, `LivelinessQosPolicy`, `ReliabilityQosPolicy`, `DestinationOrderQosPolicy`, `HistoryQosPolicy`, `ResourceLimitsQosPolicy`, `TransportPriorityQosPolicy`, `LifespanQosPolicy`, `UserDataQosPolicy`, `TopicDataQosPolicy`, `GroupDataQosPolicy`, `OwnershipQosPolicy`, `OwnershipStrengthQosPolicy`, `PresentationQosPolicy`, `PartitionQosPolicy`, `TimeBasedFilterQosPolicy`, `ReaderDataLifecycleQosPolicy`, `WriterDataLifecycleQosPolicy`, `EntityFactoryQosPolicy`.

**QoS aggregates:**

- `ReaderQos`, `WriterQos` — per-reader/writer complete QoS-set aggregates.

**Kind enums:**

- `DurabilityKind` (Volatile / TransientLocal / Transient / Persistent).
- `ReliabilityKind` (BestEffort / Reliable).
- `LivelinessKind` (Automatic / ManualByParticipant / ManualByTopic).
- `OwnershipKind` (Shared / Exclusive).
- `HistoryKind` (KeepLast / KeepAll).
- `DestinationOrderKind` (ByReceptionTimestamp / BySourceTimestamp).
- `PresentationAccessScope` (Instance / Topic / Group).

**Exclusive-ownership resolver** (`exclusive_ownership` module):

- `OwnershipResolver` — state holder per instance for strongest-writer tracking.
- `OwnershipCandidate { guid, strength }` — candidate entry.
- `resolve_strongest(&[OwnershipCandidate]) -> Option<OwnershipCandidate>` — stateless resolver function.
- `WriterGuidBytes` — `[u8; 16]` GUID alias.

### Implementation

- `forbid(unsafe_code)` across the whole crate.
- `#![cfg_attr(not(feature = "std"), no_std)]` with mandatory `alloc`.
- 200 tests green (199 unit + 1 compliance_qos_pid + 1 doctest).
- Wire roundtrip per policy via `encode_into` / `decode_from` methods against Cyclone DDS PL_CDR_LE golden vectors.

### Feature flags

| Feature | Default | Purpose |
|---------|---------|-------|
| `std`   | ✅      | std re-exports, implies `alloc` |
| `alloc` | ✅      | mandatory (Vec/String); kept for consistency with the workspace pattern |
