# RFC 0004: XTypes 1.3 Integration

**Status:** Implementiert (WP 1.5, 2026-04-19)
**Autor:** Protocol-Team
**Verwandt:** RFC 0001 (IDL-Parser-Architektur), WP 1.4 (SEDP)

## 1 Motivation

OMG XTypes 1.3 ist der De-facto-Standard fuer Type-System + Discovery
in DDS-Ökosystemen. Die Spec deckt ab:

- **Strongly-hashed TypeIdentifiers** (EK_MINIMAL / EK_COMPLETE) —
  kompakte 14-byte-Referenzen statt voller Namen im Wire.
- **TypeLookup-Service** — Peer-zu-Peer-Nachladen unbekannter Typen via
  RPC-ueber-SEDP.
- **Extensibility-Kontrakte** (Final/Appendable/Mutable) —
  Vorwaerts/Rueckwaerts-Kompatibilitaet fuer Type-Evolution.
- **Type-Assignability** — Runtime-Kompatibilitaetscheck bei
  Publication/Subscription-Match.

XTypes-Kompatibilitaet ist in Migrationsprojekten oft das
Entscheidungskriterium. ZeroDDS implementiert den vollen Stack, nicht
nur ein Minimum-Cut.

## 2 Entscheidungen

### 2.1 Volle Breite statt Stretch-Goal

Die originale Roadmap markierte TypeLookup als "Stretch-Goal, notfalls
Phase 2". WP 1.5 revidiert das: voller XTypes-1.3-Stack in Phase 1,
weil XTypes-Kompatibilitaet strategisch fuer Migration ist.

### 2.2 TypeObject in eigenem Crate `zerodds-types`

Kein Cross-Linking mit `zerodds-rtps`: TypeObject lebt in einem eigenen
no_std+alloc-Crate. SEDP-Integration (`PID_TYPE_INFORMATION`)
transportiert die TypeInformation-Bytes **opaque**, ohne direkte
Type-Abhaengigkeit — verhindert zirkulaere Crate-Abhaengigkeiten und
erlaubt Build-Zeit-Type-Tools (`zerodds-idlc`) ohne RTPS-Abhaengigkeit.

### 2.3 SHA-256 + MD5 als Dependencies

Wire-Kompatibilitaet erfordert byte-exakte Hash-Algorithmen. Statt
eigene SHA-256 / MD5 zu implementieren nutzen wir die `sha2` + `md-5`
Crates aus der rust-crypto-Familie: pure-Rust, no_std+alloc, 0
transitive deps, widely audited.

### 2.4 Minimal + Complete parallel

Beide Formen werden vollstaendig implementiert. Minimal fuer effiziente
Wire-Transports + Equivalence, Complete fuer Tooling (Namen +
Annotationen).

### 2.5 IDL-Annotation-Semantik in zerodds-idl

Der Parser (WP 0.3) parst Annotations als generische `Annotation{name,
params}`. WP 1.5 fuegt eine Lowering-Schicht (`zerodds-idl::semantics`)
hinzu die auf ein typisiertes `BuiltinAnnotation`-Enum abbildet. Custom/
Vendor-Annotations bleiben im `custom`-Vec erhalten.

AST→TypeObject-Mapping (`lower_struct_to_minimal`) kombiniert IDL und
zerodds-types und ist der Schluessel fuer Fixture-Generation in Tests.

## 3 Architektur

### 3.1 Crate-Struktur

