# Changelog

Format folgt [Keep a Changelog](https://keepachangelog.com/de/1.1.0/), Versionierung folgt [Semantic Versioning](https://semver.org/lang/de/).

## [1.0.0-rc.1]

Initiale Release-Materialisierung der `zerodds-qos`-Crate.

### Spec-Referenzen

- **OMG DDS 1.4** §2.2.3 — alle 22 Standard-QoS-Policies (Durability, Durability-Service, Deadline, Latency-Budget, Liveliness, Reliability, Destination-Order, History, Resource-Limits, Transport-Priority, Lifespan, User-Data, Topic-Data, Group-Data, Ownership, Ownership-Strength, Presentation, Partition, Time-Based-Filter, Reader/Writer-Data-Lifecycle, Entity-Factory).
- **OMG DDS 1.4** §2.2.3 Table "QoS compatibility" — Request/Offered-Compatibility-Matrix.
- **OMG DDS 1.4** §2.2.3.23 / §2.2.2.5.5 — Exclusive-Ownership-Resolver-Logik.
- **OMG DDSI-RTPS 2.5** §9.6.3.2 — Wire-PIDs für ParameterList-Encoding.

### Public-API

**Top-Level:**

- `Duration` — DDS-Duration `(seconds: i32, nanoseconds: u32)` mit Zero/Infinite-Konstruktoren.
- `Pid` — Newtype für DDSI-RTPS §9.6.3.2 PID-Konstanten der QoS-Policy-Slice.
- `CompatibilityResult::{Compatible, Incompatible(reasons)}` + `IncompatibleReason` — Request/Offered-Matrix-Output.
- `check_compatibility(&WriterQos, &ReaderQos) -> CompatibilityResult`.

**22 Standard-Policies** (`policies`-Modul + Top-Level re-export):

- `DurabilityQosPolicy`, `DurabilityServiceQosPolicy`, `DeadlineQosPolicy`, `LatencyBudgetQosPolicy`, `LivelinessQosPolicy`, `ReliabilityQosPolicy`, `DestinationOrderQosPolicy`, `HistoryQosPolicy`, `ResourceLimitsQosPolicy`, `TransportPriorityQosPolicy`, `LifespanQosPolicy`, `UserDataQosPolicy`, `TopicDataQosPolicy`, `GroupDataQosPolicy`, `OwnershipQosPolicy`, `OwnershipStrengthQosPolicy`, `PresentationQosPolicy`, `PartitionQosPolicy`, `TimeBasedFilterQosPolicy`, `ReaderDataLifecycleQosPolicy`, `WriterDataLifecycleQosPolicy`, `EntityFactoryQosPolicy`.

**QoS-Aggregate:**

- `ReaderQos`, `WriterQos` — pro-Reader/Writer Vollständige QoS-Set-Aggregate.

**Kind-Enums:**

- `DurabilityKind` (Volatile / TransientLocal / Transient / Persistent).
- `ReliabilityKind` (BestEffort / Reliable).
- `LivelinessKind` (Automatic / ManualByParticipant / ManualByTopic).
- `OwnershipKind` (Shared / Exclusive).
- `HistoryKind` (KeepLast / KeepAll).
- `DestinationOrderKind` (ByReceptionTimestamp / BySourceTimestamp).
- `PresentationAccessScope` (Instance / Topic / Group).

**Exclusive-Ownership-Resolver** (`exclusive_ownership`-Modul):

- `OwnershipResolver` — State-Holder pro Instanz für Strongest-Writer-Tracking.
- `OwnershipCandidate { guid, strength }` — Kandidat-Entry.
- `resolve_strongest(&[OwnershipCandidate]) -> Option<OwnershipCandidate>` — zustandslose Resolver-Funktion.
- `WriterGuidBytes` — `[u8; 16]` GUID-Alias.

### Implementierung

- `forbid(unsafe_code)` über die ganze Crate.
- `#![cfg_attr(not(feature = "std"), no_std)]` mit mandatory `alloc`.
- 200 Tests grün (199 unit + 1 compliance_qos_pid + 1 doctest).
- Wire-Roundtrip pro Policy via `encode_into` / `decode_from`-Methoden gegen Cyclone-DDS-PL_CDR_LE-Golden-Vectors.

### Feature-Flags

| Feature | Default | Zweck |
|---------|---------|-------|
| `std`   | ✅      | std-Re-Exports, implies `alloc` |
| `alloc` | ✅      | mandatory (Vec/String); Feature bleibt aus Konsistenz mit Workspace-Pattern |
