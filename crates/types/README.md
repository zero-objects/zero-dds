# `zerodds-types`

[![License: Apache-2.0](https://img.shields.io/badge/License-Apache_2.0-blue.svg)](../../LICENSE)
[![Crates.io](https://img.shields.io/crates/v/zerodds-types.svg)](https://crates.io/crates/zerodds-types)
[![docs.rs](https://img.shields.io/docsrs/zerodds-types)](https://docs.rs/zerodds-types)

OMG XTypes 1.3 Type-System: TypeIdentifier + TypeObject (Minimal/Complete) + Assignability + DynamicType + TypeLookup-Service.

Pure-Rust no_std + alloc, `forbid(unsafe_code)`. Part of [**ZeroDDS**](../../README.md). Safety classification: **SAFE**.

## Spec

- **OMG XTypes 1.3** §7.3 (Type-System: TypeIdentifier / TypeObject / Hashing / Resolution)
- **OMG XTypes 1.3** §7.5 (DynamicType / DynamicData Reflection-API)
- **OMG XTypes 1.3** §7.6 (wire encoding, discovery, and the bridge to DynamicType)
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
| `from_ast` | §7.3.4 | The TypeObject of a type in `zerodds-ast`'s model |
| `framing` | §7.3.4 | The DHEADER framing every TypeObject aggregate takes |

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

## The model and the wire form

`from_ast` builds a `TypeObject` from a node in `zerodds-ast`'s model, in both the minimal and the complete form. Nothing in it decides anything about a declaration: a type's extensibility, a member's id, an enumerator's value and a bit bound are all read from the frontend, which is why `builder::Extensibility` and `builder::TryConstruct` are re-exports of the model's own enums and not copies of them.

It refuses two things loudly rather than guessing. A **recursive type**, because XTypes §7.3.4.9 answers it with a strongly connected component identifier over the whole cycle and that is a different computation from a single type's hash. And a construct §7.3 has no kind for — `fixed`, `valuetype`, a value box, `native`, `any`, `ValueBase`, an object reference.

## Not yet back

`dynamic` — `DynamicType`, `DynamicData` and the reflective codec, XTypes §7.5 and §7.6.3. `docs/rebuild-plan.md` step 3 has where it stands.

## Wiring status

`assignability` and `type_matcher` are public API for end-user code that does its own type-compatibility checks outside the DDS discovery pipeline (e.g. bridge implementations, schema registries).

## Feature flags

| Feature | Default | Purpose |
|---------|---------|-------|
| `std`   | ✅       | std re-exports, implies `alloc` |
| `alloc` | ✅       | mandatory (Vec / String / BTreeMap for TypeObject containers) |

## Tests

`cargo test -p zerodds-types`: 232 unit tests, plus `compliance_typeobject`, `fuzz_smoke`, `proptest_assignability`, `type_lookup_service` and `evolution_assignability`.

`evolution_assignability` is the one that measures rather than asserts: it asks §7.2.4 about the fifteen `tests/interop-corpus` cases that carry two IDL files, and compares each verdict with the `## Evolution` rows the probe produced from the same two declarations. Three defects in the ported rules came out of it and are named in `docs/rebuild-plan.md` step 3.

## Stability

`1.0.0-rc.7`, a pre-release. The public API can still change before
1.0.0; the crate's CHANGELOG lists every change.

## Links

- Spec: [OMG XTypes 1.3](https://www.omg.org/spec/DDS-XTypes/1.3/)
- CHANGELOG: [`CHANGELOG.md`](CHANGELOG.md)
- Coverage doc: [`docs/spec-coverage/dds-xtypes-1.3.md`](../../docs/spec-coverage/dds-xtypes-1.3.md)
