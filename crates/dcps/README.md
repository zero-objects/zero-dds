# `zerodds-dcps`

[![License: Apache-2.0](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](https://www.apache.org/licenses/LICENSE-2.0)
[![docs.rs](https://docs.rs/zerodds-dcps/badge.svg)](https://docs.rs/zerodds-dcps)

DCPS Public API (OMG DDS 1.4 §2.2.2) fuer den
[ZeroDDS](https://zerodds.org)-Stack. Live-Runtime mit SPDP/SEDP/WLP,
Built-in-Topics, TypeLookup-Service, Durability-Backend, Conditions
und WaitSet. Safety classification: **STANDARD**.

## Spec-Mapping

| Spec | Abschnitt |
|------|-----------|
| OMG DDS 1.4 | §2.2 (DCPS-Module) |
| DDSI-RTPS 2.5 | §8.5 (Discovery), §8.4.13 (WLP), §9.6.4.8 (Inline-QoS) |
| OMG XTypes 1.3 | §7.6.3.3 (TypeLookup-Service), §7.6.3.7 (TypeConsistency) |

## Was ist drin

- **Entity-Hierarchie** — `DomainParticipantFactory` als Singleton,
  `DomainParticipant` als Wurzel, darunter `Publisher`/`DataWriter`
  bzw. `Subscriber`/`DataReader`.
- **Typed Writer/Reader** — `DataWriter<T>` / `DataReader<T>` mit
  `DdsType`-Bound. End-User implementieren `DdsType` per Hand oder
  via `zerodds-idl-rust`-Codegen.
- **Topics** — `Topic<T>` plus `ContentFilteredTopic` (SQL92-Filter)
  und `MultiTopic` (Hash-Join).
- **Built-in-Topics** — `DCPSParticipant`, `DCPSTopic`,
  `DCPSPublication`, `DCPSSubscription` (DDS 1.4 §2.2.5).
- **QoS-Vollabdeckung** — alle 22 Spec-Policies, inkl. Exclusive-
  Ownership-Filter im Sample-Lieferpfad und Time-Based-Filter.
- **Discovery + TypeLookup** — Live-Runtime spawnt SPDP-/SEDP-/WLP-/
  TypeLookup-Endpoints; Cross-Vendor-Interop gegen Cyclone-DDS und
  Fast-DDS verifiziert.
- **Durability** — Transient-In-Memory- und Persistent-On-Disk-
  Backends mit Sequence-orderierter Replay-Liste.
- **Conditions/WaitSet** — `ReadCondition`, `QueryCondition`,
  `GuardCondition`, `StatusCondition` und `WaitSet`.
- **Security-Hook** — opt-in `security`-Feature spannt das
  `SharedSecurityGate` ueber den UDP-Hot-Path (DDS-Security 1.2).

## Schichten-Position

Layer 4 — Core Services. Bauend auf Layer 1
(`zerodds-foundation`, `-cdr`, `-qos`, `-types`, `-time-service`),
Layer 2 (`-rtps`, `-discovery`, `-transport-*`) und Layer 3
(`-idl`, `-idl-rust`, `-xml`).

## Quickstart

```rust
use zerodds_dcps::*;

let factory = DomainParticipantFactory::instance();
// Live-Mode: spawnt Runtime, bindet UDP-Sockets, faehrt SPDP/SEDP.
// (Hier `_offline` fuer Doctest-Reproduzierbarkeit.)
let participant = factory.create_participant_offline(0, DomainParticipantQos::default());
let topic = participant
    .create_topic::<RawBytes>("Chatter", TopicQos::default())
    .expect("create_topic");

let publisher = participant.create_publisher(PublisherQos::default());
let writer = publisher
    .create_datawriter::<RawBytes>(&topic, DataWriterQos::default())
    .expect("create_datawriter");

writer.write(&RawBytes::new(vec![1, 2, 3])).expect("write");
```

## Feature-Flags

| Feature | Default | Zweck |
|---------|---------|-------|
| `std` | ✅ | Aktiviert die Live-Runtime, Threads, UDP-Sockets, Conditions/WaitSet, Durability-Backends. Implies `alloc`. |
| `alloc` | ✅ (via `std`) | Pflichtig: `Vec`/`String`/`BTreeMap`. |
| `safety` | ❌ | Reserviert fuer zukuenftige Safety-Build-Constraints. |
| `security` | ❌ | Aktiviert das `SharedSecurityGate` im UDP-Hot-Path (DDS-Security 1.2). |
| `live-interop` | ❌ | Aktiviert Cyclone-DDS-/Fast-DDS-Live-Tests gegen Lab-Hosts. |
| `flatdata-integration` | ❌ | ADR-0005: opt-in Dual-Stack-Schreib-/Lesepfad ueber `FlatStruct`. |
| `inspect` | ❌ | Embargo: PDE-Reality-Inspector-Tap-Hooks. Wird im Public-Mirror entfernt. |

## Stabilitaet

Alle `pub`-Items sind ab `1.0.0` stabil; Breaking-Changes erfordern
Major-Version-Bump. Doc-hidden `__push_raw`/`__drain_pending`-Hooks
sind interne Test-API und nicht stabil.

## Tests

```bash
cargo test -p zerodds-dcps
```

## Lizenz

Apache-2.0. Siehe [LICENSE](../../LICENSE).

## Siehe auch

- [`docs/architecture/02_architecture.md`](../../docs/architecture/02_architecture.md) — Schichten-Architektur
- [`docs/architecture/04_safety_by_architecture.md`](../../docs/architecture/04_safety_by_architecture.md) — Safety-Klassifikation
- [`docs/spec-coverage/dds-1.4.md`](../../docs/spec-coverage/dds-1.4.md) — Spec-Coverage-Tabelle
