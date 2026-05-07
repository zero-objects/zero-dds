# `zerodds-qos`

[![License: Apache-2.0](https://img.shields.io/badge/License-Apache_2.0-blue.svg)](../../LICENSE)
[![Crates.io](https://img.shields.io/crates/v/zerodds-qos.svg)](https://crates.io/crates/zerodds-qos)
[![docs.rs](https://img.shields.io/docsrs/zerodds-qos)](https://docs.rs/zerodds-qos)

DDS QoS-Policies (DDS 1.4 §2.2.3) + Request/Offered-Compatibility-Matrix + PL_CDR_LE PID-Wire-Codec (DDSI-RTPS §9.6.3.2).

Pure-Rust no_std + alloc, `forbid(unsafe_code)`. Part of [**ZeroDDS**](../../README.md). Safety classification: **SAFE**.

## Spec

- **DDS 1.4** §2.2.3 — alle 22 Standard-Policies + Compatibility-Matrix.
- **DDS 1.4** §2.2.3.23 / §2.2.2.5.5 — Exclusive-Ownership-Resolver-Logik.
- **DDSI-RTPS 2.5** §9.6.3.2 — Wire-PIDs für ParameterList.

## Quick Start

```rust
use zerodds_qos::{
    ReliabilityKind, ReliabilityQosPolicy, ReaderQos, WriterQos,
    check_compatibility, CompatibilityResult,
};

let mut writer_qos = WriterQos::default();
writer_qos.reliability = ReliabilityQosPolicy {
    kind: ReliabilityKind::Reliable,
    ..Default::default()
};
let reader_qos = ReaderQos::default();
assert!(matches!(
    check_compatibility(&writer_qos, &reader_qos),
    CompatibilityResult::Compatible
));
```

## Public API (1.0.0-rc.1)

**Top-Level:** `Duration`, `Pid`, `CompatibilityResult`, `IncompatibleReason`, `check_compatibility`.

**Policies-Modul:** 22 Standard-Policies plus Kind-Enums plus QoS-Aggregate (`ReaderQos`, `WriterQos`).

**Exclusive-Ownership-Resolver:** `exclusive_ownership::{OwnershipResolver, OwnershipCandidate, resolve_strongest, WriterGuidBytes}`.

Volle Liste in der [rustdoc](https://docs.rs/zerodds-qos).

## Feature-Flags

| Feature | Default | Zweck |
|---------|---------|-------|
| `std`   | ✅       | std-Re-Exports, implies `alloc` |
| `alloc` | ✅       | mandatory (Partition-Strings, GenericData, …); kept for consistency |

## Stability

Alle in der Public API gelisteten Items sind ab `1.0.0-rc.1` semver-stabil.

## Links

- Spec: [OMG DDS 1.4](https://www.omg.org/spec/DDS/1.4/), [DDSI-RTPS 2.5](https://www.omg.org/spec/DDSI-RTPS/2.5/)
- CHANGELOG: [`CHANGELOG.md`](CHANGELOG.md)
