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

| Modul | Spec | Zweck |
|-------|------|-------|
| `type_identifier` | §7.3.4.2 | Discriminated TypeIdentifier-Union (primitive/string/plain/hashed) |
| `type_object` | §7.3.4 | Minimal + Complete TypeObject mit allen 10 Kinds (Struct/Union/Enum/Bitmask/Bitset/Alias/Array/Sequence/Map/Annotation) |
| `type_information` | §7.3.5 | TypeInformation + Dependencies-Tracking |
| `type_lookup` | §7.3.6 | getTypes / getTypeDependencies IDL-Service |
| `builder` | (intern) | Programmatischer Builder für alle TypeObject-Kinds |
| `hash` | §7.3.1.2 | MD5 → 14-byte EquivalenceHash + NameHash |
| `resolve` | §7.3.4.10 | TypeRegistry + Alias-Resolution + DoS-Caps |
| `assignability` | §7.2.4 | Type-Compatibility-Regeln zwischen Writer + Reader |
| `type_matcher` | §7.6.3.7 | TypeConsistencyEnforcement-aware Writer↔Reader-Match |
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

`zerodds_types::dynamic::DynamicType::to_type_object` unterstützt Bridge für: **Struct, Union, Enumeration, Bitmask, Alias**. Collection-Kinds (Array/Sequence/Map) werden via TypeIdentifier exklusiv repräsentiert (XTypes §7.3.4 — keine `CompleteTypeObject`-Variante). Bitset und Annotation benötigen MemberDescriptor-Erweiterungen.

## Wiring-Status

`assignability` und `type_matcher` sind Public-API für End-User-Code, der eigene Type-Compatibility-Checks außerhalb der DDS-Discovery-Pipeline durchführt (z.B. Bridge-Implementations, Schema-Registries). Die ZeroDDS-Discovery-Pipeline matched aktuell per `type_name` (DDS 1.4 §2.2.3 Default-Path); volle XTypes-1.3-§7.6 TypeIdentifier-aware Discovery ist eine eigene cross-layer Architektur-Epic (TypeIdentifier-Konstanten in Codegen + SEDP-Propagation + TypeRegistry-shared-state).

## Feature-Flags

| Feature | Default | Zweck |
|---------|---------|-------|
| `std`   | ✅       | std-Re-Exports, implies `alloc` |
| `alloc` | ✅       | mandatory (Vec / String / BTreeMap für TypeObject-Container) |

## Tests

`cargo test -p zerodds-types`: 354+ Tests (285 unit + 9 dynamic + 40 compliance_typeobject + 5 fuzz_smoke + 5 proptest_assignability + 8 type_lookup_service + doctests).

## Stability

Alle Public-API-Items sind ab `1.0.0-rc.1` semver-stabil.

## Links

- Spec: [OMG XTypes 1.3](https://www.omg.org/spec/DDS-XTypes/1.3/)
- CHANGELOG: [`CHANGELOG.md`](CHANGELOG.md)
- Coverage-Doc: [`docs/spec-coverage/dds-xtypes-1.3.md`](../../docs/spec-coverage/dds-xtypes-1.3.md)
- RFC: [`docs/rfcs/0004-xtypes-integration.md`](../../docs/rfcs/0004-xtypes-integration.md)
