# `zerodds-dcps`

[![License: Apache-2.0](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](https://www.apache.org/licenses/LICENSE-2.0)
[![docs.rs](https://docs.rs/zerodds-dcps/badge.svg)](https://docs.rs/zerodds-dcps)

DCPS public API (OMG DDS 1.4 §2.2.2) for the
[ZeroDDS](https://zerodds.org) stack. Live runtime with SPDP/SEDP/WLP,
built-in topics, TypeLookup service, durability backend, conditions
and WaitSet. Safety classification: **STANDARD**.

## Spec mapping

| Spec | Section |
|------|-----------|
| OMG DDS 1.4 | §2.2 (DCPS module) |
| DDSI-RTPS 2.5 | §8.5 (discovery), §8.4.13 (WLP), §9.6.4.8 (inline QoS) |
| OMG XTypes 1.3 | §7.6.3.3 (TypeLookup service), §7.6.3.7 (type consistency) |

## What's inside

- **Entity hierarchy** — `DomainParticipantFactory` as a singleton,
  `DomainParticipant` as the root, below it `Publisher`/`DataWriter`
  and `Subscriber`/`DataReader`.
- **Typed writer/reader** — `DataWriter<T>` / `DataReader<T>` with a
  `DdsType` bound. End users implement `DdsType` by hand or via
  `zerodds-idl-rust` codegen.
- **Topics** — `Topic<T>` plus `ContentFilteredTopic` (SQL92 filter)
  and `MultiTopic` (hash join).
- **Built-in topics** — `DCPSParticipant`, `DCPSTopic`,
  `DCPSPublication`, `DCPSSubscription` (DDS 1.4 §2.2.5).
- **Full QoS coverage** — all 22 spec policies, including the
  exclusive-ownership filter in the sample delivery path and the
  time-based filter.
- **Discovery + TypeLookup** — the live runtime spawns SPDP/SEDP/WLP/
  TypeLookup endpoints; cross-vendor interop verified against Cyclone
  DDS and Fast-DDS.
- **Durability** — transient in-memory and persistent on-disk backends
  with a sequence-ordered replay list.
- **Conditions/WaitSet** — `ReadCondition`, `QueryCondition`,
  `GuardCondition`, `StatusCondition` and `WaitSet`.
- **Security hook** — the opt-in `security` feature stretches the
  `SharedSecurityGate` over the UDP hot path (DDS-Security 1.2).

## Layer position

Layer 4 — core services. Built on layer 1
(`zerodds-foundation`, `-cdr`, `-qos`, `-types`, `-time-service`),
layer 2 (`-rtps`, `-discovery`, `-transport-*`) and layer 3
(`-idl`, `-idl-rust`, `-xml`).

## Quickstart

```rust
use zerodds_dcps::*;

let factory = DomainParticipantFactory::instance();
// Live mode: spawns the runtime, binds UDP sockets, runs SPDP/SEDP.
// (Here `_offline` for doctest reproducibility.)
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

## Feature flags

| Feature | Default | Purpose |
|---------|---------|-------|
| `std` | ✅ | Enables the live runtime, threads, UDP sockets, conditions/WaitSet, durability backends. Implies `alloc`. |
| `alloc` | ✅ (via `std`) | Mandatory: `Vec`/`String`/`BTreeMap`. |
| `safety` | ❌ | Reserved for future safety build constraints. |
| `security` | ❌ | Enables the `SharedSecurityGate` in the UDP hot path (DDS-Security 1.2). |
| `live-interop` | ❌ | Enables Cyclone DDS / Fast-DDS live tests against lab hosts. |
| `flatdata-integration` | ❌ | ADR-0005: opt-in dual-stack write/read path via `FlatStruct`. |
| `inspect` | ❌ | Embargoed: PDE Reality-Inspector tap hooks. Removed in the public mirror. |

## Stability

All `pub` items are stable from `1.0.0` onward; breaking changes
require a major-version bump. The doc-hidden `__push_raw`/`__drain_pending`
hooks are internal test API and not stable.

## Tests

```bash
cargo test -p zerodds-dcps
```

## License

Apache-2.0. See [LICENSE](../../LICENSE).

## See also

- [`docs/architecture/02_architecture.md`](../../docs/architecture/02_architecture.md) — layered architecture
- [`docs/architecture/04_safety_by_architecture.md`](../../docs/architecture/04_safety_by_architecture.md) — safety classification
- [`docs/spec-coverage/dds-1.4.md`](../../docs/spec-coverage/dds-1.4.md) — spec-coverage table
