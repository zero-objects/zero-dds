# `zerodds-types`

[![License: Apache-2.0](https://img.shields.io/badge/License-Apache_2.0-blue.svg)](../../LICENSE)
[![Crates.io](https://img.shields.io/crates/v/zerodds-types.svg)](https://crates.io/crates/zerodds-types)
[![docs.rs](https://img.shields.io/docsrs/zerodds-types)](https://docs.rs/zerodds-types)

OMG XTypes 1.3 Type-System: TypeIdentifier + TypeObject (Minimal/Complete) + Assignability + DynamicType + TypeLookup-Service.

Pure-Rust no_std + alloc, `forbid(unsafe_code)`. Part of [**ZeroDDS**](../../README.md). Safety classification: **SAFE**.

## Spec

- **OMG XTypes 1.3** §7.3 (Type-System: TypeIdentifier / TypeObject / Hashing / Resolution)
- **OMG XTypes 1.3** §7.5 (DynamicType / DynamicData Reflection-API)
- **OMG XTypes 1.3** §7.6 (Wire-Encoding + Discovery + Bridge zu DynamicType)
- **OMG XTypes 1.3** §7.2.4 (Assignability + Compatibility)
- **OMG DDS 1.4** §2.2.3 (TypeConsistencyEnforcement QoS-Policy)

## Public-API-Module

| Module | Spec | Purpose |
|-------|------|-------|
| `type_identifier` | §7.3.4.2 | Discriminated TypeIdentifier-Union (primitive/string/plain/hashed) |
| `type_object` | §7.3.4 | Minimal + Complete TypeObject with all 10 kinds (Struct/Union/Enum/Bitmask/Bitset/Alias/Array/Sequence/Map/Annotation) |
| `type_information` | §7.3.5 | TypeInformation + dependency tracking |
| `type_lookup` | §7.3.6 | getTypes / getTypeDependencies IDL service |
| `builder` | (internal) | Programmatic builder for all TypeObject kinds |
| `hash` | §7.3.1.2 | MD5 → 14-byte EquivalenceHash + NameHash |
| `resolve` | §7.3.4.10 | TypeRegistry + alias resolution + DoS caps |
| `assignability` | §7.2.4 | Type-compatibility rules between writer + reader |
| `type_matcher` | §7.6.3.7 | TypeConsistencyEnforcement-aware writer↔reader match |
| `qos` | DDS 1.4 §2.2.3 | TypeConsistencyEnforcement + DataRepresentation |
| `dynamic` | §7.5 + §7.6.3 | DynamicType / DynamicData + TypeObject-Bridge |

## Quick Start

```rust
use zerodds_types::{TypeIdentifier, PrimitiveKind};
use zerodds_types::resolve::TypeRegistry;
use zerodds_types::qos::TypeConsistencyEnforcement;
use zerodds_types::type_matcher::TypeMatcher;

let writer = TypeIdentifier::Primitive(PrimitiveKind::Int32);
let reader = TypeIdentifier::Primitive(PrimitiveKind::Int32);
let tce = TypeConsistencyEnforcement::default();
let m = TypeMatcher::new(&tce);
let registry = TypeRegistry::new();
assert!(m.match_types(&writer, &reader, &registry).is_match());
```

## DynamicType ↔ TypeObject Bridge

`zerodds_types::dynamic::DynamicType::to_type_object` supports the bridge for all 10 TypeObject kinds: **Struct, Union, Enumeration, Bitmask, Bitset, Annotation, Alias** plus the collection kinds **Sequence, Array, Map** (XTypes §7.3.4.4 — `CompleteSequenceType` / `CompleteArrayType` / `CompleteMapType`). Anonymous plain collections are additionally referenced inline via `TypeIdentifier` (PlainCollection).

## Wiring status

`assignability` and `type_matcher` are public API for end-user code that does its own type-compatibility checks outside the DDS discovery pipeline (e.g. bridge implementations, schema registries).

## Feature flags

| Feature | Default | Purpose |
|---------|---------|-------|
| `std`   | ✅       | std re-exports, implies `alloc` |
| `alloc` | ✅       | mandatory (Vec / String / BTreeMap for TypeObject containers) |

## Tests

`cargo test -p zerodds-types`: 354+ tests (285 unit + 9 dynamic + 40 compliance_typeobject + 5 fuzz_smoke + 5 proptest_assignability + 8 type_lookup_service + doctests).

## Stability

All public-API items are semver-stable from `1.0.0-rc.1`.

## Links

- Spec: [OMG XTypes 1.3](https://www.omg.org/spec/DDS-XTypes/1.3/)
- CHANGELOG: [`CHANGELOG.md`](CHANGELOG.md)
- Coverage doc: [`docs/spec-coverage/dds-xtypes-1.3.md`](../../docs/spec-coverage/dds-xtypes-1.3.md)
- RFC: [`docs/rfcs/0004-xtypes-integration.md`](../../docs/rfcs/0004-xtypes-integration.md)
