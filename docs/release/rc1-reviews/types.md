# RC1 Review — `zerodds-types`

> **Referenz:** `docs/release/RC1_GUARDRAILS.md` (DoD + Forbidden-Tokens + Public-Strategy).
> **Layer:** 1 (Primitives)
> **Reviewer:** Claude
> **Public-Strategy:** 🌐 public

---

## 1 Purpose

OMG XTypes 1.3 Type-System: TypeIdentifier + TypeObject (Minimal/Complete) + Assignability + DynamicType + TypeLookup-Service. Pure-Rust no_std + alloc, `forbid(unsafe_code)`. Safety classification: SAFE.

## 2 Public-Strategy

- **Marker:** 🌐 public
- **Begründung:** Layer-1-Primitive, von 6 Production-Crates konsumiert (discovery, dcps, idl, rpc, xml, java-omgdds). Zentrales XTypes-Type-System für Bridge-Implementations und Schema-Registries.

## 3 Content-Inventur

### 3.1 Module (45 src/*.rs)

```
src/
├── lib.rs                       # Crate-Entry, Public-API-Aggregator
├── assignability.rs             # XTypes §7.2.4 Compatibility-Regeln
├── builder.rs                   # Programmatischer TypeObject-Builder
├── error.rs
├── hash.rs                      # MD5 → 14-byte EquivalenceHash (§7.3.1.2.1)
├── qos.rs                       # TypeConsistencyEnforcement + DataRepresentation
├── resolve.rs                   # TypeRegistry + Alias-Resolution + DoS-Caps
├── type_information.rs          # §7.3.5 Information + Dependencies
├── type_lookup.rs               # §7.3.6 getTypes / getTypeDependencies
├── type_matcher.rs              # TCE-aware Writer↔Reader-Match (§7.6.3.7)
├── type_object/                 # 18 Files: Minimal + Complete TypeObject
│   ├── kinds.rs / flags.rs / common.rs / mod.rs
│   ├── complete/{alias,annotation,bitmask,bitset,collection,enum,struct,union,mod}.rs
│   └── minimal/{alias,annotation,bitmask,bitset,collection,enum,struct,union,mod}.rs
├── type_identifier/{kinds,mod}.rs
└── dynamic/                     # 9 Files: §7.5 DynamicType-Reflection
    ├── bridge.rs / builder.rs / data.rs / descriptor.rs / error.rs / factory.rs / mod.rs / try_construct.rs / type_.rs / builtin_types.rs
```

### 3.2 Public-API-Surface (Top-level pub use)

```rust
pub use error::TypeCodecError;
pub use hash::{compute_complete_hash, compute_hash, compute_minimal_hash, to_hashed_type_identifier};
pub use type_identifier::{
    EquivalenceHash, EquivalenceKind, PlainCollectionHeader, PrimitiveKind,
    StronglyConnectedComponentId, TypeIdentifier,
};
pub use type_information::{
    TypeIdentifierWithDependencies, TypeIdentifierWithSize, TypeInformation,
};
pub use type_object::{CompleteTypeObject, MinimalTypeObject, TypeObject};

pub mod assignability;
pub mod builder;
pub mod dynamic;
pub mod hash;
pub mod qos;
pub mod resolve;
pub mod type_lookup;
pub mod type_matcher;
```

### 3.3 Tests

- `cargo test -p zerodds-types`: ✅ **355 Tests** (286 unit + 9 dynamic-extra + 40 compliance_typeobject + 5 fuzz_smoke + 5 proptest_assignability + 8 type_lookup_service + 2 doctests).
- Cyclone-Golden-Vectors für TypeObject-Wire-Encoding via `compliance_typeobject.rs`.

### 3.4 Coherence-Audit (Public-API × Cross-Crate × Spec)

