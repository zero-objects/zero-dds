# Changelog

Format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), versioning follows [Semantic Versioning](https://semver.org/).

## [1.0.0-rc.1]

Initial release materialization of the `zerodds-types` crate.

### Spec references

- **OMG XTypes 1.3** §7.3 — TypeIdentifier union, TypeObject (minimal + complete), TypeInformation, TypeLookup service, hashing.
- **OMG XTypes 1.3** §7.5 — DynamicType / DynamicData reflection API.
- **OMG XTypes 1.3** §7.6 — wire encoding + discovery integration + bridge.
- **OMG XTypes 1.3** §7.2.4 — assignability + type-compatibility rules.
- **OMG XTypes 1.3** §7.3.1.2 — EquivalenceHash (MD5 → 14 byte).
- **OMG XTypes 1.3** §7.6.8 — KeyHash with endpoint-relative computation.
- **OMG DDS 1.4** §2.2.3 — TypeConsistencyEnforcement QoS policy.

### Public-API

**TypeIdentifier** (`type_identifier`):

- `TypeIdentifier::{None, Primitive, String8Small, String8Large, String16Small, String16Large, PlainSequence{Small,Large}, PlainArray{Small,Large}, PlainMap{Small,Large}, StronglyConnectedComponentRef, EquivalenceHash}`.
- `EquivalenceHash` ([u8; 14]), `EquivalenceKind`, `PrimitiveKind`, `PlainCollectionHeader`, `StronglyConnectedComponentId`.

**TypeObject** (`type_object`):

- `TypeObject::{Minimal, Complete}`.
- `MinimalTypeObject` + `CompleteTypeObject` with 10 variants: `Alias`, `Annotation`, `Struct`, `Union`, `Bitset`, `Sequence`, `Array`, `Map`, `Enumerated`, `Bitmask`.
- Per kind: `CompleteX` / `MinimalX` structures with header + members + flags.

**TypeInformation** (`type_information`):

- `TypeInformation`, `TypeIdentifierWithDependencies`, `TypeIdentifierWithSize`.

**TypeLookup** (`type_lookup`): `getTypes` / `getTypeDependencies` IDL service wire format.

**Builder** (`builder`): programmatic builder for all kinds (struct/union/enum/bitmask/alias/bitset/annotation + collections).

**Hashing** (`hash`):

- `compute_hash`, `compute_minimal_hash`, `compute_complete_hash`.
- `to_hashed_type_identifier`.

**Resolve** (`resolve`): `TypeRegistry` + alias resolution + DoS caps (`DEFAULT_MAX_RESOLVE_DEPTH`).

**Assignability** (`assignability`):

- `is_assignable(writer, reader, registry, config) -> Assignable`.
- `flatten_inheritance` for struct/union with a base type.
- `AssignabilityConfig`, `Assignable`, `InheritanceError`.

**TypeMatcher** (`type_matcher`):

- `TypeMatcher::new(&TypeConsistencyEnforcement)`.
- `match_types(writer_ti, reader_ti, registry) -> TypeMatchResult`.
- `TypeMatchResult::{Matches, Mismatch{reason}}`.

**QoS** (`qos`): `TypeConsistencyEnforcement` (all fields + default), `TypeConsistencyKind`, `DataRepresentation`.

**Dynamic** (`dynamic`):

- `DynamicType` / `DynamicTypeMember` / `DynamicData` reflection API (XTypes §7.5).
- `DynamicTypeBuilderFactory` + `DynamicTypeBuilder` for programmatic type construction.
- `TypeKind`, `TypeDescriptor`, `MemberDescriptor`, `ExtensibilityKind`, `TryConstructKind`.
- `DynamicError` with `Unsupported` / `Inconsistent` / `IllegalOperation` / `BuilderConflict` / `PreconditionNotMet`.
- **Bridge** (`dynamic::bridge`): `DynamicType::to_type_object` for **Struct, Union, Enumeration, Bitmask, Alias** (spec §7.6.3). Collection kinds (array/sequence/map) are TypeIdentifier-exclusive; bitset/annotation need MemberDescriptor extensions.

### Implementation

- `forbid(unsafe_code)`.
- `#![cfg_attr(not(feature = "std"), no_std)]` with mandatory `alloc`.
- 350+ tests green (281 unit + 9 dynamic + 40 compliance_typeobject + 5 fuzz_smoke + 5 proptest_assignability + 8 type_lookup_service).
- proptest suite for assignability rules.
- compliance_typeobject golden vectors for TypeObject wire encoding.
- DynamicType bridge with 5 implemented kinds (struct + union + enum + bitmask + alias) plus 5 explicitly classified (collection-3 + bitset + annotation).

### Feature flags

| Feature | Default | Purpose |
|---------|---------|-------|
| `std`   | ✅      | std re-exports, implies `alloc` |
| `alloc` | ✅      | mandatory (Vec/String/BTreeMap) |
