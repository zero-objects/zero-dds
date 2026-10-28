# `zerodds-dcps`

[![License: Apache-2.0](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](https://www.apache.org/licenses/LICENSE-2.0)
[![docs.rs](https://docs.rs/zerodds-dcps/badge.svg)](https://docs.rs/zerodds-dcps)

DCPS public API (OMG DDS 1.4 §2.2.2) for the
[ZeroDDS](https://zerodds.org) stack. Live runtime with SPDP/SEDP/WLP,
built-in topics, TypeLookup service, durability backend, conditions
and WaitSet. Safety classification: **STANDARD**.

This documentation follows the RC7 preparation source, not a published-release
or whole-specification conformance claim.

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
  the current [zerodds-build](../zerodds-build/README.md) generation pipeline.
- **Topics** — `Topic<T>` plus `ContentFilteredTopic` (SQL92 filter)
  and `MultiTopic` (hash join).
- **Built-in topics** — `DCPSParticipant`, `DCPSTopic`,
  `DCPSPublication`, `DCPSSubscription` (DDS 1.4 §2.2.5).
- **QoS mechanisms** — including the
  exclusive-ownership filter in the sample delivery path and the
  time-based filter.
- **Discovery + TypeLookup** — the live runtime spawns SPDP/SEDP/WLP/
  TypeLookup endpoints. Interoperability results apply to their measured
  fixtures and revisions, not every type, policy or vendor combination.
- **Durability** — transient in-memory and persistent on-disk backends
  with a sequence-ordered replay list.
- **Conditions/WaitSet** — `ReadCondition`, `QueryCondition`,
  `GuardCondition`, `StatusCondition` and `WaitSet`.
- **Security hook** — the opt-in `security` feature stretches the
  `SharedSecurityGate` over the UDP hot path (DDS-Security 1.2).
- **Real-time thread tuning** — the opt-in `rt` feature applies the
  `threads.*` config keys (`recv_policy`/`tick_policy`,
  `recv_priority`/`tick_priority`, `recv_deadline`/`tick_deadline`,
  `recv_cpus`/`tick_cpus`) through [`zerodds-rt-linux`](../rt-linux/README.md):
  `SCHED_FIFO`/`SCHED_RR`/`SCHED_DEADLINE` and CPU affinity for the recv and
  tick worker threads. Every worker reports its own tuning outcome back to
  `DcpsRuntime::start` before doing any other work; a refused request stops
  the runtime start rather than run silently on the default scheduler.
  Without this feature (or off Linux), a config that asks for any `threads.*`
  policy or CPU list makes `start` fail, naming the missing feature.

## Layer position

Core services compose `zerodds-wire`, `zerodds-wire-codec`, the AST crates,
`zerodds-foundation`, `zerodds-qos`, `zerodds-types`, `zerodds-config`,
`zerodds-rtps`, `zerodds-discovery` and transport crates. The
[manifest](Cargo.toml) defines the actual dependency and optional-feature graph.

## Quickstart

Use the maintained [live publisher/subscriber walkthrough](../../documentation/01-getting-started/first-publisher.md).
It links the executable sources and runs two processes with `RawBytes`.
Offline participant construction is not a network interoperability example.
For generated typed data, follow the [build integration guide](../zerodds-build/README.md).

## Selected feature flags

The default set is `std` and `metrics`. See the manifest for the complete list.

| Feature | Default | Purpose |
|---------|---------|-------|
| `std` | ✅ | Enables the live runtime, threads, UDP sockets, conditions/WaitSet, durability backends. Implies `alloc`. |
| `alloc` | ✅ (via `std`) | Mandatory: `Vec`/`String`/`BTreeMap`. |
| `metrics` | ✅ | Enables the `zerodds-monitor` integration and `std`. |
| `safety` | ❌ | Reserved for future safety build constraints. |
| `security` | ❌ | Enables the `SharedSecurityGate` in the UDP hot path (DDS-Security 1.2). |
| `rt` | ❌ | Applies `threads.*` real-time scheduling + CPU affinity via `zerodds-rt-linux`. On by default for `rmw-zerodds-shim` and `zerodds-c-api`; a passthrough feature on the bridge/service daemons. |
| `live-interop` | ❌ | Enables Cyclone DDS / Fast-DDS live tests against lab hosts. |
| `flatdata-integration` | ❌ | ADR-0005: opt-in dual-stack write/read path via `FlatStruct`. |
| `inspect` | ❌ | Optional Reality-Inspector endpoint integration. |

## Stability

This is a release-candidate API. Consult the selected release's migration notes
before upgrading. Doc-hidden test hooks are not application API.

## Tests

```bash
cargo test -p zerodds-dcps
```

## License

Apache-2.0. See [LICENSE](../../LICENSE).

## See also

- [`docs/architecture/02_architecture.md`](../../docs/architecture/02_architecture.md) — layered architecture
- [`docs/architecture/04_safety_by_architecture.md`](../../docs/architecture/04_safety_by_architecture.md) — safety classification
- [`docs/spec-coverage/dds-dcps-1.4.md`](../../docs/spec-coverage/dds-dcps-1.4.md) — spec-coverage table