```
crates/types/                         # XTypes-1.3-Core
├── src/type_identifier/              # §7.3.4.2 TypeIdentifier
├── src/type_object/
│   ├── minimal/                      # §7.3.4.4 MinimalTypeObject
│   ├── complete/                     # §7.3.4.4 CompleteTypeObject
│   ├── common.rs                     # NameHash, AppliedAnnotation, etc.
│   ├── flags.rs                      # StructTypeFlag, Member-Flags
│   └── kinds.rs                      # TypeKind Discriminators
├── src/builder.rs                    # programmatischer Builder
├── src/hash.rs                       # SHA-256 Hash-Computation
├── src/type_information.rs           # §7.6.3.2.2 TypeInformation
├── src/type_lookup.rs                # §7.6.3.3 RPC-IDL
├── src/resolve.rs                    # TypeRegistry + Alias-Resolution
├── src/assignability.rs              # §7.2.4 Type-Compat
└── src/qos.rs                        # TypeConsistencyEnforcement + DataRepr

crates/discovery/src/type_lookup/     # TypeLookupStack (Transport-agnostisch)

crates/idl/src/semantics/
├── annotations.rs                    # typisiertes BuiltinAnnotation
└── to_typeobject.rs                  # AST → TypeObject Mapper

crates/rtps/src/
├── parameter_list.rs                 # + PID_TYPE_INFORMATION (0x0075),
│                                     #   PID_DATA_REPRESENTATION (0x0073),
│                                     #   PID_TYPE_CONSISTENCY (0x0074)
├── publication_data.rs               # + type_information: Option<Vec<u8>>,
└── subscription_data.rs              #   data_representation: Vec<i16>
```

### 3.2 Daten-Flow fuer Discovery

```
[IDL Source]
    │
    ▼  zerodds-idl::semantics
[StructDef] ──lower_struct_to_minimal──► [MinimalStructType]
    │
    ▼  zerodds-types::hash
[EquivalenceHash (14 byte SHA-256)]
    │
    ▼  zerodds-types::type_information
[TypeInformation]
    │
    ▼  to_bytes_le()
[opaque bytes] ──► PID_TYPE_INFORMATION in PublicationBuiltinTopicData
    │                                       │
    │                                       ▼
    │                                  [SEDP DATA]
    │                                       │
    │                                       ▼
    │                            Peer empfaengt, parst PID
    │                                       │
    │                                       ▼
    └──► Peer kennt Typ nicht? ──► TypeLookup::getTypes(hash)
                                           │
                                           ▼
                                   TypeObject in Peer-Registry
```

### 3.3 Data-Representation-Negotiation

1. Publisher annonciert `PID_DATA_REPRESENTATION = [XCDR2=2, XCDR1=0]`
2. Subscriber annonciert `PID_DATA_REPRESENTATION = [XCDR2, XCDR1]`
3. Match: erste Writer-Wahl, die im Reader vorhanden ist = XCDR2.

## 4 Interop-Matrix

| Vendor     | MinimalTypeObject | CompleteTypeObject | TypeLookup | Status          |
|------------|-------------------|--------------------|------------|-----------------|
| Cyclone DDS | ✓ Wire-Format getestet (T10) | ✓ Wire-Format | Wire-format OK, Live-RPC Phase 2 | bi-direktional Parse-OK |
| Fast DDS   | Same spec (XTypes 1.3) | Same | tbd        | theoretisch OK  |
| RTI Connext | tbd (vendor ext)  | tbd                | tbd        | Phase 2+        |

## 5 Migration-Guide (XCDR1 → XCDR2)

1. Publisher-Seite: `data_representation: vec![Xcdr2 as i16, Xcdr1 as i16]`
   (Writer offeriert beides, bevorzugt XCDR2).
2. Subscriber-Seite: `data_representation: vec![Xcdr2, Xcdr1]` akzeptiert beides.
3. Negotiation (T18) waehlt XCDR2 wenn beide unterstuetzen, faellt
   sonst auf XCDR1.

## 6 Offene Punkte fuer Phase 2

- TypeLookup-RPC-Wire-Flow uber ReliableWriter/Reader (WP 1.6 oder
  frueher WP-2.x).
- Union + Collection + Bitset TypeObject-Builder in `zerodds-types::builder`
  (heute nur Struct/Enum/Alias).
- `zerodds-idlc` komplettiert AST→TypeObject fuer Union/Enum/Alias/
  Collections + Codegen nach Rust-Structs.
- RTI-Vendor-Extensions (DynamicData) — deferred Phase 2+.

## 7 Referenzen

- OMG DDS-XTypes 1.3 (formal/2019-02-01)
- OMG DDS-RPC 1.0
- Eclipse Cyclone DDS XTypes Implementation
  (`src/core/ddsi/src/ddsi_typelib.c`)
