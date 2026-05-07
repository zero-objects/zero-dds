# Changelog

Format folgt [Keep a Changelog](https://keepachangelog.com/de/1.1.0/), Versionierung folgt [Semantic Versioning](https://semver.org/lang/de/).

## [1.0.0-rc.1]

Initiale Release-Materialisierung der `zerodds-types`-Crate.

### Spec-Referenzen

- **OMG XTypes 1.3** §7.3 — TypeIdentifier-Union, TypeObject (Minimal + Complete), TypeInformation, TypeLookup-Service, Hashing.
- **OMG XTypes 1.3** §7.5 — DynamicType / DynamicData Reflection-API.
- **OMG XTypes 1.3** §7.6 — Wire-Encoding + Discovery-Integration + Bridge.
- **OMG XTypes 1.3** §7.2.4 — Assignability + Type-Compatibility-Regeln.
- **OMG XTypes 1.3** §7.3.1.2 — EquivalenceHash (MD5 → 14 byte).
- **OMG XTypes 1.3** §7.6.8 — KeyHash mit Endpoint-relativer Berechnung.
- **OMG DDS 1.4** §2.2.3 — TypeConsistencyEnforcement QoS-Policy.

### Public-API

**TypeIdentifier** (`type_identifier`):

- `TypeIdentifier::{None, Primitive, String8Small, String8Large, String16Small, String16Large, PlainSequence{Small,Large}, PlainArray{Small,Large}, PlainMap{Small,Large}, StronglyConnectedComponentRef, EquivalenceHash}`.
- `EquivalenceHash` ([u8; 14]), `EquivalenceKind`, `PrimitiveKind`, `PlainCollectionHeader`, `StronglyConnectedComponentId`.

**TypeObject** (`type_object`):

- `TypeObject::{Minimal, Complete}`.
- `MinimalTypeObject` + `CompleteTypeObject` mit 10 Variants: `Alias`, `Annotation`, `Struct`, `Union`, `Bitset`, `Sequence`, `Array`, `Map`, `Enumerated`, `Bitmask`.
- Pro Kind: `CompleteX` / `MinimalX` Strukturen mit Header + Members + Flags.

**TypeInformation** (`type_information`):

- `TypeInformation`, `TypeIdentifierWithDependencies`, `TypeIdentifierWithSize`.

**TypeLookup** (`type_lookup`): `getTypes` / `getTypeDependencies` IDL-Service-Wire-Format.

**Builder** (`builder`): programmatischer Builder für alle Kinds (Struct/Union/Enum/Bitmask/Alias/Bitset/Annotation + Collections).

**Hashing** (`hash`):

- `compute_hash`, `compute_minimal_hash`, `compute_complete_hash`.
- `to_hashed_type_identifier`.

**Resolve** (`resolve`): `TypeRegistry` + Alias-Resolution + DoS-Caps (`DEFAULT_MAX_RESOLVE_DEPTH`).

**Assignability** (`assignability`):

- `is_assignable(writer, reader, registry, config) -> Assignable`.
- `flatten_inheritance` für Struct/Union mit Base-Type.
- `AssignabilityConfig`, `Assignable`, `InheritanceError`.

**TypeMatcher** (`type_matcher`):

- `TypeMatcher::new(&TypeConsistencyEnforcement)`.
- `match_types(writer_ti, reader_ti, registry) -> TypeMatchResult`.
- `TypeMatchResult::{Matches, Mismatch{reason}}`.

**QoS** (`qos`): `TypeConsistencyEnforcement` (alle Felder + Default), `TypeConsistencyKind`, `DataRepresentation`.

**Dynamic** (`dynamic`):

- `DynamicType` / `DynamicTypeMember` / `DynamicData` Reflection-API (XTypes §7.5).
- `DynamicTypeBuilderFactory` + `DynamicTypeBuilder` für programmatische Type-Konstruktion.
- `TypeKind`, `TypeDescriptor`, `MemberDescriptor`, `ExtensibilityKind`, `TryConstructKind`.
- `DynamicError` mit `Unsupported` / `Inconsistent` / `IllegalOperation` / `BuilderConflict` / `PreconditionNotMet`.
- **Bridge** (`dynamic::bridge`): `DynamicType::to_type_object` für **Struct, Union, Enumeration, Bitmask, Alias** (Spec §7.6.3). Collection-Kinds (Array/Sequence/Map) sind TypeIdentifier-exklusiv; Bitset/Annotation benötigen MemberDescriptor-Erweiterungen.

### Implementierung

- `forbid(unsafe_code)`.
- `#![cfg_attr(not(feature = "std"), no_std)]` mit mandatory `alloc`.
- 350+ Tests grün (281 unit + 9 dynamic + 40 compliance_typeobject + 5 fuzz_smoke + 5 proptest_assignability + 8 type_lookup_service).
- proptest-Suite für Assignability-Regeln.
- compliance_typeobject Golden-Vectors für TypeObject-Wire-Encoding.
- DynamicType-Bridge mit 5 implementierten Kinds (Struct + Union + Enum + Bitmask + Alias) plus 5 explizit klassifizierten (Collection-3 + Bitset + Annotation).

### Feature-Flags

| Feature | Default | Zweck |
|---------|---------|-------|
| `std`   | ✅      | std-Re-Exports, implies `alloc` |
| `alloc` | ✅      | mandatory (Vec/String/BTreeMap) |