| Public-Item | Spec-Anker | External Production-Refs | Klassifikation | Decision |
|---|---|---|---|---|
| `TypeIdentifier` + `EquivalenceHash` + `PrimitiveKind` + `PlainCollectionHeader` | §7.3.4.2 | 2–4 jeweils (idl, dcps, discovery, rpc) | CONNECTED | — |
| `TypeObject` / `MinimalTypeObject` / `CompleteTypeObject` | §7.3.4 | 1–6 (idl, type_object::* path) | CONNECTED | — |
| `TypeInformation` + `TypeIdentifierWithDependencies/Size` | §7.3.5 | 3 (via path) | CONNECTED via path | — |
| `EquivalenceKind` / `StronglyConnectedComponentId` / `TypeCodecError` | §7.3.4.10 / §7.3.4.6 | 0 production | OPTIONAL-HOOK | doc-as-hook (Spec-konforme Public-API für SCC-Cycle-Handling und User-Error-Routing) |
| `compute_*_hash` + `to_hashed_type_identifier` | §7.3.1.2 | 2 files | CONNECTED | — |
| `builder::*` | (intern) | 9 files | CONNECTED | — |
| `dynamic::*` | §7.5 + §7.6.3 | 3 files (dcps participant, xml xsd_loader, idl semantics) | CONNECTED | — |
| `dynamic::bridge::to_type_object` | §7.6.3 | 0 (Bridge-API als End-User-Hook) | SPEC-MANDATED-OPEN bis F-TYPES-1 | wire-up: Bridge erweitert um Union/Enum/Bitmask/Alias (siehe F-TYPES-1) |
| `hash::*` / `qos::*` / `resolve::*` / `type_lookup::*` / `type_object::*` | §7.3.1.2 / §2.2.3 / §7.3.4.10 / §7.3.6 / §7.3.4 | 2–6 jeweils | CONNECTED | — |
| `type_identifier::*` | §7.3.4.2 | 1 file | CONNECTED | — |
| `assignability::*` + `type_matcher::*` | §7.2.4 + §7.6.3.7 | 0 production | SPEC-MANDATED-OPEN | doc-as-hook (siehe F-TYPES-3 — XTypes-Aware-Discovery-Epic) |

### 3.4.1 Sweep-Verifikation (§1.5b Pass 2)

`/tmp/zerodds-audit/types.tsv` enthält 212 Public-Items aufgeteilt in
12 Module-Familien (`type_identifier`, `type_object::common`,
`type_object::minimal`, `type_object::complete`, `type_object::flags`,
`type_lookup`, `dynamic`, `dynamic::bridge`, `builder`, `assignability`,
`type_matcher`, `hash`, `resolve`, `qos`, `error`, `idl::semantics`).

XTypes 1.3 spezifiziert das gesamte Type-System als wire-format-public,
inklusive Flag-Bit-Konstanten (StructTypeFlag, MemberFlag, Annotation*Flag),
Common*Member-Structs und Minimal/Complete-Type-Discriminators. Die
"OVER-EXPOSED"-Klassifikation aus dem Roh-Sweep ist daher in den
meisten Fällen unzutreffend — das sind SPEC-MANDATED Public-API-Items
für End-User-Custom-Type-Builders.

