# `zerodds-qos`

[![License: Apache-2.0](https://img.shields.io/badge/License-Apache_2.0-blue.svg)](../../LICENSE)
[![Crates.io](https://img.shields.io/crates/v/zerodds-qos.svg)](https://crates.io/crates/zerodds-qos)
[![docs.rs](https://img.shields.io/docsrs/zerodds-qos)](https://docs.rs/zerodds-qos)

DDS QoS-Policies (DDS 1.4 §2.2.3) + Request/Offered-Compatibility-Matrix + PL_CDR_LE PID-Wire-Codec (DDSI-RTPS §9.6.3.2).

Pure-Rust no_std + alloc, `forbid(unsafe_code)`. Part of [**ZeroDDS**](../../README.md). Safety classification: **SAFE**.

## Spec

- **DDS 1.4** §2.2.3 — all 22 standard policies + compatibility matrix.
- **DDS 1.4** §2.2.3.23 / §2.2.2.5.5 — exclusive-ownership resolver logic.
- **DDSI-RTPS 2.5** §9.6.3.2 — wire PIDs for ParameterList.

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

**Policies module:** 22 standard policies plus kind enums plus QoS aggregates (`ReaderQos`, `WriterQos`).

**Exclusive-ownership resolver:** `exclusive_ownership::{OwnershipResolver, OwnershipCandidate, resolve_strongest, WriterGuidBytes}`.

Full list in the [rustdoc](https://docs.rs/zerodds-qos).

## Feature flags

| Feature | Default | Purpose |
|---------|---------|-------|
| `std`   | ✅       | std re-exports, implies `alloc` |
| `alloc` | ✅       | mandatory (partition strings, GenericData, …); kept for consistency |

## Stability

All items listed in the public API are semver-stable from `1.0.0-rc.1`.

## Links

- Spec: [OMG DDS 1.4](https://www.omg.org/spec/DDS/1.4/), [DDSI-RTPS 2.5](https://www.omg.org/spec/DDSI-RTPS/2.5/)
- CHANGELOG: [`CHANGELOG.md`](CHANGELOG.md)