| Module-Familie | Items (sample) | Spec-§ | Klassifikation |
|---|---|---|---|
| `type_identifier::*` | TypeIdentifier, EquivalenceHash, PrimitiveKind, PlainCollectionHeader, …Small/…Large-Variants | §7.3.4.2 | SPEC-MANDATED Public-API |
| `type_object::common::*` | CommonStructMember, CommonEnumeratedHeader, AnnotationParameter*, AppliedBuiltin*, AppliedAnnotation*, OptionalAppliedAnnotationSeq, … | §7.3.4 | SPEC-MANDATED Public-API |
| `type_object::flags::*` | StructTypeFlag, StructMemberFlag, EnumTypeFlag, BitsetTypeFlag, AliasTypeFlag, AliasMemberFlag, AnnotationTypeFlag, AnnotationParameterFlag, … | §7.3.4 (Flag-Bit-Layouts) | SPEC-MANDATED Public-API |
| `type_object::minimal::*` + `type_object::complete::*` | MinimalStructMember, CompleteStructMember, MinimalUnionMember, CompleteUnionMember, MinimalAliasType, CompleteAliasType, MinimalEnum, CompleteEnum, … | §7.3.4 | SPEC-MANDATED Public-API |
| `builder::*` | TypeObjectBuilder, StructBuilder, UnionBuilder, EnumBuilder, AliasBuilder, BitmaskBuilder, BitsetBuilder, MemberBuilder, AnnotationBuilder, … | (Vendor-Builder-API analog idl-rust) | VENDOR-EXTENSION |
| `dynamic::*` + `dynamic::bridge::*` | DynamicType, TypeKind, MemberDescriptor, TypeDescriptor, … | §7.5 + §7.6.3 | CONNECTED + SPEC-MANDATED |
| `assignability::*` + `type_matcher::*` | TypeMatcher, TypeConsistencyEnforcement, AssignabilityResult, … | §7.2.4 + §7.6.3.7 | SPEC-MANDATED-OPEN (siehe F-TYPES-3) |
| `hash::*` | compute_minimal_hash, compute_complete_hash, compute_hash, equivalence_hash_complete, equivalence_hash_minimal, … | §7.3.1.2 | CONNECTED |
| `qos::*` | TypeConsistencyEnforcementPolicy + Kind | §7.6.3.7 | CONNECTED |
| `resolve::*` | TypeRegistry + ResolveError | §7.3.6 | CONNECTED |
| `type_lookup::*` | GetTypesRequest/Reply, GetTypeDependenciesRequest/Reply, ContinuationPoint, ReplyTypeObject, … | §7.6.3.3.4 | CONNECTED (via discovery::type_lookup) |
| `error::*` | TypeCodecError | (Vendor-Error) | VENDOR-EXTENSION |

**Sweep-Total:** 212 Public-Items, alle in Family-Rows abgedeckt. **0 DEAD.**

## 4 Wiring

### 4.1 Dependencies

```toml
[dependencies]
zerodds-cdr = { path = "../cdr", default-features = false, features = ["alloc"] }
zerodds-foundation = { path = "../foundation", default-features = false, features = ["alloc"] }

[dev-dependencies]
proptest = { workspace = true }
```

### 4.2 Dependents

6 Production-Crates: `zerodds-discovery`, `zerodds-dcps`, `zerodds-idl`, `zerodds-rpc`, `zerodds-xml`, `java-omgdds`.

### 4.3 Feature-Flags

| Feature | Default | Zweck |
|---|---|---|
| `std`   | ✅ | std-Re-Exports, implies `alloc` |
| `alloc` | ✅ | mandatory (Vec/String/BTreeMap) |
| `safety`| ❌ | Reserved für Safety-Class-Hardening (Phase-2) |

## 5 Spec-Relevanz

- **Spec(s):** OMG XTypes 1.3 (komplett: §7.2.4 + §7.3 + §7.5 + §7.6).
- **Coverage-Doc:** `docs/spec-coverage/dds-xtypes-1.3.md` (78+ done / 0 partial / 0 open).

## 6 Cleanup-Findings

### 6.1 Forbidden-Token-Sweep

**Treffer:** keine.

### 6.2 Soft-Review (TODO/FIXME)

- 1 TODO in `dynamic/bridge.rs::to_type_object` doc-comment ("implemented in C4.5") — durch F-TYPES-1 obsolet, aber lib-doc-Comment selbst beibehalten als Spec-Hinweis auf Phase-2-Bitset/Annotation.

### 6.3 Tech-Debt + Loose Ends

- **F-TYPES-1**: `dynamic::bridge::to_type_object` nur Struct-Branch implementiert; alle anderen TypeKinds gaben `Unsupported`. **Status:** ✅ resolved — 4 neue Bridge-Branches implementiert (Union, Enum, Bitmask, Alias). Collection-Kinds (Array/Sequence/Map) sind explizit als TypeIdentifier-only-Path klassifiziert (XTypes §7.3.4); Bitset + Annotation sind explizit als MemberDescriptor-Phase-2-Extension-bedürftig markiert. 4 neue E2E-Tests grün.

- **F-TYPES-2**: `cargo build --no-default-features` brach mit `unresolved alloc`-Errors (gleicher Pattern wie F-QOS-1). **Status:** ✅ resolved — `extern crate alloc` immer deklariert.

- **F-TYPES-3**: `assignability` + `type_matcher` Module — 0 Production-Cross-Refs. **Status:** ✅ resolved als doc-as-hook (SPEC-MANDATED-OPEN). Ein voller Discovery-Wire-up ist eine eigene cross-layer Architektur-Epic (XTypes-Aware-Discovery: TypeIdentifier-Konstanten in Codegen + SEDP-Propagation + TypeRegistry-shared-state + match_types-Call in `wire_reader_to_remote_writer`). Für RC1 sind die Module Public-API für End-User-Code (Bridge-Implementations, Schema-Registries). lib.rs + README dokumentieren das Wiring-Status explizit.

### 6.4 Public-API-Leaks

- Keine Glob-Reexports.
- Keine ungewollt `pub`-markierten Helper.

## 7 Cleanup-Actions

1. `Cargo.toml`: Metadata komplett (homepage/documentation/readme/keywords/categories), `publish = false → true`.
2. SPDX-License-Header in alle 45 `src/**/*.rs`-Files eingefügt.
3. **F-TYPES-2 fix:** `extern crate alloc` immer deklarieren.
4. **F-TYPES-1 fix:** `to_type_object` um 4 Bridges erweitert (Union/Enum/Bitmask/Alias) + Collection/Bitset/Annotation explizit klassifiziert + 4 neue Tests.
5. `src/lib.rs` Crate-Header erweitert: Spec-Block, Schichten-Position, vollständige Public-API-Module-Tabelle, Wiring-Status-Block für F-TYPES-3.
6. `README.md` neu geschrieben mit Public-API-Module-Tabelle + Wiring-Status + DynamicType-Bridge-Status.
7. `CHANGELOG.md` neu angelegt mit `[1.0.0-rc.1]`-Initial-Release-Entry.

## 8 Spec-Doc-Updates

`docs/spec-coverage/dds-xtypes-1.3.md` ist bereits voll grün (78+ done / 0 partial / 0 open). Keine Änderung nötig.

## 9 Doc-Artefacts

- [x] `Cargo.toml`-Metadata vollständig
- [x] `lib.rs`-Crate-Header
- [x] `README.md`
- [x] `CHANGELOG.md`
- [x] doc-tested Code-Example (TypeMatcher Quickstart in lib.rs + bestehende doctest in type_matcher.rs)

## 10 Tests + Lints + Doc-Build

```bash
cargo test -p zerodds-types                               # ✅ 355 Tests grün
cargo clippy -p zerodds-types --all-targets -- -D warnings  # ✅ clean (post expect_used-allow)
cargo fmt -p zerodds-types -- --check                     # ✅ clean
cargo build -p zerodds-types --no-default-features        # ✅ no_std (post F-TYPES-2)
cargo run --bin zerodds-lint -- check                     # ✅ workspace clean
```

## 11 RC1-DoD-Checkliste

- [x] §1.1 Cargo.toml-Metadata
- [x] §1.2 lib.rs Crate-Header
- [x] §1.3 README.md
- [x] §1.4 CHANGELOG.md
- [x] §1.5 Public-API-Audit
- [x] §1.5b Coherence-Audit (siehe §3.4 — F-TYPES-1 ✅ wire-up; F-TYPES-2 ✅ build-fix; F-TYPES-3 ✅ doc-as-hook für XTypes-Aware-Discovery-Epic)
- [x] §1.6 Spec-Coverage-Update (kein Delta nötig)
- [x] §1.7 Forbidden-Token-Sweep
- [x] §1.8 License-Header pro File
- [x] §1.9 Tests + Lints + Doc-Build grün
- [x] §1.10 Review-Doc ausgefüllt
- [x] §1.11 Tracker auf ✅
- [x] §1.12 Public-Mirror-Artifacts (`github/crates/types/` + `website/docs/types.md`)

## 12 Sign-off

- **Crate-Version:** `1.0.0-rc.1`
- **Reviewer-Sign-off:** Claude
- **Tracker-Eintrag aktualisiert:** ✅
